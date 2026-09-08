//! The Codex app-server wire format used by the chat engine.
//!
//! Codex speaks newline-delimited JSON-RPC over stdio. This module deliberately owns only the wire:
//! process lifecycle and timeline projection stay in `engine`, while model discovery stays in
//! `codex_models`. Keeping the JSON here makes protocol changes reviewable without mixing them with UI
//! state.

use serde::Serialize;
use serde_json::{json, Value};

use super::protocol::ChatImage;

/// One app-server line, reduced to the three JSON-RPC shapes the engine routes.
pub enum Incoming {
    Response {
        request_id: String,
        result: Value,
        error: Option<String>,
    },
    Request {
        request_id: Value,
        method: String,
        params: Value,
    },
    Notification { method: String, params: Value },
    Other,
}

/// One collaboration preset reported by the installed Codex app-server.
///
/// The server owns this catalogue because both the modes and their availability are experimental and can
/// change with the installed CLI. The frontend localizes the two protocol-defined mode names, while `name`
/// remains a forward-compatible fallback for a preset this build does not yet know.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationModePreset {
    pub name: String,
    pub mode: String,
    /// A concrete effort supplied by the preset. Codex currently uses `medium` for Plan and leaves
    /// Default unset; an explicit effort selected in VelaTerm still takes precedence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// What the preset is for, in the provider's own words. OpenCode agents describe themselves; Codex
    /// modes are described by the view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Stable map key for a JSON-RPC id while preserving the original value for the response.
pub fn request_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

pub fn parse_line(line: &str) -> Incoming {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Incoming::Other;
    };
    let id = value.get("id").cloned();
    let method = value.get("method").and_then(Value::as_str).map(str::to_string);
    match (id, method) {
        (Some(request_id), Some(method)) => Incoming::Request {
            request_id,
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        },
        (Some(request_id), None) => {
            let error = value.get("error").map(error_text);
            Incoming::Response {
                request_id: request_key(&request_id),
                result: value.get("result").cloned().unwrap_or(Value::Null),
                error,
            }
        }
        (None, Some(method)) => Incoming::Notification {
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        },
        _ => Incoming::Other,
    }
}

fn error_text(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| error.to_string())
}

pub fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"id":id,"method":method,"params":params})
}

pub fn response(id: Value, result: Value) -> Value {
    json!({"id":id,"result":result})
}

pub fn initialized() -> Value {
    json!({"method":"initialized","params":{}})
}

/// Ask the installed app-server which collaboration presets it supports.
pub fn collaboration_mode_list(id: &str) -> Value {
    request(id, "collaborationMode/list", json!({}))
}

pub fn parse_collaboration_modes(result: &Value) -> Vec<CollaborationModePreset> {
    result
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let mode = entry.get("mode").and_then(Value::as_str)?;
            if !matches!(mode, "default" | "plan") {
                return None;
            }
            Some(CollaborationModePreset {
                name: entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(mode)
                    .to_string(),
                mode: mode.to_string(),
                reasoning_effort: entry
                    .get("reasoning_effort")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                description: None,
            })
        })
        .collect()
}

