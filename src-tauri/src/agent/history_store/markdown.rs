//! Stream the existing context-export sections without collecting event bodies or the document.

use std::io::Write;
use std::sync::atomic::AtomicBool;

use crate::agent::export::Event;
use super::{api::Branch,event_stream::{Cursor,Text}};

fn write(writer:&mut impl Write,text:&str) -> Result<(),String> {writer.write_all(text.as_bytes()).map_err(|e|e.to_string())}

fn literal(writer:&mut impl Write,text:&str,language:&str) -> Result<(),String> {
    let mut longest=0;let mut run=0;
    for ch in text.chars() {if ch=='`' {run+=1;longest=longest.max(run);} else {run=0;}}
    let fence="`".repeat((longest+1).max(3));write(writer,&format!("{fence}{language}\n{}\n{fence}",text.trim_end_matches('\n')))
}

fn fenced(branch:&Branch,body:&Text,cancel:&AtomicBool,writer:&mut impl Write,language:&str) -> Result<(),String> {
    let mut longest=0;let mut run=0;let mut bytes=0u64;let mut end=0u64;
    body.walk(branch,cancel,&mut |text| {
        for ch in text.chars() {
            if ch=='`' {run=(run+1).min(129);longest=longest.max(run);} else {run=0;}
            bytes+=ch.len_utf8() as u64;if ch!='\n' {end=bytes;}
        }Ok(())
    })?;
    if longest>128 {
        // An indented code block avoids a fence proportional to a pathological backtick-only line.
        let mut line=true;
        body.walk(branch,cancel,&mut |text| {
            for piece in text.split_inclusive('\n') {
                if line {write(writer,"    ")?;}write(writer,piece)?;line=piece.ends_with('\n');
            }Ok(())
        })
    } else {
        let fence="`".repeat((longest+1).max(3));write(writer,&format!("{fence}{language}\n"))?;
        if end>0 {body.write_range(branch,cancel,writer,0,end)?;write(writer,"\n")?;}
        write(writer,&fence)
    }
}

