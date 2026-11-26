//! `vapp`: lets the agent in any session operate VelaTerm itself. The agent reads what the app can do and
//! its current state, then submits actions; by default the session's view shows them for confirmation, while
//! `--yes` (`-y`, `--no-confirm`) runs validated actions directly (except folder choices).

use std::io::{IsTerminal, Read};
use std::time::{Duration, Instant};
use serde_json::{json, Value};
use crate::host::AppCtx;

const USAGE: &str = "usage: vapp catalog [area]
       vapp session list [project] [--json]
       vapp session create-group <project> [--parent-group <group>] <name>
       vapp session rename <item> <new name>
       vapp session move <item> [--group <group> | --root] [--parent-session <session>] [--project <project>] [--order N]
       vapp session sort <group|project|session> [--by name | --order <id,id,...>]
       vapp session kill <session>
       vapp do [JSON]
       vapp status <id>
catalog  Print the actions VelaTerm can perform and the current state (collections, projects, vaults...).
session  List or reorganize a project's groups and sessions. Each change command submits one action.
do       Submit a JSON array of actions (as an argument or on stdin) as one request.
status   Print the outcome of an earlier request.
Areas: projects, files, knowledge, settings.
Items are named by id, an id prefix of at least 6 characters, or a unique exact name.
By default, actions wait for the user to confirm them in the session's view in VelaTerm. Options for submitting:
  --yes, -y, --no-confirm   run validated actions without waiting for confirmation (folder choices still ask)
  --summary TEXT            shown to the user above the changes
  --no-wait                 return at once instead of waiting for the decision
  --timeout SECS            how long to wait (default 900)";

/// Default wait for the user's decision; past it the command reports that the request is still pending.
const WAIT: Duration = Duration::from_secs(900);

pub fn handle(app: AppCtx, mut request: crate::diagnostics::HttpRequest, token: String) {
    let authorized = request.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));
    let mut body = String::new();
    let result = if !authorized {
        Err((401, "Authentication required".to_owned()))
    } else if request.method() != &tiny_http::Method::Post {
        Err((405, "Use POST".to_owned()))
    } else if request.as_reader().take(524_289).read_to_string(&mut body).is_err() || body.len() > 524_288 {
        Err((413, "Request too large".to_owned()))
    } else {
        serde_json::from_str::<Value>(&body).map_err(|e| e.to_string()).and_then(|args| dispatch(&app, &args)).map_err(|e| (400, e))
    };
    let (status, value) = match result {
        Ok(result) => (200, json!({"result": result})),
        Err((status, error)) => (status, json!({"error": error})),
    };
    let _ = request.respond(tiny_http::Response::from_string(value.to_string()).with_status_code(status)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()));
}

fn dispatch(app: &AppCtx, args: &Value) -> Result<Value, String> {
    match args["op"].as_str() {
        Some("catalog") => crate::assist::catalog(app, args["area"].as_str()),
        Some("list") => crate::assist::list(app, args["project"].as_str(), args["sessionId"].as_str().unwrap_or_default()),
        Some("submit") => crate::assist::submit(
            app,
            args["sessionId"].as_str().unwrap_or_default(),
            args["summary"].as_str().unwrap_or_default(),
            &args["actions"],
            args["noConfirm"].as_bool().unwrap_or(false),
        ),
        Some("status") => crate::assist::outcome(app, args["id"].as_str().ok_or("id is required")?),
        _ => Err("Unknown vapp operation".into()),
    }
}

fn parse_actions(text: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text.trim()).map_err(|e| format!("Actions must be JSON: {e}"))?;
    // A single action object is accepted as a one-item list.
    Ok(if value.is_object() { json!([value]) } else { value })
}

fn print(value: &Value) { println!("{}", serde_json::to_string_pretty(value).unwrap_or_default()); }

/// Options shared by every command that submits changes.
struct Submit { summary: String, wait: bool, no_confirm: bool, timeout: Duration }

/// Splits `--flag value` (or `--flag=value`) options from positional arguments. `flags` lists options without a value.
fn split(args: &[String], flags: &[&str]) -> Result<(Vec<String>, Vec<(String, Option<String>)>), String> {
    let (mut positional, mut options) = (Vec::new(), Vec::new());
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "-y" && flags.contains(&"yes") {
            options.push(("yes".to_string(), None));
            continue;
        }
        match arg.strip_prefix("--") {
            Some(name) if flags.contains(&name) => options.push((name.to_string(), None)),
            Some(name) if !name.is_empty() => {
                if let Some((key, value)) = name.split_once('=') {
                    if flags.contains(&key) { return Err(format!("--{key} does not take a value")); }
                    options.push((key.to_string(), Some(value.to_string())));
                } else {
                    options.push((name.to_string(), Some(rest.next().ok_or(format!("--{name} needs a value"))?.clone())));
                }
            }
            _ => positional.push(arg.clone()),
        }
    }
    Ok((positional, options))
}

