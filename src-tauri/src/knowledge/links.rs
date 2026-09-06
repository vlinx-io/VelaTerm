//! References record a reviewed file digest, so edits are detected even before the next index sync.
use super::*;

pub(super) fn all(app: &AppCtx, entry_id: &str, index_id: &str) -> Result<Vec<Value>> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT l.id,l.entry_id,l.index_id,l.file_path,l.symbol,l.kind,l.digest,l.reviewed_at,i.root,i.project_id,e.title FROM knowledge_links l JOIN knowledge_indexes i ON i.id=l.index_id JOIN memory_entries e ON e.id=l.entry_id WHERE (?1='' OR l.entry_id=?1) AND (?2='' OR l.index_id=?2) ORDER BY l.reviewed_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![entry_id,index_id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"entryId":r.get::<_,String>(1)?,"indexId":r.get::<_,String>(2)?,"filePath":r.get::<_,String>(3)?,"symbol":r.get::<_,String>(4)?,"kind":r.get::<_,String>(5)?,"digest":r.get::<_,String>(6)?,"reviewedAt":r.get::<_,i64>(7)?,"root":r.get::<_,String>(8)?,"projectId":r.get::<_,String>(9)?,"title":r.get::<_,String>(10)?})))
        .map_err(|e| e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    Ok(rows)
}
fn inspect(mut link: Value) -> Value {
    let root = PathBuf::from(link["root"].as_str().unwrap());
    let current = graph::file(&root,link["filePath"].as_str().unwrap()).map(|b|graph::digest(&b));
    link["status"] = json!(match &current { Ok(d) if Some(d.as_str())==link["digest"].as_str() => "current", Ok(_) => "review", Err(_) => "unavailable" });
    link["nodeId"] = graph::open(&root).and_then(|c|graph::relocate(&c,link["filePath"].as_str().unwrap(),link["symbol"].as_str().unwrap(),link["kind"].as_str().unwrap())).map(|n|n["id"].clone()).unwrap_or(Value::Null);
    link
}
pub fn for_entry(app: &AppCtx, entry: &str) -> Result<Value> {
    Ok(json!(all(app,entry,"")?.into_iter().map(inspect).collect::<Vec<_>>()))
}
pub fn for_node(app: &AppCtx, index: &Index, node: &Value) -> Result<Value> {
    Ok(json!(all(app,"",&index.id)?.into_iter().filter(|l| l["filePath"]==node["filePath"] && l["symbol"]==node["qualifiedName"] && l["kind"]==node["kind"]).map(inspect).collect::<Vec<_>>()))
}
pub fn create(app: &AppCtx, args: &Value) -> Result<Value> {
    let entry = required(args,"entryId")?;
    let index = ready(app,required(args,"indexId")?)?;
    let detail = graph::detail(Path::new(&index.root),required(args,"nodeId")?)?;
    if detail["changedDuringRead"] == true { return Err("knowledge_conflict".into()); }
    let n = &detail["node"];
    let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e|e.to_string())?;
    // The review applies to the displayed memory version as well as to the exact displayed source.
    let version: i64 = tx.query_row("SELECT version FROM memory_entries WHERE id=?1",[entry],|r|r.get(0)).map_err(|_| "knowledge_not_found")?;
    if args["version"].as_i64()!=Some(version) || args["digest"]!=detail["digest"] { return Err("knowledge_conflict".into()); }
    tx.execute("INSERT INTO knowledge_links(id,entry_id,index_id,file_path,symbol,kind,digest,reviewed_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(entry_id,index_id,file_path,symbol,kind) DO NOTHING",
        params![uuid::Uuid::new_v4().to_string(),entry,index.id,n["filePath"].as_str(),n["qualifiedName"].as_str(),n["kind"].as_str(),detail["digest"].as_str(),now()]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(Value::Null)
}
pub fn remove(app: &AppCtx,args: &Value) -> Result<Value> {
    app.db().conn.lock().map_err(|e|e.to_string())?.execute("DELETE FROM knowledge_links WHERE id=?1 AND entry_id=?2",params![required(args,"id")?,required(args,"entryId")?]).map_err(|e|e.to_string())?;
    Ok(Value::Null)
}
pub fn review(app: &AppCtx,args: &Value) -> Result<Value> {
    let entry = required(args,"entryId")?; let id = required(args,"id")?;
    let link = all(app,entry,"")?.into_iter().find(|l| l["id"]==id).ok_or("knowledge_not_found")?;
    let index = ready(app,link["indexId"].as_str().unwrap())?;
    let root = Path::new(&index.root);
    let node = graph::relocate(&graph::open(root)?,link["filePath"].as_str().unwrap(),link["symbol"].as_str().unwrap(),link["kind"].as_str().unwrap())?;
    let detail = graph::detail(root,node["id"].as_str().unwrap())?;
    if detail["changedDuringRead"]==true || args["digest"]!=detail["digest"] { return Err("knowledge_conflict".into()); }
    let conn = app.db().conn.lock().map_err(|e|e.to_string())?;
    let count = conn.execute("UPDATE knowledge_links SET digest=?1,reviewed_at=?2 WHERE id=?3 AND digest=?4 AND EXISTS (SELECT 1 FROM memory_entries WHERE id=?5 AND version=?6)",
        params![detail["digest"].as_str(),now(),id,link["digest"].as_str(),entry,args["version"].as_i64().unwrap_or(-1)]).map_err(|e|e.to_string())?;
    if count!=1 { return Err("knowledge_conflict".into()); }
    Ok(Value::Null)
}
