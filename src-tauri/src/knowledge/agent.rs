//! Session-scoped, read-only knowledge queries over the existing authenticated loopback channel.
use super::*;
use std::io::Read;
use std::time::{Duration, Instant};

pub fn query(app: &AppCtx, args: &Value) -> Result<Value> {
    let sid = required(args,"sessionId")?;
    let session = {
        let conn = app.db().conn.lock().map_err(|e|e.to_string())?;
        crate::db::repo::get_session(&conn,sid)?.ok_or("knowledge_not_found")?
    };
    let action = required(args,"action")?;
    let text = args.get("query").and_then(Value::as_str).unwrap_or("");
    if text.len()>500 { return Err("knowledge_invalid".into()); }
    if action=="memory" {
        let mut value = crate::memory::dispatch(app,"memory_get",&json!({"id":text}))?;
        value["codeReferences"] = links::for_entry(app,text)?;
        return Ok(value);
    }
    let memories = crate::memory::dispatch(app,"memory_list",&json!({"query":text,"tag":"","sort":"updated","page":0}))?;
    if action=="memories" { return Ok(memories); }
    if !["search","status","node"].contains(&action) { return Err("knowledge_invalid".into()); }
    let cwd = canonical(Path::new(required(args,"cwd")?))?;
    let current_checkout = git_root(&cwd);
    let targets = list(app,&session.project_id)?;
    let index = targets["indexes"].as_array().and_then(|items|items.iter().filter(|i| i["root"].as_str().is_some_and(|r| cwd.starts_with(r) && git_root(Path::new(r))==current_checkout)).max_by_key(|i|i["root"].as_str().map(str::len).unwrap_or(0)));
    let Some(index) = index else {
        return Ok(json!({"code":{"available":false,"reason":"knowledge_index_missing"},"memories":memories,"cwd":cwd}));
    };
    if action=="status" { return Ok(json!({"index":index,"runtime":targets["runtime"]})); }
    let id = index["id"].as_str().ok_or("knowledge_invalid")?;
    let result = if action=="node" { dispatch(app,"knowledge_node",&json!({"id":id,"nodeId":text})) }
        else { dispatch(app,"knowledge_search",&json!({"id":id,"query":text})) };
    let code = match result { Ok(data) => json!({"available":true,"data":data}), Err(error) => json!({"available":false,"reason":error}) };
    Ok(json!({"code":code,"memories":memories,"indexId":id,"root":cwd}))
}
pub fn handle(app: AppCtx, mut request: tiny_http::Request, token: String) {
    let started = Instant::now();
    let method = request.method().as_str().to_string();
    let request_id = request.headers().iter().find(|h|h.field.equiv("X-Request-Id")).map(|h|h.value.as_str())
        .filter(|s| !s.is_empty() && s.len()<=64 && s.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_'))
        .map(str::to_owned).unwrap_or_else(||uuid::Uuid::new_v4().to_string());
    let authorized = request.headers().iter().any(|h|h.field.equiv("Authorization") && h.value.as_str()==format!("Bearer {token}"));
    let mut body = String::new();
    let result = if !authorized { Err((401,"knowledge_unauthorized".to_string())) }
        else if request.method()!=&tiny_http::Method::Post { Err((405,"knowledge_invalid".to_string())) }
        else if request.as_reader().take(16_385).read_to_string(&mut body).is_err() || body.len()>16_384 { Err((413,"knowledge_invalid".to_string())) }
        else { serde_json::from_str(&body).map_err(|_|"knowledge_invalid".to_string()).and_then(|args|query(&app,&args)).map_err(|e|(400,e)) };
    let (status,value) = match result { Ok(value)=>(200,json!({"result":value})), Err((status,error))=>(status,json!({"error":error})) };
    let response = tiny_http::Response::from_string(value.to_string()).with_status_code(status)
        .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())
        .with_header(tiny_http::Header::from_bytes("X-Request-Id",request_id.as_str()).unwrap());
    let _ = request.respond(response);
    runtime::audit(&app,&request_id,if status==200{"INFO"}else{"ERROR"},
        &format!("http_access requestMethod={method} path=/knowledge statusCode={status}"),started.elapsed().as_millis());
}
pub fn run(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    if rest.is_empty() || rest[0]=="--help" || rest[0]=="-h" {
        println!("usage: vknowledge search <query> | node <symbol-id> | memories <query> | memory <entry-id> | status\nQueries the current checkout and this VelaTerm backend's global memory. Code indexes must be enabled in the project's Code Graph page. Results carry availability and review status; no memory is changed.");
        std::process::exit(0);
    }
    let result = (|| -> Result<Value> {
        let base = std::env::var("VLX_SPAWN_URL").map_err(|_|"Run vknowledge inside a VelaTerm session.")?;
        let sid = std::env::var("VLX_SESSION_ID").map_err(|_|"Missing VelaTerm session.")?;
        let token = std::env::var("VLX_TOKEN").map_err(|_|"Missing VelaTerm session credentials.")?;
        let url = url::Url::parse(&base).map_err(|_|"Invalid VelaTerm endpoint.")?;
        if url.scheme()!="http" || !matches!(url.host_str(),Some("127.0.0.1" | "localhost")) { return Err("Invalid VelaTerm endpoint.".into()); }
        let cwd = std::env::current_dir().map_err(|_|"Cannot resolve current directory.")?;
        let body = json!({"sessionId":sid,"cwd":cwd,"action":rest[0],"query":rest[1..].join(" ")});
        let request = ureq::AgentBuilder::new().timeout(Duration::from_secs(1900)).build().post(&format!("{base}/knowledge"))
            .set("Authorization",&format!("Bearer {token}")).set("Content-Type","application/json").set("X-Request-Id",&uuid::Uuid::new_v4().to_string()).send_string(&body.to_string());
        let response = match request { Ok(r)=>r, Err(ureq::Error::Status(_,r))=>r, Err(_)=>return Err("Cannot reach the VelaTerm knowledge service.".into()) };
        let mut bytes = String::new(); response.into_reader().take(8*1024*1024).read_to_string(&mut bytes).map_err(|_|"Cannot read knowledge response.")?;
        let value: Value = serde_json::from_str(&bytes).map_err(|_|"Invalid knowledge response.")?;
        if let Some(error) = value["error"].as_str() { return Err(error.into()); }
        Ok(value["result"].clone())
    })();
    match result { Ok(value)=>{println!("{}",serde_json::to_string_pretty(&value).unwrap());std::process::exit(0)}, Err(error)=>{eprintln!("vknowledge: {error}");std::process::exit(1)} }
}