fn submit_options(options: &mut Vec<(String, Option<String>)>) -> Result<Submit, String> {
    let mut submit = Submit { summary: String::new(), wait: true, no_confirm: false, timeout: WAIT };
    let mut kept = Vec::new();
    for (name, value) in options.drain(..) {
        match name.as_str() {
            "summary" => submit.summary = value.unwrap_or_default(),
            "no-wait" => submit.wait = false,
            "yes" | "no-confirm" => submit.no_confirm = true,
            "timeout" => submit.timeout = Duration::from_secs(value.and_then(|v| v.parse().ok()).ok_or("--timeout needs seconds")?),
            _ => kept.push((name, value)),
        }
    }
    *options = kept;
    Ok(submit)
}

fn post(body: Value) -> Result<Value, String> { super::tell::post_local("app", &body) }

/// Submits actions and, unless already settled or told not to wait, waits for them to finish. Returns the exit code.
fn submit(actions: Value, options: Submit) -> Result<i32, String> {
    let session = std::env::var("VLX_SESSION_ID").map_err(|_| "Run vapp inside a VelaTerm session")?;
    let mut body = json!({"op": "submit", "sessionId": session, "summary": options.summary, "actions": actions});
    if options.no_confirm { body["noConfirm"] = json!(true); }
    let submitted = post(body)?;
    if submitted["accepted"] == false {
        print(&submitted);
        eprintln!("vapp: nothing was submitted; fix the listed actions and submit the whole request again");
        return Ok(1);
    }
    if submitted["settled"] == true {
        print(&submitted);
        return Ok(0);
    }
    let id = submitted["id"].as_str().ok_or("Invalid VelaTerm response")?.to_string();
    if !options.wait {
        eprintln!("vapp: submitted {id}; the user confirms it in this session's view. Check with `vapp status {id}`");
        print(&submitted);
        return Ok(0);
    }
    if options.no_confirm {
        eprintln!("vapp: submitted {id}; waiting for remaining window or choice actions to finish");
    } else {
        eprintln!("vapp: submitted {id}; waiting for the user to confirm it in this session's view");
    }
    let started = Instant::now();
    loop {
        let interval = if options.no_confirm && started.elapsed() < Duration::from_secs(5) {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(1500)
        };
        std::thread::sleep(interval);
        let current = post(json!({"op": "status", "id": id}))?;
        if current["settled"] == true { print(&current); return Ok(0); }
        if started.elapsed() >= options.timeout {
            print(&current);
            eprintln!("vapp: some actions still await the user; check again with `vapp status {id}`");
            return Ok(2);
        }
    }
}

/// Renders `vapp session list` as an indented tree with ids, groups before sessions at each level.
fn render(list: &Value) -> String {
    let project = &list["project"];
    let mut out = format!("{} ({} {})", project["name"].as_str().unwrap_or_default(), project["kind"].as_str().unwrap_or_default(), project["id"].as_str().unwrap_or_default());
    if let Some(path) = project["path"].as_str().filter(|p| !p.is_empty()) { out.push_str(&format!("  {path}")); }
    out.push('\n');
    let empty = Vec::new();
    let groups = list["groups"].as_array().unwrap_or(&empty);
    let sessions = list["sessions"].as_array().unwrap_or(&empty);
    fn sessions_in(out: &mut String, sessions: &[Value], group: &Value, parent: &Value, depth: usize) {
        for s in sessions.iter().filter(|s| &s["groupId"] == group && &s["parentSessionId"] == parent) {
            let running = if s["alive"] == true { " running" } else { "" };
            out.push_str(&format!("{}- {}  [{} {}] order={}{}\n", "  ".repeat(depth), s["name"].as_str().unwrap_or_default(),
                s["kind"].as_str().unwrap_or("session"), s["id"].as_str().unwrap_or_default(), s["sortOrder"], running));
            sessions_in(out, sessions, group, &s["id"], depth + 1);
        }
    }
    fn level(out: &mut String, groups: &[Value], sessions: &[Value], parent: &Value, depth: usize) {
        for g in groups.iter().filter(|g| &g["parentGroupId"] == parent) {
            out.push_str(&format!("{}+ {}  [group {}] order={}\n", "  ".repeat(depth), g["name"].as_str().unwrap_or_default(),
                g["id"].as_str().unwrap_or_default(), g["sortOrder"]));
            level(out, groups, sessions, &g["id"], depth + 1);
        }
        sessions_in(out, sessions, parent, &Value::Null, depth);
    }
    level(&mut out, groups, sessions, &Value::Null, 1);
    out
}