pub fn initialize(id: &str) -> Value {
    request(
        id,
        "initialize",
        json!({
            // Codex reserves this client name for daemon-style integrations that should not become the
            // model-request originator shown in account usage. Paseo uses the same identity.
            "clientInfo":{"name":"codex_app_server_daemon","title":"VelaTerm","version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"experimentalApi":true}
        }),
    )
}

/// Start or resume the native thread after the initialize handshake.
pub fn open_thread(
    id: &str,
    resume: Option<&str>,
    cwd: Option<&str>,
    model: Option<&str>,
    mode: &str,
    service_tier: Option<&str>,
    personality: Option<&str>,
) -> Value {
    let mut params = serde_json::Map::new();
    if let Some(thread_id) = resume {
        params.insert("threadId".into(), json!(thread_id));
        // The timeline is restored from local recordings by history::replay. Request only metadata
        // here to avoid the deprecated full-history response for paginated threads.
        params.insert("excludeTurns".into(), json!(true));
    }
    if let Some(cwd) = cwd {
        params.insert("cwd".into(), json!(cwd));
    }
    if let Some(model) = model {
        params.insert("model".into(), json!(model));
    }
    if let Some(tier) = service_tier {
        params.insert("serviceTier".into(), json!(tier));
    }
    if let Some(personality) = personality {
        params.insert("personality".into(), json!(personality));
    }
    let (approval, sandbox) = workflow(mode);
    params.insert("approvalPolicy".into(), json!(approval));
    params.insert("sandbox".into(), json!(sandbox));
    request(
        id,
        if resume.is_some() { "thread/resume" } else { "thread/start" },
        Value::Object(params),
    )
}

/// Everything a turn carries besides its text: the model, how hard it thinks, how fast it answers, how it
/// speaks, and the permission and collaboration policies in force.
#[derive(Clone, Copy, Default)]
pub struct TurnOptions<'a> {
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub mode: &'a str,
    pub collaboration_mode: Option<&'a str>,
    pub collaboration_preset_effort: Option<&'a str>,
    /// A service tier id from the model catalogue, such as `fast`; None keeps the thread's tier.
    pub service_tier: Option<&'a str>,
    /// `none`, `friendly` or `pragmatic`; None keeps the configured personality.
    pub personality: Option<&'a str>,
}

pub fn turn_start(
    id: &str,
    thread_id: &str,
    text: &str,
    images: &[ChatImage],
    options: TurnOptions<'_>,
) -> Value {
    let TurnOptions {
        model,
        effort,
        mode,
        collaboration_mode,
        collaboration_preset_effort,
        service_tier,
        personality,
    } = options;
    // Codex accepts an omitted effort but rejects empty values, including in collaboration settings.
    let effort = effort.map(str::trim).filter(|value| !value.is_empty());
    let collaboration_preset_effort = collaboration_preset_effort
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut params = turn_params(thread_id, text, images);
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    if let Some(effort) = effort {
        params["effort"] = json!(effort);
    }
    if let Some(tier) = service_tier {
        params["serviceTier"] = json!(tier);
    }
    if let Some(personality) = personality {
        params["personality"] = json!(personality);
    }
    let (approval, sandbox) = workflow(mode);
    params["approvalPolicy"] = json!(approval);
    params["sandboxPolicy"] = sandbox_policy(sandbox);
    if let (Some(collaboration_mode), Some(model)) = (collaboration_mode, model) {
        if matches!(collaboration_mode, "default" | "plan") {
            let mut settings = json!({
                "model":model,
                // Null deliberately selects Codex's built-in instructions for this mode.
                "developer_instructions":Value::Null
            });
            // A deliberate effort choice wins; otherwise apply the installed preset's recommendation
            // (currently medium for Plan). Default has no preset override and keeps the native default.
            if let Some(effort) = effort.or(collaboration_preset_effort) {
                settings["reasoning_effort"] = json!(effort);
            }
            params["collaborationMode"] = json!({
                "mode":collaboration_mode,
                "settings":settings
            });
        }
    }
    request(id, "turn/start", params)
}

pub fn turn_steer(
    id: &str,
    thread_id: &str,
    turn_id: &str,
    text: &str,
    images: &[ChatImage],
) -> Value {
    let input = user_input(text, images);
    request(
        id,
        "turn/steer",
        json!({"threadId":thread_id,"expectedTurnId":turn_id,"input":input}),
    )
}

pub fn turn_interrupt(id: &str, thread_id: &str, turn_id: &str) -> Value {
    request(id, "turn/interrupt", json!({"threadId":thread_id,"turnId":turn_id}))
}

/// Fork a thread immediately before one native turn. The source stays untouched and recoverable.
pub fn thread_fork(
    id: &str,
    thread_id: &str,
    before_turn_id: Option<&str>,
    cwd: Option<&str>,
    model: Option<&str>,
) -> Value {
    let mut params = json!({
        "threadId":thread_id,
        "cwd":cwd,
        "model":model,
        "serviceTier":Value::Null,
        // Rewind uses the returned thread id; its visible history comes from the local timeline.
        "excludeTurns":true,
        "persistExtendedHistory":true,
        "deferGoalContinuation":true,
    });
    if let Some(turn_id) = before_turn_id {
        params["beforeTurnId"] = json!(turn_id);
    }
    request(id, "thread/fork", params)
}

