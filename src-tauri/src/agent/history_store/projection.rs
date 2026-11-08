//! A JSONL structural projection that never allocates a complete physical record.
//!
//! Large strings retain only a UTF-8 preview. Their raw ranges and independently verifiable decode
//! checkpoints are sent to the caller while bytes arrive. Limits apply to unknown fields as well as
//! known provider fields, and unsupported records are reported explicitly instead of replayed in full.

use std::io::{self, Read};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::limits::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub path: String,
    /// Raw content range, excluding the surrounding JSON quotes.
    pub start: u64,
    pub end: u64,
    pub decoded_bytes: u64,
    pub truncated: bool,
    pub encoding: String,
    /// Hash of the raw range of a string stored without a checkpoint; None when checkpoints cover it.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub hash: Option<Vec<u8>>,
}

/// Field names that carry readable content, as opposed to signatures, identities, or tool metadata.
fn semantic_path(path:&str) -> bool {
    matches!(path.rsplit('/').next().unwrap_or_default(),
        "text"|"thinking"|"output"|"content"|"prompt"|"data"|"arguments"|"diff"|"unified_diff"|"unifiedDiff"|"delta"|"aggregated_output"|"aggregatedOutput")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub record_start: u64,
    pub path: String,
    pub raw_start: u64,
    pub raw_end: u64,
    pub decoded_start: u64,
    pub decoded_end: u64,
    pub hash: Vec<u8>,
    pub sequence: u64,
}

#[derive(Debug)]
pub struct Record {
    pub start: u64,
    pub end: u64,
    pub value: Value,
    pub fields: Vec<Field>,
    pub error: Option<String>,
    /// Only a physical newline makes a JSONL record durable while its writer is still running.
    pub complete: bool,
}

pub(super) fn identity_path(path:&str) -> bool {
    let parts=path.split('/').collect::<Vec<_>>();
    if matches!(parts.as_slice(),["","message_ids",ordinal] if !ordinal.is_empty() && ordinal.bytes().all(|byte|byte.is_ascii_digit())) {return true;}
    if matches!(parts.as_slice(),["","payload"|"params","item","receiver_thread_ids"|"receiverThreadIds",ordinal] if !ordinal.is_empty() && ordinal.bytes().all(|byte|byte.is_ascii_digit())) {return true;}
    let Some(name)=parts.last() else {return false;};
    if !matches!(*name,"type"|"id"|"uuid"|"role"|"name"|"method"|"event"|"call_id"|"callId"|"tool_use_id"|"toolCallId"|"toolName"|"tool"|"sessionUpdate"|
        "parentUuid"|"logicalParentUuid"|"parentId"|"status"|"timestamp"|"modelID"|"providerID"|"model"|"request_id"|"session_id"|
        "sessionId"|"sessionID"|"agentId"|"agent_thread_id"|"agentThreadId"|"thread_id"|"threadId"|"turn_id"|"turnId"|"itemId"|"subtype"|"step_type"|
        "version"|"kind"|"conversation_id"|"message_id"|"model_id"|"created_at"|"updated_at"|"cwd"|"current_working_directory"|"step_index"|"isSidechain"|"isMeta"|"rewound") {return false;}
    let index=|value:&&str|!value.is_empty() && value.bytes().all(|byte|byte.is_ascii_digit());
    match parts.as_slice() {
        ["",_]=>true,
        ["","payload"|"message"|"params"|"request"|"state",_]=>true,
        ["","payload"|"params","item"|"turn"|"thread"|"history_base"|"historyBase",_]=>true,
        ["","params","update",_]=>true,
        ["","message"|"payload","content"|"summary",ordinal,_]=>index(ordinal),
        ["","content"|"tool_calls",ordinal,_]=>index(ordinal),
        ["","payload"|"params","item","content",ordinal,_]=>index(ordinal),
        ["","part",_]=>true,
        ["","data",_]=>true,
        ["","data","content",ordinal,_]=>index(ordinal),
        ["","assistant","Response",_]=>true,
        ["","user",_]=>true,
        ["","toolUseResult",_]=>true,
        ["","part","state","metadata",_]=>true,
        ["","model_info",_]=>true,
        ["","session_state","rts_model_state",_]=>true,
        ["","session_state","rts_model_state","model_info",_]=>true,
        ["","user","env_context","env_state",_]=>true,
        _=>false,
    }
}