fn session_action(command: &str, positional: &[String], options: &[(String, Option<String>)]) -> Result<Value, String> {
    let option = |name: &str| options.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone().unwrap_or_default());
    let flag = |name: &str| options.iter().any(|(n, _)| n == name);
    let usage = |line: &str| format!("usage: vapp session {line}");
    match command {
        "create-group" => {
            let [project, name] = positional else { return Err(usage("create-group <project> [--parent-group <group>] <name>")) };
            Ok(json!({"kind": "createGroup", "projectId": project, "parentGroupId": option("parent-group"), "name": name}))
        }
        "rename" => {
            let [item, name] = positional else { return Err(usage("rename <item> <new name>")) };
            Ok(json!({"kind": "rename", "id": item, "name": name}))
        }
        "move" => {
            let [item] = positional else { return Err(usage("move <item> [--group <group> | --root] [--parent-session <session>] [--project <project>] [--order N]")) };
            let mut action = json!({"kind": "move", "id": item});
            if let Some(group) = option("group") { action["groupId"] = json!(group); }
            if flag("root") { action["groupId"] = Value::Null; }
            if let Some(parent) = option("parent-session") { action["parentSessionId"] = json!(parent); }
            if let Some(project) = option("project") { action["projectId"] = json!(project); }
            if let Some(order) = option("order") { action["order"] = json!(order.parse::<i64>().map_err(|_| "--order needs a number")?); }
            Ok(action)
        }
        "sort" => {
            let [target] = positional else { return Err(usage("sort <group|project|session> [--by name | --order <id,id,...>]")) };
            let mut action = json!({"kind": "sort", "target": target, "by": option("by").unwrap_or_else(|| "name".into())});
            if let Some(order) = option("order") {
                action["order"] = json!(order.split(',').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>());
                action["by"] = Value::Null;
            }
            Ok(action)
        }
        "kill" | "kill-process" => {
            let [session] = positional else { return Err(usage("kill <session>")) };
            Ok(json!({"kind": "killProcess", "id": session}))
        }
        "" => Err(format!("Missing session command\n{USAGE}")),
        other => Err(format!("Unknown session command: {other}\n{USAGE}")),
    }
}

fn session(args: &[String]) -> Result<i32, String> {
    let command = args.first().map(String::as_str).unwrap_or_default().replace('_', "-");
    let (positional, mut options) = split(&args[args.len().min(1)..], &["json", "root", "no-wait", "yes", "no-confirm"])?;
    let flag = |options: &[(String, Option<String>)], name: &str| options.iter().any(|(n, _)| n == name);
    if command == "list" {
        let session = std::env::var("VLX_SESSION_ID").unwrap_or_default();
        let list = post(json!({"op": "list", "project": positional.first(), "sessionId": session}))?;
        if flag(&options, "json") { print(&list) } else { print!("{}", render(&list)) }
        return Ok(0);
    }
    let submit_with = submit_options(&mut options)?;
    let action = session_action(command.as_str(), positional.as_slice(), &options)?;
    submit(json!([action]), submit_with)
}

fn run(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => { println!("{USAGE}"); Ok(0) }
        Some("catalog") => { print(&post(json!({"op": "catalog", "area": args.get(1)}))?); Ok(0) }
        Some("status") => { print(&post(json!({"op": "status", "id": args.get(1).ok_or("usage: vapp status <id>")?}))?); Ok(0) }
        Some("session") => session(&args[1..]),
        Some("do") => {
            let (positional, mut options) = split(&args[1..], &["no-wait", "yes", "no-confirm"])?;
            let submit_with = submit_options(&mut options)?;
            if let Some((name, _)) = options.first() { return Err(format!("Unknown option: --{name}")); }
            let text = match positional.as_slice() {
                [text] => text.clone(),
                [] => {
                    if std::io::stdin().is_terminal() { return Err("Provide the actions as a JSON argument or on stdin".into()); }
                    let mut text = String::new();
                    std::io::stdin().take(524_289).read_to_string(&mut text).map_err(|_| "Cannot read the actions from stdin")?;
                    text
                }
                _ => return Err("Pass the actions as one JSON argument".into()),
            };
            submit(parse_actions(&text)?, submit_with)
        }
        Some(other) => Err(format!("Unknown command: {other}\n{USAGE}")),
    }
}