pub fn thread_read_metadata(id: &str, thread_id: &str) -> Value {
    request(id, "thread/read", json!({"threadId":thread_id,"includeTurns":false}))
}

pub fn thread_rollback(id: &str, thread_id: &str, num_turns: usize) -> Value {
    request(id, "thread/rollback", json!({"threadId":thread_id,"numTurns":num_turns}))
}

/// Ask Codex to summarize the thread now rather than when the context window fills up.
pub fn thread_compact_start(id: &str, thread_id: &str) -> Value {
    request(id, "thread/compact/start", json!({"threadId":thread_id}))
}

/// Run Codex's code review as a turn of this thread, so its findings land in the conversation.
pub fn review_start(id: &str, thread_id: &str, target: Value) -> Value {
    request(
        id,
        "review/start",
        json!({"threadId":thread_id,"target":target,"delivery":"inline"}),
    )
}

/// What `/review` was asked to look at, from the words after the command.
///
/// Nothing means the working tree, as it does in Codex's own interface. `branch <name>` and `commit
/// <sha>` name the other two targets the protocol knows; any other text is free-form instructions.
pub fn review_target(args: &str) -> Value {
    let args = args.trim();
    if args.is_empty() {
        return json!({"type":"uncommittedChanges"});
    }
    let mut words = args.splitn(2, char::is_whitespace);
    let head = words.next().unwrap_or("");
    let rest = words.next().map(str::trim).unwrap_or("");
    match (head, rest.is_empty()) {
        ("branch", false) => json!({"type":"baseBranch","branch":rest}),
        ("commit", false) => json!({"type":"commit","sha":rest,"title":Value::Null}),
        _ => json!({"type":"custom","instructions":args}),
    }
}

fn turn_params(thread_id: &str, text: &str, images: &[ChatImage]) -> Value {
    json!({"threadId":thread_id,"input":user_input(text, images)})
}

fn user_input(text: &str, images: &[ChatImage]) -> Vec<Value> {
    let mut input = Vec::new();
    if !text.is_empty() {
        input.push(json!({"type":"text","text":text,"text_elements":[]}));
    }
    for image in images {
        input.push(json!({
            "type":"image",
            "url":format!("data:{};base64,{}", image.mime_type, image.data)
        }));
    }
    input
}

/// The three Codex modes exposed by Paseo, translated to app-server policy fields.
pub fn workflow(mode: &str) -> (&'static str, &'static str) {
    match mode {
        "read-only" => ("on-request", "read-only"),
        "full-access" | "skip" => ("never", "danger-full-access"),
        _ => ("on-request", "workspace-write"),
    }
}

fn sandbox_policy(sandbox: &str) -> Value {
    match sandbox {
        "read-only" => json!({"type":"readOnly"}),
        "danger-full-access" => json!({"type":"dangerFullAccess"}),
        _ => json!({
            "type":"workspaceWrite",
            "writableRoots":[],
            "networkAccess":false,
            "excludeTmpdirEnvVar":false,
            "excludeSlashTmp":false
        }),
    }
}

/// The standing rule a permission answer adopted, if any, read from the card's `updatedPermissions`.
///
/// The card hands back the suggestion object it was given, so the type names here are the ones the
/// engine put on the request: `codexAcceptForSession`, `codexExecpolicyAmendment` and
/// `codexNetworkPolicyAmendment`. The first found wins; a card offers one button per rule.
fn adopted_rule<'a>(updated_permissions: Option<&'a Value>) -> Option<&'a Value> {
    updated_permissions?
        .as_array()?
        .iter()
        .find(|value| {
            matches!(
                value.get("type").and_then(Value::as_str),
                Some("codexAcceptForSession" | "codexExecpolicyAmendment" | "codexNetworkPolicyAmendment")
            )
        })
}

