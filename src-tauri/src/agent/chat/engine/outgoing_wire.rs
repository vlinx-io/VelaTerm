//! Provider envelopes borrow manual originals and escape strings in fixed-size pieces.

use std::io::{self,Write};
use serde_json::Value;
use super::ChatImage;

fn json(writer:&mut dyn Write,value:&impl serde::Serialize) -> io::Result<()> {
    serde_json::to_writer(writer,value).map_err(io::Error::other)
}
fn string(writer:&mut dyn Write,parts:&[&str]) -> io::Result<()> {
    writer.write_all(b"\"")?;
    for part in parts {let mut from=0;while from<part.len() {
        let mut end=(from+32*1024).min(part.len());while !part.is_char_boundary(end) {end-=1;}
        let encoded=serde_json::to_vec(&part[from..end]).map_err(io::Error::other)?;
        writer.write_all(&encoded[1..encoded.len()-1])?;from=end;
    }}writer.write_all(b"\"")
}
pub(super) fn claude(writer:&mut dyn Write,text:&str,images:&[ChatImage],uuid:Option<&str>) -> io::Result<()> {
    writer.write_all(b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[")?;let mut comma=false;
    if !text.is_empty() {writer.write_all(b"{\"type\":\"text\",\"text\":")?;string(writer,&[text])?;writer.write_all(b"}")?;comma=true;}
    for image in images {
        if comma {writer.write_all(b",")?;}comma=true;
        writer.write_all(b"{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":")?;string(writer,&[&image.mime_type])?;
        writer.write_all(b",\"data\":")?;string(writer,&[&image.data])?;writer.write_all(b"}}")?;
    }
    writer.write_all(b"]}")?;if let Some(uuid)=uuid {writer.write_all(b",\"uuid\":")?;json(writer,&uuid)?;}writer.write_all(b"}")
}
pub(super) fn pi(writer:&mut dyn Write,id:&str,kind:&str,text:&str,images:&[ChatImage]) -> io::Result<()> {
    writer.write_all(b"{\"id\":")?;json(writer,&id)?;writer.write_all(b",\"type\":")?;json(writer,&kind)?;
    writer.write_all(b",\"message\":")?;string(writer,&[text])?;
    if !images.is_empty() {
        writer.write_all(b",\"images\":[")?;for (index,image) in images.iter().enumerate() {
            if index>0 {writer.write_all(b",")?;}writer.write_all(b"{\"type\":\"image\",\"data\":")?;string(writer,&[&image.data])?;
            writer.write_all(b",\"mimeType\":")?;string(writer,&[&image.mime_type])?;writer.write_all(b"}")?;
        }writer.write_all(b"]")?;
    }writer.write_all(b"}")
}
pub(super) fn codex(writer:&mut dyn Write,request:&Value,text:&str,images:&[ChatImage]) -> io::Result<()> {
    let root=request.as_object().ok_or_else(||io::Error::other("Invalid Codex request envelope"))?;
    let params=request["params"].as_object().ok_or_else(||io::Error::other("Invalid Codex request parameters"))?;
    let extras=params.get("input").and_then(Value::as_array).ok_or_else(||io::Error::other("Invalid Codex request inputs"))?;
    if extras.iter().any(|input|input["type"]!="skill") {return Err(io::Error::other("Codex envelope must contain only small additional skill inputs"));}
    writer.write_all(b"{")?;
    for (index,(key,value)) in root.iter().enumerate() {
        if index>0 {writer.write_all(b",")?;}json(writer,key)?;writer.write_all(b":")?;
        if key!="params" {json(writer,value)?;continue;}
        writer.write_all(b"{")?;
        for (index,(key,value)) in params.iter().enumerate() {
            if index>0 {writer.write_all(b",")?;}json(writer,key)?;writer.write_all(b":")?;
            if key!="input" {json(writer,value)?;continue;}
            writer.write_all(b"[")?;let mut comma=false;
            if !text.is_empty() {writer.write_all(b"{\"type\":\"text\",\"text\":")?;string(writer,&[text])?;
                writer.write_all(b",\"text_elements\":[]}")?;comma=true;}
            for image in images {
                if comma {writer.write_all(b",")?;}comma=true;writer.write_all(b"{\"type\":\"image\",\"url\":")?;
                string(writer,&["data:",&image.mime_type,";base64,",&image.data])?;writer.write_all(b"}")?;
            }
            for extra in extras {if comma {writer.write_all(b",")?;}comma=true;json(writer,extra)?;}
            writer.write_all(b"]")?;
        }writer.write_all(b"}")?;
    }writer.write_all(b"}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{protocol,codex_protocol};
    #[test]
    fn streamed_envelopes_match_existing_provider_shapes_including_skills_images_and_empty_captions() {
        let images=[ChatImage {mime_type:"image/png".into(),data:"AP8=雪\\\"".into()}];
        for text in ["", "雪😀\n\"escaped\\text\" $review"] {for images in [&[][..],&images[..]] {
            let mut actual=Vec::new();claude(&mut actual,text,images,None).unwrap();assert_eq!(serde_json::from_slice::<Value>(&actual).unwrap(),protocol::user_message(text,images));
            for kind in ["prompt","steer"] {
                actual.clear();pi(&mut actual,"input",kind,text,images).unwrap();let expected=if kind=="prompt" {crate::agent::chat::pi_protocol::prompt("input",text,images)} else {crate::agent::chat::pi_protocol::steer("input",text,images)};
                assert_eq!(serde_json::from_slice::<Value>(&actual).unwrap(),expected);
            }
            let commands=[serde_json::json!({"name":"review","skillPath":"/verified/review"})];
            for steer in [true,false] {
                let request=|text,images|if steer {codex_protocol::turn_steer("input","thread","turn",text,images)} else {codex_protocol::turn_start("input","thread",text,images,codex_protocol::TurnOptions {model:Some("model"),mode:"auto",..Default::default()})};
                let expected=crate::agent::chat::skills::with_inputs(request(text,images),text,&commands);
                let envelope=crate::agent::chat::skills::with_inputs(request("",&[]),text,&commands);
                actual.clear();codex(&mut actual,&envelope,text,images).unwrap();assert_eq!(serde_json::from_slice::<Value>(&actual).unwrap(),expected);
            }
        }}
    }
    #[test]
    fn long_strings_escape_across_utf8_boundaries_without_a_whole_encoded_frame() {
        struct Counting {largest:usize,bytes:u64}
        impl Write for Counting {
            fn write(&mut self,bytes:&[u8]) -> io::Result<usize> {self.largest=self.largest.max(bytes.len());self.bytes+=bytes.len() as u64;Ok(bytes.len())}
            fn flush(&mut self) -> io::Result<()> {Ok(())}
        }
        let text="雪😀\n\\\"\u{0}".repeat(400_000);let images=[ChatImage {mime_type:"image/png".into(),data:"AP8=".repeat(800_000)}];
        let mut writer=Counting {largest:0,bytes:0};claude(&mut writer,&text,&images,Some("uuid")).unwrap();
        assert!(writer.bytes>text.len() as u64+images[0].data.len() as u64);assert!(writer.largest<=256*1024);
        let mut writer=Counting {largest:0,bytes:0};let envelope=codex_protocol::turn_steer("input","thread","turn","",&[]);
        codex(&mut writer,&envelope,&text,&images).unwrap();assert!(writer.largest<=256*1024);
    }
}