pub fn run_cli(args: &[String]) -> ! {
    match run(&args[args.len().min(2)..]) {
        Ok(code) => std::process::exit(code),
        Err(error) => { eprintln!("vapp: {error}"); std::process::exit(1); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_actions_become_lists_and_bad_json_is_explained() {
        assert_eq!(parse_actions(r#"{"domain":"settings","kind":"openSettings"}"#).unwrap(), json!([{"domain":"settings","kind":"openSettings"}]));
        assert_eq!(parse_actions("[]").unwrap(), json!([]));
        assert!(parse_actions("rename it").unwrap_err().starts_with("Actions must be JSON"));
    }

    #[test]
    fn options_split_from_positional_arguments() {
        let args = ["item", "--group", "g1", "--root", "--order", "5"].map(String::from);
        let (positional, options) = split(&args, &["root"]).unwrap();
        assert_eq!(positional, vec!["item"]);
        assert_eq!(options, vec![("group".into(), Some("g1".into())), ("root".into(), None), ("order".into(), Some("5".into()))]);
        assert!(split(&["--group".to_string()], &[]).is_err());
    }

    #[test]
    fn yes_flags_and_inline_values_parse_into_submit_options() {
        for flag in ["--yes", "-y", "--no-confirm"] {
            let args = [flag, "--summary=Auto organize", "--no-wait", "[]"].map(String::from);
            let (positional, mut options) = split(&args, &["no-wait", "yes", "no-confirm"]).unwrap();
            let submit = submit_options(&mut options).unwrap();
            assert_eq!(positional, vec!["[]"]);
            assert!(options.is_empty());
            assert!(submit.no_confirm, "{flag} should set no_confirm");
            assert!(!submit.wait);
            assert_eq!(submit.summary, "Auto organize");
        }
        let (_, mut default_opts) = split(&["[]".to_string()], &["no-wait", "yes", "no-confirm"]).unwrap();
        assert!(!submit_options(&mut default_opts).unwrap().no_confirm);
        assert!(split(&["--yes=true".to_string()], &["yes"]).is_err());
    }

    #[test]
    fn list_renders_groups_before_sessions_with_nesting() {
        let list = json!({
            "project": {"id": "p", "name": "Course", "kind": "project", "path": "/c"},
            "groups": [{"id": "g", "name": "Ancient", "parentGroupId": null, "sortOrder": 1000}],
            "sessions": [
                {"id": "s1", "name": "Lesson 1", "kind": "claude", "groupId": "g", "parentSessionId": null, "sortOrder": 1000},
                {"id": "s2", "name": "Child", "kind": "codex", "groupId": "g", "parentSessionId": "s1", "sortOrder": 1000},
                {"id": "s3", "name": "Loose", "kind": "terminal", "groupId": null, "parentSessionId": null, "sortOrder": 2000},
            ],
        });
        assert_eq!(render(&list), "Course (project p)  /c\n  + Ancient  [group g] order=1000\n    - Lesson 1  [claude s1] order=1000\n      - Child  [codex s2] order=1000\n  - Loose  [terminal s3] order=2000\n");
    }

    #[test]
    fn list_marks_running_sessions_and_session_kill_builds_action() {
        let list = json!({
            "project": {"id": "p", "name": "Course", "kind": "project", "path": ""},
            "groups": [],
            "sessions": [
                {"id": "s1", "name": "Worker", "kind": "claude", "groupId": null, "parentSessionId": null, "sortOrder": 1000, "alive": true},
                {"id": "s2", "name": "Idle", "kind": "terminal", "groupId": null, "parentSessionId": null, "sortOrder": 2000, "alive": false},
            ],
        });
        assert_eq!(render(&list), "Course (project p)\n  - Worker  [claude s1] order=1000 running\n  - Idle  [terminal s2] order=2000\n");
        assert_eq!(session_action("kill", &["s1".into()], &[]).unwrap(), json!({"kind": "killProcess", "id": "s1"}));
        assert_eq!(session_action("kill-process", &["s1".into()], &[]).unwrap(), json!({"kind": "killProcess", "id": "s1"}));
        assert!(session_action("kill", &[], &[]).unwrap_err().contains("vapp session kill <session>"));
    }
}