/// Translate the generic permission card's answer into the native server-request response.
///
/// `message` is the card's refusal note; the one value that matters here is `cancel`, which an
/// elicitation card sends to close the request without answering it either way.
pub fn permission_response(
    request: &Value,
    allow: bool,
    updated_input: Option<&Value>,
    updated_permissions: Option<&Value>,
    message: Option<&str>,
) -> Value {
    let id = request.get("_codexRequestId").cloned().unwrap_or(Value::Null);
    let method = request
        .get("_codexMethod")
        .and_then(Value::as_str)
        .unwrap_or("");
    let rule = adopted_rule(updated_permissions);
    let rule_type = rule.and_then(|value| value.get("type")).and_then(Value::as_str);
    let remember = rule_type == Some("codexAcceptForSession");
    let result = match method {
        value if value.ends_with("requestUserInput") => {
            question_response(request, allow, updated_input)
        }
        "mcpServer/elicitation/request" => {
            let action = if allow {
                "accept"
            } else if message == Some("cancel") {
                "cancel"
            } else {
                "decline"
            };
            json!({
                "action":action,
                // The card's filled-in form travels as the tool's input; an empty form is still consent.
                "content":if allow { updated_input.cloned().unwrap_or_else(|| json!({})) } else { Value::Null },
                "_meta":Value::Null
            })
        }
        "item/permissions/requestApproval" => json!({
            "permissions":if allow {
                request.pointer("/_codexParams/permissions").cloned().unwrap_or_else(|| json!({}))
            } else {
                json!({})
            },
            "scope":if remember { "session" } else { "turn" }
        }),
        "item/commandExecution/requestApproval" => {
            let decision = if !allow {
                json!("decline")
            } else {
                match (rule_type, rule.and_then(|value| value.get("amendment"))) {
                    (Some("codexExecpolicyAmendment"), Some(amendment)) => {
                        json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":amendment}})
                    }
                    (Some("codexNetworkPolicyAmendment"), Some(amendment)) => {
                        json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":amendment}})
                    }
                    _ if remember => json!("acceptForSession"),
                    _ => json!("accept"),
                }
            };
            json!({"decision":decision})
        }
        _ => json!({
            "decision":if allow { if remember { "acceptForSession" } else { "accept" } } else { "decline" }
        }),
    };
    response(id, result)
}