pub struct Projector<R> {
    reader: R,
    buffer: Box<[u8]>,
    at: usize,
    filled: usize,
    offset: u64,
    record_start: u64,
    fields: Vec<Field>,
    nodes: usize,
    preview_bytes: usize,
    metadata_bytes: usize,
    semantic_bytes: usize,
    classification_bytes: usize,
    key_bytes: usize,
    capture: Option<Vec<u8>>,
    document: bool,
    /// Skip the checkpoint of a string that fits in one piece; readers decode its raw range directly.
    long_only: bool,
}

impl<R: Read> Projector<R> {
    pub fn new(reader: R, offset: u64) -> Self {
        Self {
            reader, buffer: vec![0; INPUT_BYTES].into_boxed_slice(), at: 0, filled: 0, offset,
            record_start: offset, fields: Vec::new(), nodes: 0, preview_bytes: 0, metadata_bytes: 0, semantic_bytes: 0, classification_bytes: 0, key_bytes: 0, capture: None, document:false, long_only:false,
        }
    }

    /// Emit checkpoints only for strings longer than one piece. A short string's raw range is read
    /// and verified against the source blocks when it is needed, so storing its hash would only
    /// duplicate what the index already guarantees.
    pub fn long_checkpoints_only(mut self) -> Self {self.long_only=true;self}

    pub fn document(reader:R,offset:u64) -> Self {
        let mut parser=Self::new(reader,offset);parser.document=true;parser
    }

    #[cfg(test)] pub fn offset(&self) -> u64 { self.offset }

    fn reset(&mut self) {
        self.record_start=self.offset;self.fields.clear();self.nodes=0;self.preview_bytes=0;
        self.metadata_bytes=0;self.semantic_bytes=0;self.classification_bytes=0;self.key_bytes=0;self.capture=None;
    }

    fn peek(&mut self) -> Result<Option<u8>, String> {
        if self.at == self.filled {
            self.filled = self.reader.read(&mut self.buffer).map_err(|error| {
                // A reader that verifies what it returns reports a stable code; keep it for the caller.
                let error=error.to_string();
                if ["source_changed:","detail_unavailable:","index_incomplete:","read_canceled"].iter().any(|code|error.starts_with(code)) {error}
                else {format!("History read failed: {error}")}
            })?;
            self.at = 0;
        }
        Ok((self.at < self.filled).then(|| self.buffer[self.at]))
    }

    fn byte(&mut self) -> Result<u8, String> {
        let byte = self.peek()?.ok_or("Incomplete JSON record")?;
        self.at += 1;
        self.offset += 1;
        if let Some(capture) = &mut self.capture { capture.push(byte); }
        Ok(byte)
    }