pub fn write_markdown(branch:Branch,name:&str,exported_at:Option<&str>,cancel:&AtomicBool,writer:&mut impl Write) -> Result<(),String> {
    let _work=super::work::Work::acquire(cancel)?;
    let mut cursor=Cursor::new(branch,cancel)?;let branch=cursor.branch.clone();let (mut users,mut turns,mut calls)=(0u64,0u64,0u64);let mut role="";
    while let Some(item)=cursor.next(cancel)? {
        match item.event {
            Event::Command {..}=>continue,
            Event::User {..}|Event::Shell {..}=>{users+=1;role="user";},
            _=>{if role!="assistant" {turns+=1;role="assistant";}if matches!(item.event,Event::ToolUse {..}) {calls+=1;}}
        }
    }
    let path=branch.sources.last().map(|source|source.segment.path.display().to_string()).unwrap_or_default();
    write(writer,&format!("# Session Context Export: {name}\n\n- **Session kind**: {}\n- **Agent Session ID**: `{}`\n- **Transcript file**: `{path}`\n",branch.kind.as_str(),branch.native_id))?;
    if let Some(at)=exported_at {write(writer,&format!("- **Exported at**: {at}\n"))?;}
    write(writer,&format!("- **Content stats**: {users} user messages · {turns} assistant turns · {calls} tool calls\n\n"))?;
    write(writer,"> This file contains the full context of the session's main conversation: user messages, assistant replies with thinking, and the input/result of every tool call.\n> Sub-agent internal messages and system-injected reminder blocks are excluded; for raw data, see the transcript file above.\n\n---\n\n")?;
    cursor.rewind();let mut role="";let mut blocks=0;
    while let Some(item)=cursor.next(cancel)? {
        // Like the transcript export, a background-task wake is a boundary in the view, not context.
        if matches!(item.event,Event::Command {..}|Event::Wake {..}) {continue;}
        if blocks>0 {write(writer,"\n\n")?;}blocks+=1;
        let (next,label,at)=match &item.event {
            Event::User {ts,..}|Event::Shell {ts,..}=>("user","👤 User",ts.as_deref()),
            Event::AssistantText {ts,..}|Event::Thinking {ts,..}|Event::ToolUse {ts,..}=>("assistant","🤖 Assistant",ts.as_deref()),
            _=>("assistant","🤖 Assistant",None),
        };
        if next!=role {
            write(writer,&format!("## {label}{}\n\n",at.filter(|at|!at.is_empty()).map(|at|format!(" · {at}")).unwrap_or_default()))?;role=next;
        }
        match item.event {
            Event::User {..}|Event::AssistantText {..}=>item.body.write(&branch,cancel,writer,!branch.kind.uses_opencode()&&branch.kind!=crate::models::SessionKind::Kiro)?,
            Event::Thinking {..}=> {
                write(writer,"**💭 Thinking**\n\n")?;let mut line=true;
                item.body.walk(&branch,cancel,&mut |text| {
                    for piece in text.split_inclusive('\n') {
                        if line {write(writer,if piece=="\n" {">"} else {"> "})?;}
                        write(writer,piece)?;line=piece.ends_with('\n');
                    }Ok(())
                })?;
            },
            Event::ToolUse {name,..}=> {write(writer,&format!("**🔧 Tool call: {name}**\n\n"))?;fenced(&branch,&item.body,cancel,writer,"json")?;},
            Event::ToolResult {name,is_error,..}=> {
                write(writer,&format!("**📥 Tool result{}{}**\n\n",name.map(|name|format!(" ({name})")).unwrap_or_default(),if is_error {" · error"} else {""}))?;
                fenced(&branch,&item.body,cancel,writer,"")?;
            },
            Event::Shell {command,exit_code,cancelled,truncated,..}=> {
                literal(writer,&command,"sh")?;write(writer,"\n\n")?;fenced(&branch,&item.body,cancel,writer,"")?;
                write(writer,&format!("\n\nExit code: {}",exit_code.map(|code|code.to_string()).unwrap_or_else(||"unknown".into())))?;
                if cancelled {write(writer,"\n\nCommand cancelled.")?;}if truncated {write(writer,"\n\nOutput truncated.")?;}
            },
            Event::Command {..}|Event::Wake {..}=>unreachable!(),
        }
    }
    if blocks==0 {write(writer,"(Transcript is empty)")?;}write(writer,"\n")?;Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn streamed_export_retains_full_long_input_output_and_message_order() {
        let temp=super::super::tests::Temp::new();let long="雪😀 end-of-original\n".repeat(3000);
        let mut raw=String::new();
        for value in [json!({"type":"response_item","payload":{"type":"message","role":"user","id":"user","content":[{"type":"input_text","text":"Question"}]}}),
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"call","name":"Write","arguments":json!({"content":long,"lateNumber":719}).to_string()}}),
            json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call","output":"complete output"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":"assistant","content":[{"type":"output_text","text":"Answer"}]}})] {raw.push_str(&value.to_string());raw.push('\n');}
        let branch=super::super::tests::indexed(&temp,&raw);let mut output=Vec::new();
        write_markdown(branch,"Fixture",None,&AtomicBool::new(false),&mut output).unwrap();let output=String::from_utf8(output).unwrap();
        assert!(output.contains("719"));assert_eq!(output.matches("end-of-original").count(),3000);
        assert!(output.find("Question").unwrap()<output.find("Tool call: Write").unwrap());
        assert!(output.find("Tool result (Write)").unwrap()<output.find("Answer").unwrap());
    }

    #[test]
    fn small_codex_export_matches_the_existing_document_exactly() {
        let temp=super::super::tests::Temp::new();let mut raw=String::new();
        for value in [json!({"type":"response_item","timestamp":"2026-10-09T00:00:00Z","payload":{"type":"message","role":"user","id":"user","content":[{"type":"input_text","text":" Question "}]}}),
            json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"text":"First\n\nSecond"}]}}),
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"call","name":"Write","arguments":"{\"content\":\"small\",\"path\":\"file\"}"}}),
            json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call","output":"result\n\n"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":"assistant","content":[{"type":"output_text","text":" Answer "}]}})] {raw.push_str(&value.to_string());raw.push('\n');}
        let branch=super::super::tests::indexed(&temp,&raw);let path=branch.sources[0].segment.path.display().to_string();let mut output=Vec::new();
        write_markdown(branch,"Fixture",Some("2026-10-09 10:00"),&AtomicBool::new(false),&mut output).unwrap();
        let expected=crate::agent::export::render(&crate::agent::export::codex_events(&raw),"Fixture","codex","thread",&path,Some("2026-10-09 10:00"));
        assert_eq!(String::from_utf8(output).unwrap(),expected);
    }
}