fn question_response(request: &Value, allow: bool, updated_input: Option<&Value>) -> Value {
    let mut answers = serde_json::Map::new();
    let given = updated_input
        .and_then(|input| input.get("answers"))
        .and_then(Value::as_object);
    if allow {
        if let Some(questions) = request
            .get("_codexQuestions")
            .and_then(Value::as_array)
        {
            for question in questions {
                let Some(id) = question.get("id").and_then(Value::as_str) else { continue };
                let text = question.get("question").and_then(Value::as_str).unwrap_or("");
                let answer = given.and_then(|map| map.get(text)).and_then(Value::as_str).unwrap_or("");
                answers.insert(id.to_string(), json!({"answers":[answer]}));
            }
        }
    }
    json!({"answers":answers})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_carries_text_images_model_effort_and_safe_policy() {
        let value = turn_start(
            "1",
            "thread-1",
            "look",
            &[ChatImage { mime_type: "image/png".into(), data: "AAAA".into() }],
            TurnOptions {
                model: Some("gpt-5.6-terra"),
                effort: Some("high"),
                mode: "auto",
                collaboration_mode: Some("plan"),
                collaboration_preset_effort: Some("medium"),
                service_tier: Some("fast"),
                personality: Some("pragmatic"),
            },
        );
        assert_eq!(value["method"], "turn/start");
        assert_eq!(value["params"]["serviceTier"], "fast");
        assert_eq!(value["params"]["personality"], "pragmatic");
        assert_eq!(value["params"]["input"][0]["text"], "look");
        assert_eq!(value["params"]["input"][1]["url"], "data:image/png;base64,AAAA");
        assert_eq!(value["params"]["model"], "gpt-5.6-terra");
        assert_eq!(value["params"]["effort"], "high");
        assert_eq!(value["params"]["approvalPolicy"], "on-request");
        assert_eq!(value["params"]["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(value["params"]["collaborationMode"]["mode"], "plan");
        assert_eq!(
            value["params"]["collaborationMode"]["settings"],
            json!({
                "model":"gpt-5.6-terra",
                "developer_instructions":Value::Null,
                "reasoning_effort":"high"
            })
        );
    }

    #[test]
    fn collaboration_catalogue_keeps_supported_native_presets() {
        let modes = parse_collaboration_modes(&json!({"data":[
            {"name":"Plan","mode":"plan","reasoning_effort":"medium"},
            {"name":"Default","mode":"default","reasoning_effort":null},
            {"name":"Future","mode":"future"},
            {"name":"Incomplete","mode":null}
        ]}));
        assert_eq!(modes.len(), 2);
        assert_eq!(modes[0].mode, "plan");
        assert_eq!(modes[0].reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(modes[1].name, "Default");
        assert_eq!(modes[1].reasoning_effort, None);
    }

    #[test]
    fn plan_uses_its_native_effort_when_no_explicit_effort_was_selected() {
        let value = turn_start(
            "1",
            "thread-1",
            "plan this",
            &[],
            TurnOptions {
                model: Some("gpt-5.6-terra"),
                mode: "read-only",
                collaboration_mode: Some("plan"),
                collaboration_preset_effort: Some("medium"),
                ..TurnOptions::default()
            },
        );
        assert!(value["params"].get("serviceTier").is_none());
        assert!(value["params"].get("personality").is_none());
        assert_eq!(
            value["params"]["collaborationMode"]["settings"]["reasoning_effort"],
            "medium"
        );
    }

    #[test]
    fn parses_responses_requests_and_notifications() {
        assert!(matches!(
            parse_line(r#"{"id":"a","result":{"ok":true}}"#),
            Incoming::Response { request_id, .. } if request_id == "a"
        ));
        assert!(matches!(
            parse_line(r#"{"id":7,"method":"item/fileChange/requestApproval","params":{}}"#),
            Incoming::Request { request_id: Value::Number(_), method, .. } if method.ends_with("requestApproval")
        ));
        assert!(matches!(
            parse_line(r#"{"method":"turn/started","params":{}}"#),
            Incoming::Notification { method, .. } if method == "turn/started"
        ));
    }

    #[test]
    fn automatic_effort_never_sends_empty_reasoning_values() {
        for effort in [None, Some(""), Some(" \t ")] {
            for (mode, preset, expected) in [
                ("default", None, None),
                ("default", Some(""), None),
                ("default", Some(" \t "), None),
                ("plan", Some("medium"), Some("medium")),
            ] {
                let value = turn_start(
                    "1", "thread-1", "continue", &[],
                    TurnOptions {
                        model: Some("gpt-5.6-luna"),
                        effort,
                        collaboration_mode: Some(mode),
                        collaboration_preset_effort: preset,
                        ..TurnOptions::default()
                    },
                );
                assert!(value["params"].get("effort").is_none());
                assert_eq!(value["params"]["model"], "gpt-5.6-luna");
                assert_eq!(value["params"]["collaborationMode"]["mode"], mode);
                let settings = &value["params"]["collaborationMode"]["settings"];
                assert_eq!(settings.get("reasoning_effort"), expected.map(|e| json!(e)).as_ref());
            }
        }
    }

    #[test]
    fn question_answers_are_keyed_by_native_question_id() {
        let request = json!({
            "_codexRequestId":9,
            "_codexMethod":"item/tool/requestUserInput",
            "_codexQuestions":[{"id":"colour","question":"Which colour?"}]
        });
        let input = json!({"answers":{"Which colour?":"Blue"}});
        let value = permission_response(&request, true, Some(&input), None, None);
        assert_eq!(value["id"], 9);
        assert_eq!(value["result"]["answers"]["colour"]["answers"][0], "Blue");
    }

    fn command_request() -> Value {
        json!({"_codexRequestId":3,"_codexMethod":"item/commandExecution/requestApproval"})
    }

    #[test]
    fn a_plain_command_approval_is_accept_and_a_refusal_is_decline() {
        assert_eq!(permission_response(&command_request(), true, None, None, None)["result"]["decision"], "accept");
        assert_eq!(permission_response(&command_request(), false, None, None, None)["result"]["decision"], "decline");
    }

    #[test]
    fn adopting_the_session_rule_accepts_for_the_session() {
        let rules = json!([{"type":"codexAcceptForSession","destination":"session","subject":"ls"}]);
        let value = permission_response(&command_request(), true, None, Some(&rules), None);
        assert_eq!(value["result"]["decision"], "acceptForSession");
        // A refusal never adopts a rule, whatever button the card thought it pressed.
        let value = permission_response(&command_request(), false, None, Some(&rules), None);
        assert_eq!(value["result"]["decision"], "decline");
    }

    #[test]
    fn adopting_the_execpolicy_amendment_sends_it_back_verbatim() {
        let rules = json!([{
            "type":"codexExecpolicyAmendment",
            "destination":"userSettings",
            "subject":"cargo test",
            "amendment":["cargo","test"]
        }]);
        let value = permission_response(&command_request(), true, None, Some(&rules), None);
        assert_eq!(
            value["result"]["decision"],
            json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo","test"]}})
        );
    }

    #[test]
    fn adopting_the_network_rule_applies_the_amendment() {
        let rules = json!([{
            "type":"codexNetworkPolicyAmendment",
            "destination":"userSettings",
            "subject":"crates.io",
            "amendment":{"action":"allow","host":"crates.io"}
        }]);
        let value = permission_response(&command_request(), true, None, Some(&rules), None);
        assert_eq!(
            value["result"]["decision"],
            json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"action":"allow","host":"crates.io"}}})
        );
    }

    #[test]
    fn an_elicitation_form_travels_as_content_and_cancel_is_its_own_action() {
        let request = json!({"_codexRequestId":4,"_codexMethod":"mcpServer/elicitation/request"});
        let content = json!({"name":"Ada","confirm":true});
        let value = permission_response(&request, true, Some(&content), None, None);
        assert_eq!(value["result"]["action"], "accept");
        assert_eq!(value["result"]["content"], content);
        let value = permission_response(&request, false, None, None, Some("cancel"));
        assert_eq!(value["result"]["action"], "cancel");
        assert!(value["result"]["content"].is_null());
        let value = permission_response(&request, false, None, None, None);
        assert_eq!(value["result"]["action"], "decline");
    }

    #[test]
    fn review_targets_follow_the_words_after_the_command() {
        assert_eq!(review_target(""), json!({"type":"uncommittedChanges"}));
        assert_eq!(review_target("branch main"), json!({"type":"baseBranch","branch":"main"}));
        assert_eq!(review_target("commit abc123")["sha"], "abc123");
        assert_eq!(
            review_target("focus on error handling"),
            json!({"type":"custom","instructions":"focus on error handling"})
        );
        // A bare keyword has nothing to name, so it reads as instructions rather than an empty target.
        assert_eq!(review_target("branch")["type"], "custom");
    }

    #[test]
    fn paginated_rewind_forks_immediately_before_the_target_turn() {
        let value = thread_fork(
            "9",
            "thread-old",
            Some("turn-target"),
            Some("/work"),
            Some("gpt-5.6-terra"),
        );
        assert_eq!(value["method"], "thread/fork");
        assert_eq!(value["params"]["threadId"], "thread-old");
        assert_eq!(value["params"]["beforeTurnId"], "turn-target");
        assert_eq!(value["params"]["excludeTurns"], true);
        assert_eq!(value["params"]["persistExtendedHistory"], true);
    }

    #[test]
    fn legacy_rewind_forks_the_full_thread_then_rolls_back_a_turn_count() {
        let fork = thread_fork("10", "thread-old", None, None, None);
        assert_eq!(fork["params"]["excludeTurns"], true);
        assert!(fork["params"].get("beforeTurnId").is_none());
        let rollback = thread_rollback("11", "thread-fork", 3);
        assert_eq!(rollback["method"], "thread/rollback");
        assert_eq!(rollback["params"], json!({"threadId":"thread-fork","numTurns":3}));
    }
}
