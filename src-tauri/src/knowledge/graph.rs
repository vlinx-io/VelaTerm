//! Read-only adapter for the pinned CodeGraph 1.6 index schema. Symbol identities stay exact.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

pub fn open(root: &Path) -> Result<Connection> {
    let dir = root.join(".codegraph").canonicalize().map_err(|_| "knowledge_index_missing")?;
    if !dir.starts_with(root) { return Err("knowledge_invalid_directory".into()); }
    let path = dir.join("codegraph.db").canonicalize().map_err(|_| "knowledge_index_missing")?;
    if !path.starts_with(&dir) { return Err("knowledge_invalid_directory".into()); }
    let conn = Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|_| "knowledge_index_invalid")?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|_| "knowledge_index_invalid")?;
    Ok(conn)
}
pub fn stats(root: &Path) -> Result<Value> {
    let conn = open(root)?;
    let count = |table: &str| -> Result<i64> { conn.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0)).map_err(|_| "knowledge_index_invalid".into()) };
    let state: Option<String> = conn.query_row("SELECT value FROM project_metadata WHERE key='index_state'",[],|r|r.get(0)).optional().map_err(|_| "knowledge_index_invalid")?;
    Ok(json!({"files":count("files")?,"nodes":count("nodes")?,"edges":count("edges")?,"state":state}))
}
const NODE: &str = "id,name,qualified_name,kind,file_path,start_line,end_line,signature,language";
fn node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({"id":row.get::<_,String>(0)?,"name":row.get::<_,String>(1)?,"qualifiedName":row.get::<_,String>(2)?,
        "kind":row.get::<_,String>(3)?,"filePath":row.get::<_,String>(4)?,"startLine":row.get::<_,i64>(5)?,
        "endLine":row.get::<_,i64>(6)?,"signature":row.get::<_,Option<String>>(7)?,"language":row.get::<_,String>(8)?}))
}
pub fn find(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("SELECT {NODE} FROM nodes WHERE id=?1"),[id],node).optional()
        .map_err(|_| "knowledge_index_invalid")?.ok_or_else(|| "knowledge_symbol_missing".into())
}
pub fn relocate(conn: &Connection, file: &str, symbol: &str, kind: &str) -> Result<Value> {
    let mut stmt = conn.prepare(&format!("SELECT {NODE} FROM nodes WHERE file_path=?1 AND qualified_name=?2 AND kind=?3 LIMIT 2")).map_err(|_| "knowledge_index_invalid")?;
    let matches = stmt.query_map(params![file,symbol,kind],node).map_err(|_| "knowledge_index_invalid")?
        .collect::<std::result::Result<Vec<_>,_>>().map_err(|_| "knowledge_index_invalid")?;
    if matches.len() != 1 { return Err("knowledge_symbol_missing".into()); }
    Ok(matches[0].clone())
}
pub fn search(root: &Path, query: &str, page: u64) -> Result<Value> {
    if query.len() > 500 { return Err("knowledge_invalid".into()); }
    let conn = open(root)?;
    let pattern = format!("%{}%",query.replace('\\',"\\\\").replace('%',"\\%").replace('_',"\\_"));
    let predicate = r"(?1='%%' OR name LIKE ?1 ESCAPE '\' OR qualified_name LIKE ?1 ESCAPE '\' OR file_path LIKE ?1 ESCAPE '\')";
    let total: i64 = conn.query_row(&format!("SELECT count(*) FROM nodes WHERE {predicate}"),[&pattern],|r|r.get(0)).map_err(|_| "knowledge_index_invalid")?;
    let mut stmt = conn.prepare(&format!("SELECT {NODE} FROM nodes WHERE {predicate} ORDER BY name,file_path,start_line,id LIMIT 40 OFFSET ?2")).map_err(|_| "knowledge_index_invalid")?;
    let nodes = stmt.query_map(params![pattern,page.min(100_000)*40],node).map_err(|_| "knowledge_index_invalid")?
        .collect::<std::result::Result<Vec<_>,_>>().map_err(|_| "knowledge_index_invalid")?;
    Ok(json!({"nodes":nodes,"total":total,"pageSize":40}))
}
pub fn file(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let path = Path::new(relative);
    if path.is_absolute() || path.components().any(|c| !matches!(c,std::path::Component::Normal(_))) { return Err("knowledge_invalid".into()); }
    let path = root.join(path).canonicalize().map_err(|_| "knowledge_source_missing")?;
    if !path.starts_with(root) { return Err("knowledge_invalid".into()); }
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|_| "knowledge_source_missing")?.take(4*1024*1024+1).read_to_end(&mut bytes).map_err(|_| "knowledge_source_missing")?;
    if bytes.len() > 4*1024*1024 { return Err("knowledge_source_large".into()); }
    Ok(bytes)
}
pub fn digest(bytes: &[u8]) -> String { format!("{:x}",Sha256::digest(bytes)) }
pub fn detail(root: &Path, id: &str) -> Result<Value> {
    let conn = open(root)?;
    // Keep symbols, edges and indexed file hashes on one SQLite snapshot while another process syncs.
    let _snapshot = conn.unchecked_transaction().map_err(|_| "knowledge_index_invalid")?;
    let selected = find(&conn,id)?;
    let mut incoming = Vec::new(); let mut outgoing = Vec::new();
    for (incoming_direction, destination) in [(true,&mut incoming),(false,&mut outgoing)] {
        let (other,focus) = if incoming_direction {("source","target")} else {("target","source")};
        let cols = NODE.split(',').map(|s| format!("n.{s}")).collect::<Vec<_>>().join(",");
        let mut stmt = conn.prepare(&format!("SELECT {cols},e.kind,e.line,e.provenance,e.metadata FROM edges e JOIN nodes n ON n.id=e.{other} WHERE e.{focus}=?1 ORDER BY n.file_path,n.start_line,e.id LIMIT 201")).map_err(|_| "knowledge_index_invalid")?;
        let rows = stmt.query_map([id],|r| Ok(json!({"node":node(r)?,"kind":r.get::<_,String>(9)?,"line":r.get::<_,Option<i64>>(10)?,"provenance":r.get::<_,Option<String>>(11)?,"metadata":r.get::<_,Option<String>>(12)?})))
            .map_err(|_| "knowledge_index_invalid")?.collect::<std::result::Result<Vec<_>,_>>().map_err(|_| "knowledge_index_invalid")?;
        destination.extend(rows);
    }
    let bytes = file(root,selected["filePath"].as_str().ok_or("knowledge_index_invalid")?)?;
    let text = String::from_utf8(bytes.clone()).map_err(|_| "knowledge_index_invalid")?;
    let start = selected["startLine"].as_u64().unwrap_or(1).max(1) as usize;
    let end = selected["endLine"].as_u64().unwrap_or(start as u64) as usize;
    let source = text.lines().skip(start-1).take(end.saturating_sub(start)+1).take(600).collect::<Vec<_>>().join("\n");
    let indexed_hash: String = conn.query_row("SELECT content_hash FROM files WHERE path=?1",[&selected["filePath"].as_str().unwrap()],|r|r.get(0)).map_err(|_| "knowledge_index_invalid")?;
    let truncated = incoming.len()>200 || outgoing.len()>200;
    incoming.truncate(200); outgoing.truncate(200);
    Ok(json!({"node":selected,"incoming":incoming,"outgoing":outgoing,"truncated":truncated,
        "source":source,"sourceTruncated":end.saturating_sub(start)+1>600,"digest":digest(&bytes),"changedDuringRead":indexed_hash!=digest(&bytes)}))
}
