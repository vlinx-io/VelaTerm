use super::*;
use crate::models::SessionKind;
fn fixture() -> (AppCtx, Arc<ChatProcess>) {
    let app = super::super::super::tests::ctx("zcode-v4-projection");
    let proc = super::super::super::tests::inert_process(SessionKind::Zcode);
    *proc.agent_session_id.lock().unwrap() = Some("root".into());
    proc.zcode
        .lock()
        .unwrap()
        .projection
        .topics
        .insert("conversation/root".into(), Topic::default());
    (app, proc)
}
fn frame(topic: &str, seq: u64, payload: Value) -> Value {
    json!({"kind":"complete","topic":topic,"subscriptionId":topic,"logicalFrameId":format!("frame-{seq}"),"frame":{"topic":topic,"subscriptionId":topic,"logEpoch":"epoch","fromSeq":0,"toSeq":seq,"payload":payload}})
}
#[test]
fn guide_fallback_is_reported_once_even_when_the_queue_drains_in_the_same_frame() {
    use std::sync::Mutex;
    let (app, proc) = fixture();
    let events = Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured = events.clone();
    app.listen("chat://event/s", move |payload| captured.lock().unwrap().push(serde_json::from_str(payload).unwrap()));
    let queue = json!({"items":[{"queueItemId":"guide-1","sourceCommandId":"velaterm-guide:input-1","delivery":{"requested":"guide","admitted":"queue","fallbackReasonCode":"guide.noToolBoundary"}}]});
    handle(&app,"s",&proc,&frame("conversation/root",1,json!({"kind":"snapshot","snapshot":{"rows":{"window":[]}}})));
    let mut delta=frame("conversation/root",2,json!({"kind":"deltas","deltas":[{"op":"state.updated","patch":{"queue":queue}},{"op":"state.updated","patch":{"queue":{"items":[]}}}]}));
    delta["frame"]["fromSeq"]=json!(1);
    handle(&app,"s",&proc,&delta);
    handle(&app,"s",&proc,&frame("conversation/root",3,json!({"kind":"snapshot","snapshot":{"rows":{"window":[]},"queue":queue}})));
    let emitted=events.lock().unwrap();
    let fallbacks=emitted.iter().filter(|e|e["type"]=="steerFallback").collect::<Vec<_>>();
    assert_eq!(fallbacks.len(),1);
    assert_eq!(fallbacks[0]["reason"],"guide.noToolBoundary");
}
#[test]
fn native_v4_context_does_not_reset_restored_token_totals() {
    let (app, proc) = fixture();
    super::super::apply_snapshot(
        &app,
        "s",
        &proc,
        &json!({"messages":[{"info":{"role":"assistant","messageId":"persisted","tokens":{"input":320,"output":20,"cache":{"read":80}}},"parts":[]}]}),
        true,
    );
    let wire = frame(
        "conversation/root",
        1,
        json!({"kind":"snapshot","snapshot":{"usage":{"contextWindow":null,"cumulative":{"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0}},"rows":{"window":[]}}}),
    );
    for _ in 0..2 {
        handle(&app, "s", &proc, &wire);
    }
    let extras = proc.extras.lock().unwrap();
    assert_eq!(extras.native_usage.as_ref().unwrap()["inputTokens"], 320);
    assert_eq!(extras.native_usage.as_ref().unwrap()["cachedInputTokens"], 80);
    assert_eq!(extras.context_window, None);
    assert_eq!(extras.total_cost_usd, None);
}
#[test]
fn child_projection_is_owned_scoped_and_kept_when_parent_tool_finishes() {
    let (app, proc) = fixture();
    let parent = json!({"method":"session/event","params":{"sessionId":"root","seq":1,"type":"tool.updated","payload":{"kind":"started","toolCallId":"call","toolName":"Agent","input":{"description":"native child"}}}});
    handle(
        &app,
        "s",
        &proc,
        &frame(
            "conversation/foreign",
            1,
            json!({"kind":"snapshot","snapshot":{"usage":{"cumulative":{"inputTokens":999}}}}),
        ),
    );
    assert!(proc.extras.lock().unwrap().native_usage.is_none());
    handle(
        &app,
        "s",
        &proc,
        &frame(
            "conversation/root",
            1,
            json!({"kind":"snapshot","snapshot":{"rows":{"window":[{"rowId":1,"kind":"subagent","childSessionId":"child","parentToolCallId":"call","subagentType":"general-purpose"}]}}}),
        ),
    );
    handle(
        &app,
        "s",
        &proc,
        &frame(
            "conversation/child",
            1,
            json!({"kind":"snapshot","snapshot":{"config":{"provider":"p","model":"child-model"},"rows":{"window":[{"rowId":0,"kind":"turnHeader","state":"completedSuccess"},{"rowId":1,"kind":"toolCall","toolCallId":"call","toolName":"Read","status":"success","input":{"file_path":"proof.txt"},"output":{"text":"PROOF"}}]}}}),
        ),
    );
    super::super::handle_line(&app, "s", &proc, &parent.to_string());
    let done = json!({"method":"session/event","params":{"sessionId":"root","seq":2,"type":"tool.updated","payload":{"kind":"result","toolCallId":"call","result":{"content":"DONE"}}}});
    super::super::handle_line(&app, "s", &proc, &done.to_string());
    let timeline = proc.timeline.lock().unwrap();
    assert_eq!(timeline.rows.len(), 1);
    assert!(
        matches!(&timeline.rows[0],ChatRow::Tool{status,subagent:Some(info),children,..} if *status=="completed"&&info.model.as_deref()==Some("p/child-model")&&matches!(&children[0],ChatRow::Tool{id,name,output,..} if id!="call"&&name=="Read"&&output.as_deref()==Some("PROOF")))
    );
}
#[test]
fn fragmented_native_snapshot_is_checked_and_reassembled_before_publication() {
    use base64::Engine;
    assert_eq!(crc32(b"123456789"), "cbf43926");
    let (app, proc) = fixture();
    let wire = frame(
        "conversation/root",
        1,
        json!({"kind":"snapshot","snapshot":{"usage":{"contextWindow":{"usedTokens":42,"maxTokens":128000}},"rows":{"window":[]}}}),
    );
    let bytes = serde_json::to_vec(&wire["frame"]).unwrap();
    let cut = bytes.len() / 2;
    for index in [1, 0] {
        let chunk = if index == 0 { &bytes[..cut] } else { &bytes[cut..] };
        let fragment = json!({"kind":"fragment","topic":"conversation/root","subscriptionId":"conversation/root","logicalFrameId":"fragments","fragmentIndex":index,"fragmentCount":2,"logicalBytes":bytes.len(),"checksum":{"algorithm":"crc32","value":crc32(&bytes)},"dataBase64":base64::engine::general_purpose::STANDARD.encode(chunk)});
        handle(&app, "s", &proc, &fragment);
        if index == 1 {
            assert!(proc.extras.lock().unwrap().native_usage.is_none());
        }
    }
    assert_eq!(proc.extras.lock().unwrap().context_tokens, Some(42));
}