    /// Consume one JSON value without keeping any of it.
    fn skip(&mut self) -> Result<(), String> {
        let mut depth=0usize;
        loop {
            self.space()?;
            match self.peek()?.ok_or("Incomplete JSON record")? {
                b'"'=>{
                    self.byte()?;
                    loop {
                        // Pass ordinary bytes a buffer at a time; skipped strings are the largest ones.
                        if self.capture.is_none() {
                            let available=&self.buffer[self.at..self.filled];
                            let plain=available.iter().position(|b|*b<0x20 || matches!(*b,b'"'|b'\\')).unwrap_or(available.len());
                            self.at+=plain;self.offset+=plain as u64;
                        }
                        match self.byte()? {b'\\'=>{self.byte()?;},b'"'=>break,byte if byte<0x20=>return Err("Invalid JSON string".into()),_=>{}}
                    }
                },
                b'{'|b'['=>{
                    self.byte()?;depth+=1;
                    if depth>MAX_DEPTH {return Err("unsupported_record: JSON structure exceeds the projection budget".into());}
                    continue;
                },
                b'}'|b']'=>{if depth==0 {return Err("Invalid JSON structure".into());}self.byte()?;depth-=1;},
                b','|b':' if depth>0=>{self.byte()?;continue;},
                b'-'|b'0'..=b'9'|b't'|b'f'|b'n'=>{while matches!(self.peek()?,Some(b'-'|b'+'|b'.'|b'0'..=b'9'|b'a'..=b'z'|b'E')) {self.byte()?;}},
                _=>return Err("Invalid JSON value".into()),
            }
            if depth==0 {return Ok(());}
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        if self.peek()? != Some(expected) { return Err("Invalid JSON structure".into()); }
        self.byte()?;
        Ok(())
    }

    fn space(&mut self) -> Result<(), String> {
        // A newline is a physical recovery boundary and cannot belong to a JSONL value.
        while matches!(self.peek()?, Some(b' ' | b'\t' | b'\r')) || (self.document && self.peek()?==Some(b'\n')) { self.byte()?; }
        Ok(())
    }

    pub fn next(&mut self, sink: &mut impl FnMut(Checkpoint) -> Result<(), String>) -> Result<Option<Record>, String> {
        if self.peek()?.is_none() { return Ok(None); }
        self.reset();
        self.space()?;
        let result = self.value("", 0, sink).and_then(|value| {
            self.space()?;
            if !matches!(self.peek()?, None | Some(b'\n')) { return Err("Trailing JSON content".into()); }
            Ok(value)
        });
        self.capture = None;
        let (value, error) = match result {
            Ok(value) => (value, None),
            Err(error) => {
                if !(error.starts_with("Invalid JSON") || error.starts_with("Incomplete JSON")
                    || error.starts_with("unsupported_record:") || error=="Trailing JSON content") {
                    // Storage, cancellation, and transport failures must stop promptly. Recovery to the
                    // next line is only for syntax/structure failures and cannot conceal failed spooling.
                    return Err(error);
                }
                // Never retain a malformed or over-budget tail. Its physical range remains known.
                while !matches!(self.peek()?, None | Some(b'\n')) { self.byte()?; }
                (Value::Null, Some(error))
            }
        };
        let complete = self.peek()? == Some(b'\n');
        if complete { self.byte()?; }
        Ok(Some(Record { start: self.record_start, end: self.offset, value,
            fields: std::mem::take(&mut self.fields), error, complete }))
    }

    fn value(&mut self, path: &str, depth: usize, sink: &mut impl FnMut(Checkpoint) -> Result<(), String>) -> Result<Value, String> {
        if depth > MAX_DEPTH || path.len() > MAX_KEY_BYTES * 4 || self.nodes >= MAX_NODES {
            return Err("unsupported_record: JSON structure exceeds the projection budget".into());
        }
        self.space()?;
        // A scan never stores these subtrees; read past them without building or counting them.
        if self.long_only && !super::adapter::addressed(path) {self.skip()?;return Ok(Value::Null);}
        self.nodes += 1;
        let start=self.offset;
        let container=matches!(self.peek()?,Some(b'{'|b'['));
        let mut number_bytes=None;
        let parsed:Result<Value,String>=match self.peek()? {
            Some(b'"') => {
                if self.fields.len() >= MAX_FIELDS { return Err("unsupported_record: too many JSON fields".into()); }
                let (text, field) = self.string(path, false, sink)?;
                self.fields.push(field);
                Ok(Value::String(text))
            }
            Some(b'{') => {
                self.byte()?;
                let mut object = Map::new();
                self.space()?;
                if self.peek()? != Some(b'}') {
                    loop {
                        let (key, _) = self.string(path, true, sink)?;
                        self.space()?;
                        self.expect(b':')?;
                        let child = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                        let value = self.value(&child, depth + 1, sink)?;
                        object.insert(key, value);
                        self.space()?;
                        if self.peek()? != Some(b',') { break; }
                        self.byte()?;
                        self.space()?;
                    }
                }
                self.expect(b'}')?;
                Ok(Value::Object(object))
            }
            Some(b'[') => {
                self.byte()?;
                let mut array = Vec::new();
                self.space()?;
                if self.peek()? != Some(b']') {
                    loop {
                        let child = format!("{path}/{}", array.len());
                        array.push(self.value(&child, depth + 1, sink)?);
                        self.space()?;
                        if self.peek()? != Some(b',') { break; }
                        self.byte()?;
                    }
                }
                self.expect(b']')?;
                Ok(Value::Array(array))
            }
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => {
                let mut number = Vec::new();
                while matches!(self.peek()?, Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')) {
                    if number.len() == 128 { return Err("unsupported_record: JSON number exceeds the projection budget".into()); }
                    number.push(self.byte()?);
                }
                let value=serde_json::from_slice(&number).map_err(|_| "Invalid JSON number".into());number_bytes=Some(number);value
            }
            _ => Err("Invalid JSON value".into()),
        };
        let value=parsed?;
        if container || !value.is_string() && identity_path(path) {
            if self.fields.len()>=MAX_FIELDS {return Err("unsupported_record: too many JSON fields".into());}
            // A scalar's original bytes are exactly its literal. Its own hash lets a read verify only
            // those bytes instead of the whole source block around them.
            let hash=(self.long_only && !container).then(||Sha256::digest(number_bytes.unwrap_or_else(||value.to_string().into_bytes())).to_vec());
            self.fields.push(Field {path:path.into(),start,end:self.offset,decoded_bytes:self.offset-start,
                truncated:self.offset-start>PREVIEW_BYTES as u64,encoding:"json".into(),hash});
        }
        Ok(value)
    }

    fn literal(&mut self, expected: &[u8], value: Value) -> Result<Value, String> {
        for byte in expected { self.expect(*byte)?; }
        Ok(value)
    }

    fn hex(&mut self) -> Result<u16, String> {
        let mut value = 0u16;
        for _ in 0..4 {
            let digit = match self.peek()? {
                Some(b'0'..=b'9') => self.byte()? - b'0',
                Some(b'a'..=b'f') => self.byte()? - b'a' + 10,
                Some(b'A'..=b'F') => self.byte()? - b'A' + 10,
                _ => return Err("Invalid JSON Unicode escape".into()),
            };
            value = (value << 4) | u16::from(digit);
        }
        Ok(value)
    }

    fn character(&mut self) -> Result<char, String> {
        let first = self.peek()?.ok_or("Incomplete JSON string")?;
        if first < 0x20 { return Err("Invalid JSON string control character".into()); }
        self.byte()?;
        if first == b'\\' {
            return match self.peek()? {
                Some(b'"' | b'\\' | b'/') => Ok(self.byte()? as char),
                Some(b'b') => { self.byte()?; Ok('\u{8}') }
                Some(b'f') => { self.byte()?; Ok('\u{c}') }
                Some(b'n') => { self.byte()?; Ok('\n') }
                Some(b'r') => { self.byte()?; Ok('\r') }
                Some(b't') => { self.byte()?; Ok('\t') }
                Some(b'u') => {
                    self.byte()?;
                    let high = self.hex()?;
                    let code = if (0xd800..=0xdbff).contains(&high) {
                        self.expect(b'\\')?;
                        self.expect(b'u')?;
                        let low = self.hex()?;
                        if !(0xdc00..=0xdfff).contains(&low) { return Err("Invalid JSON surrogate pair".into()); }
                        0x10000 + ((u32::from(high) - 0xd800) << 10) + u32::from(low) - 0xdc00
                    } else { u32::from(high) };
                    char::from_u32(code).ok_or_else(|| "Invalid JSON Unicode scalar".into())
                }
                _ => Err("Invalid JSON escape".into()),
            };
        }
        if first < 0x80 { return Ok(first as char); }
        let width = match first { 0xc2..=0xdf => 2, 0xe0..=0xef => 3, 0xf0..=0xf4 => 4,
            _ => return Err("Invalid JSON UTF-8".into()) };
        let mut bytes = [0u8; 4];
        bytes[0] = first;
        for byte in &mut bytes[1..width] {
            let next = self.peek()?.ok_or("Incomplete JSON UTF-8")?;
            if next & 0xc0 != 0x80 { return Err("Invalid JSON UTF-8".into()); }
            *byte = self.byte()?;
        }
        std::str::from_utf8(&bytes[..width]).ok().and_then(|s| s.chars().next())
            .ok_or_else(|| "Invalid JSON UTF-8".into())
    }

    fn string(&mut self, path: &str, key: bool, sink: &mut impl FnMut(Checkpoint) -> Result<(), String>) -> Result<(String, Field), String> {
        self.expect(b'"')?;
        let start = self.offset;
        let mut raw_start = start;
        let mut decoded_start = 0;
        let mut decoded_bytes = 0;
        let mut sequence=0;
        let mut text = String::new();
        // Classification and identity can follow an arbitrary number of large payload fields. Reserve
        // their own bounded budget so field order never turns a valid type/id into an empty preview.
        let metadata=!key && identity_path(path);
        let semantic=!key && semantic_path(path);
        let limit=if key {MAX_KEY_BYTES} else if metadata {MAX_KEY_BYTES.min((64*1024usize).saturating_sub(self.metadata_bytes))}
            else if semantic {PREVIEW_BYTES.min((RECORD_PREVIEW_BYTES/2-MAX_FIELDS*4-4096).saturating_sub(self.semantic_bytes))}
            else {PREVIEW_BYTES.min((RECORD_PREVIEW_BYTES/2).saturating_sub(self.preview_bytes))};
        // A later communicated block must remain classifiable even if earlier thinking or unknown
        // fields consume the previews. Reserve one non-whitespace codepoint per field and a short
        // prefix for the first user block, whose injected-context markers determine visibility.
        let first=matches!(path,"/message/content"|"/message/content/0/text"|"/payload/content/0/text"|"/user/content/Prompt/prompt");
        let extra=4096usize.saturating_sub(self.classification_bytes.saturating_sub(self.fields.len()*4));
        let probe_limit=if semantic && !metadata {4+if first {124.min(extra)} else {0}} else {0};
        let mut probe=String::new();
        if !key { self.capture = Some(Vec::with_capacity(CHECKPOINT_BYTES + 16)); }
        while self.peek()? != Some(b'"') {
            // Consume ordinary ASCII in slices. Long diffs and Base64 spend almost all their time here.
            let available = &self.buffer[self.at..self.filled];
            let plain = available.iter().position(|b| *b < 0x20 || *b >= 0x80 || matches!(*b, b'"' | b'\\'))
                .unwrap_or(available.len());
            let plain = plain.min(CHECKPOINT_BYTES.saturating_sub(self.capture.as_ref().map_or(0, Vec::len)));
            if plain > 0 {
                if probe.len()<probe_limit {
                    let prefix=std::str::from_utf8(&available[..plain]).unwrap();
                    let prefix=if probe.is_empty() {prefix.trim_start()} else {prefix};
                    probe.push_str(&prefix[..prefix.len().min(probe_limit-probe.len())]);
                }
                let kept = plain.min(limit.saturating_sub(decoded_bytes as usize));
                text.push_str(std::str::from_utf8(&available[..kept]).unwrap());
                if let Some(capture) = &mut self.capture { capture.extend_from_slice(&available[..plain]); }
                self.at += plain;
                self.offset += plain as u64;
                decoded_bytes += plain as u64;
            } else {
                let ch = self.character()?;
                if probe.len()+ch.len_utf8()<=probe_limit && (!probe.is_empty() || !ch.is_whitespace()) {probe.push(ch);}
                decoded_bytes += ch.len_utf8() as u64;
                if decoded_bytes <= limit as u64 { text.push(ch); }
            }
            if key && decoded_bytes > limit as u64 { return Err("unsupported_record: JSON key exceeds the projection budget".into()); }
            if metadata && decoded_bytes>limit as u64 {return Err("unsupported_record: identity metadata exceeds the projection budget".into());}
            if self.capture.as_ref().is_some_and(|bytes| bytes.len() >= CHECKPOINT_BYTES) {
                let bytes = self.capture.as_mut().unwrap();
                sink(Checkpoint { record_start: self.record_start, path: path.to_owned(), raw_start,
                    raw_end: self.offset, decoded_start, decoded_end: decoded_bytes, hash: Sha256::digest(&*bytes).to_vec(),sequence })?;
                sequence+=1;
                bytes.clear();
                raw_start = self.offset;
                decoded_start = decoded_bytes;
            }
        }
        let end = self.offset;
        let mut hash=None;
        if let Some(bytes) = self.capture.take() {
            if self.long_only && sequence==0 {hash=Some(Sha256::digest(&bytes).to_vec());}
            else if !bytes.is_empty() {
                sink(Checkpoint { record_start: self.record_start, path: path.to_owned(), raw_start,
                    raw_end: end, decoded_start, decoded_end: decoded_bytes, hash: Sha256::digest(&bytes).to_vec(),sequence })?;
            }
        }
        self.expect(b'"')?;
        let substituted=semantic && decoded_bytes>text.len() as u64 && text.trim().is_empty() && !probe.is_empty();
        if substituted {text=probe;self.classification_bytes+=text.len();}
        if key {
            self.key_bytes+=text.len();
            if self.key_bytes>64*1024 {return Err("unsupported_record: JSON keys exceed the projection budget".into());}
        } else if metadata {self.metadata_bytes+=text.len();} else if semantic && !substituted {self.semantic_bytes+=text.len();} else if !semantic {self.preview_bytes+=text.len();}
        Ok((text, Field { path: path.to_owned(), start, end, decoded_bytes, truncated: substituted || decoded_bytes > limit as u64,encoding:"string".into(),hash }))
    }
}

/// Decode one independently verified checkpoint. Its size is bounded before allocation by the caller.
pub fn decode_checkpoint(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > CHECKPOINT_BYTES + 16 { return Err("History checkpoint exceeds its byte budget".into()); }
    let mut quoted = Vec::with_capacity(bytes.len() + 2);
    quoted.push(b'"'); quoted.extend_from_slice(bytes); quoted.push(b'"');
    serde_json::from_slice(&quoted).map_err(|_| "History checkpoint has invalid JSON encoding".into())
}

impl<R: Read> Read for Projector<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let mut count = 0;
        while count < output.len() {
            match self.peek().map_err(io::Error::other)? {
                Some(_) => { output[count] = self.byte().map_err(io::Error::other)?; count += 1; }
                None => break,
            }
        }
        Ok(count)
    }
}
