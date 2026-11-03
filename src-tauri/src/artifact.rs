//! Artifact previews: serve a local HTML page, and the files beside it, to a sandboxed iframe.
//!
//! An artifact is a page an agent or a person wrote on disk. It has to load from a real, hierarchical URL
//! because pages reference their neighbours (`./style.css`, `./app.js`, the `./support.js` runtime Claude
//! Design exports), which neither `srcdoc` nor a Blob URL can resolve. So a tab asks for a grant: a random ID
//! bound to the directory holding the page, and every request names a grant plus a path under it.
//!
//! The same [`serve`] answers three entrances: the desktop app's `vlxart` scheme, and `/api/artifact/` on the
//! web server for Electron, browsers and remote clients. A grant is the whole credential, like a download
//! ticket, because the iframe's requests carry no session; it is 128 random bits, bound to one directory, and
//! a lease that lapses unless the tab renews it.
//!
//! What runs in the frame is untrusted. The iframe is sandboxed without `allow-same-origin`, so it gets an
//! opaque origin and no access to the app; this module adds a Content-Security-Policy that keeps it offline
//! unless the user allowed the network for that file, and injects a small script that reports console errors
//! and blocked requests to the parent. The script only posts messages; it is given nothing to call.
//!
//! Version snapshots live here too: each detected change of the entry file is copied into the data
//! directory, so the preview can step back through earlier saves.

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// How long a grant survives without renewal. The tab renews every minute while it is open, so a closed
/// window or a dropped remote connection frees its grants without an explicit revoke.
const LEASE: Duration = Duration::from_secs(180);

/// Largest file served. A page and its assets are small; anything larger is a mistake or an attack.
const MAX_FILE: u64 = 20 * 1024 * 1024;

/// Snapshots kept per file, and the byte budget they share.
const MAX_VERSIONS: usize = 20;
const MAX_VERSION_BYTES: u64 = 10 * 1024 * 1024;

/// Path prefix of the web-server entrance.
pub const HTTP_PREFIX: &str = "/api/artifact/";

struct Grant {
    /// Canonical directory every served file must sit under.
    root: PathBuf,
    /// The entry page, relative to `root`, with `/` separators.
    entry: String,
    /// A snapshot served in place of the entry page when an earlier version is being viewed.
    entry_override: Option<PathBuf>,
    /// The URL prefix this grant is reached through, ending in `<grant>/`; the CSP names it as the only source.
    source: String,
    /// Whether the user allowed this page to reach the internet.
    net: bool,
    /// Directory that must never be served even when it sits under `root` (the data directory, for remote callers).
    deny: Option<PathBuf>,
    renewed: Instant,
}

fn grants() -> &'static Mutex<HashMap<String, Grant>> {
    static GRANTS: OnceLock<Mutex<HashMap<String, Grant>>> = OnceLock::new();
    GRANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// What a tab needs to build the iframe URL: `<base><grantId>/<entry>`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    pub grant_id: String,
    /// The entry page relative to the root, percent-encoded per segment.
    pub entry: String,
    pub root: String,
}

/// One saved version of an artifact's entry page.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: u32,
    pub mtime_ms: u64,
    pub size: u64,
}

/// Options for [`issue`].
pub struct IssueOptions<'a> {
    /// URL prefix the client reaches grants through, such as `vlxart://localhost/`; must end in `/`.
    pub base: &'a str,
    pub net: bool,
    /// Serve this snapshot instead of the current entry page.
    pub version: Option<u32>,
    pub data_dir: Option<&'a Path>,
    pub deny: Option<PathBuf>,
}

/// Mint a grant for the page at `path`.
pub fn issue(path: &str, opts: IssueOptions) -> Result<GrantInfo, String> {
    if !valid_base(opts.base) {
        return Err("Invalid preview address.".into());
    }
    let file = Path::new(path)
        .canonicalize()
        .map_err(|_| "This file is no longer available.".to_string())?;
    if !file.is_file() {
        return Err("This file is no longer available.".into());
    }
    let root = file.parent().ok_or("This file is no longer available.")?.to_path_buf();
    if let Some(deny) = &opts.deny {
        if file.starts_with(deny) {
            return Err(format!("remote_path_forbidden:{path}"));
        }
    }
    let name = file.file_name().and_then(|n| n.to_str()).ok_or("This file name is not supported.")?.to_string();
    let entry_override = match opts.version {
        Some(n) => {
            let dir = versions_dir(opts.data_dir.ok_or("Version history is unavailable.")?, &file);
            let snap = dir.join(format!("v{n}.html"));
            if !snap.is_file() {
                return Err("This version is no longer available.".into());
            }
            Some(snap)
        }
        None => None,
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    let source = format!("{}{id}/", opts.base);
    let mut map = grants().lock().map_err(|_| "Preview is unavailable.")?;
    map.retain(|_, g| g.renewed.elapsed() < LEASE);
    map.insert(
        id.clone(),
        Grant { root: root.clone(), entry: name.clone(), entry_override, source, net: opts.net, deny: opts.deny, renewed: Instant::now() },
    );
    Ok(GrantInfo { grant_id: id, entry: encode_segment(&name), root: root.to_string_lossy().into_owned() })
}

/// Extend a grant's lease; false when it already lapsed and the tab must ask for a new one.
pub fn renew(id: &str) -> bool {
    let Ok(mut map) = grants().lock() else { return false };
    map.retain(|_, g| g.renewed.elapsed() < LEASE);
    match map.get_mut(id) {
        Some(g) => {
            g.renewed = Instant::now();
            true
        }
        None => false,
    }
}

pub fn revoke(id: &str) {
    if let Ok(mut map) = grants().lock() {
        map.remove(id);
    }
}

/// A base the CSP can quote safely: a scheme the app uses, no whitespace or CSP punctuation, ending in `/`.
fn valid_base(base: &str) -> bool {
    let scheme_ok = ["vlxart://", "http://", "https://"].iter().any(|s| base.starts_with(s));
    scheme_ok
        && base.len() <= 512
        && base.ends_with('/')
        && !base.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, ';' | ',' | '\'' | '"' | '\\'))
}

/// One HTTP-shaped answer, built once and adapted by each entrance.
pub struct Served {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Served {
    fn not_found() -> Served {
        Served { status: 404, headers: vec![("Content-Type", "text/plain; charset=utf-8".into())], body: b"Not found".to_vec() }
    }
}

/// Answer a request path of the form `<grant>/<relative path>` (no leading slash, query already removed).
pub fn serve(path: &str) -> Served {
    let (id, rel) = match path.split_once('/') {
        Some((id, rel)) => (id, rel),
        None => return Served::not_found(),
    };
    let (file, source, net) = {
        let Ok(mut map) = grants().lock() else { return Served::not_found() };
        map.retain(|_, g| g.renewed.elapsed() < LEASE);
        let Some(grant) = map.get(id) else { return Served::not_found() };
        let rel = match decode_rel(rel) {
            Some(rel) => rel,
            None => return Served::not_found(),
        };
        // An empty path means the entry page itself.
        let rel = if rel.is_empty() { grant.entry.clone() } else { rel };
        let file = if rel == grant.entry && grant.entry_override.is_some() {
            grant.entry_override.clone()
        } else {
            resolve(&grant.root, &rel, grant.deny.as_deref())
        };
        match file {
            Some(file) => (file, grant.source.clone(), grant.net),
            None => return Served::not_found(),
        }
    };
    let meta = match fs::metadata(&file) {
        Ok(m) if m.is_file() && m.len() <= MAX_FILE => m,
        _ => return Served::not_found(),
    };
    let mut body = match fs::read(&file) {
        Ok(b) if b.len() as u64 == meta.len() => b,
        _ => return Served::not_found(),
    };
    let mime = mime_for(&file);
    let html = mime.starts_with("text/html");
    let policy = csp(&source, net);
    if html {
        body = inject_bootstrap(&body, &policy);
    }
    let mut headers = vec![
        ("Content-Type", mime.to_string()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Cache-Control", "no-store".into()),
        ("Referrer-Policy", "no-referrer".into()),
        // The frame's origin is opaque, so its own module scripts, fonts and fetches are cross-origin
        // requests; the grant in the URL is the credential, not the origin.
        ("Access-Control-Allow-Origin", "*".into()),
    ];
    if html {
        headers.push(("Content-Security-Policy", policy));
    }
    Served { status: 200, headers, body }
}

/// Resolve `rel` under `root`, refusing anything that escapes it, including through a symlink.
fn resolve(root: &Path, rel: &str, deny: Option<&Path>) -> Option<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let full = root.join(rel_path).canonicalize().ok()?;
    if !full.starts_with(root) {
        return None;
    }
    if deny.is_some_and(|d| full.starts_with(d)) {
        return None;
    }
    Some(full)
}

/// Percent-decode a URL path. Rejects NUL, backslashes and invalid UTF-8 rather than guessing.
fn decode_rel(rel: &str) -> Option<String> {
    let bytes = rel.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let s = String::from_utf8(out).ok()?;
    if s.contains('\0') || s.contains('\\') || s.starts_with('/') {
        return None;
    }
    Some(s)
}

/// Percent-encode one path segment for a URL.
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn mime_for(file: &Path) -> &'static str {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" | "cjs" | "jsx" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "wasm" => "application/wasm",
        "txt" | "md" | "csv" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

/// The policy for one grant: only the grant's own files, plus the internet when the user allowed it.
fn csp(source: &str, net: bool) -> String {
    let web = if net { " https:" } else { "" };
    let sock = if net { " https: wss:" } else { "" };
    format!(
        "default-src {source}{web}; \
         script-src {source} 'unsafe-inline' 'unsafe-eval' blob: data:{web}; \
         style-src {source} 'unsafe-inline' data:{web}; \
         img-src {source} data: blob:{web}; \
         font-src {source} data:{web}; \
         media-src {source} data: blob:{web}; \
         connect-src {source} data: blob:{sock}; \
         worker-src {source} blob:; \
         frame-src {source} data: blob:{web}; \
         object-src 'none'; form-action 'none'; base-uri 'none'"
    )
}

/// Reports console errors, uncaught errors, blocked requests and scroll position to the parent, and restores
/// the scroll position a reload passed back in the query. It posts messages and accepts none.
const BOOTSTRAP: &str = r#"<script>(function(){var S="vlx-artifact";function send(m){try{m.source=S;parent.postMessage(m,"*")}catch(e){}}function txt(a){try{return Array.prototype.map.call(a,function(x){if(x instanceof Error)return x.stack||String(x);if(x&&typeof x==="object"){try{return JSON.stringify(x)}catch(e){return String(x)}}return String(x)}).join(" ").slice(0,2000)}catch(e){return""}}["error","warn"].forEach(function(l){var o=console[l];console[l]=function(){send({type:"console",level:l,text:txt(arguments)});return o.apply(console,arguments)}});addEventListener("error",function(e){var t=e.target;if(t&&t!==window&&(t.src||t.href)){send({type:"console",level:"error",text:"Failed to load "+String(t.src||t.href).slice(0,300)});return}send({type:"error",text:String(e.message||"Error").slice(0,2000),line:e.lineno|0})},true);addEventListener("unhandledrejection",function(e){send({type:"error",text:txt([e.reason]),line:0})});document.addEventListener("securitypolicyviolation",function(e){send({type:"blocked",uri:String(e.blockedURI||"").slice(0,300),directive:String(e.effectiveDirective||e.violatedDirective||"")})});var q=/[?&]__vlx_scroll=(\d+)/.exec(location.search),pend=0;addEventListener("scroll",function(){if(pend)return;pend=setTimeout(function(){pend=0;send({type:"scroll",y:scrollY|0})},250)},{passive:true});if(q)addEventListener("load",function(){scrollTo(0,+q[1])});send({type:"ready"})})();</script>"#;

/// Review tools the preview drives from outside: picking an element to comment on, numbered pins on commented
/// elements, scrolling to an element or a design option, and listing the page's design options (elements marked
/// `data-vlx-option="<id>"`). It accepts messages only from the parent window and only in its own source tag;
/// they change the overlay it draws (marked `data-vlx-ui`) and the scroll position, nothing in the page itself.
const REVIEW: &str = r##"<script>(function(){
var S="vlx-artifact",H="vlx-artifact-host",ACC="#10b981";
function send(m){try{m.source=S;parent.postMessage(m,"*")}catch(e){}}
var picking=false,hover=null,box=null,marks=[],pins=[],layer=null;
function el(tag,css){var e=document.createElement(tag);e.style.cssText=css;e.setAttribute("data-vlx-ui","");return e}
function root(){var host=document.body||document.documentElement;if(!layer)layer=el("div","position:fixed;inset:0;pointer-events:none;z-index:2147483647");if(!layer.isConnected)host.appendChild(layer);return layer}
function ours(n){return !!(n&&n.closest&&n.closest("[data-vlx-ui]"))}
function sel(n){var parts=[];while(n&&n.nodeType===1&&n!==document.documentElement&&parts.length<6){if(n.id&&document.querySelectorAll("#"+CSS.escape(n.id)).length===1){parts.unshift("#"+CSS.escape(n.id));break}var t=n.tagName.toLowerCase(),p=n.parentElement;if(p){var same=Array.prototype.filter.call(p.children,function(c){return c.tagName===n.tagName});if(same.length>1)t+=":nth-of-type("+(same.indexOf(n)+1)+")"}parts.unshift(t);n=p}return parts.join(" > ")}
function find(s){try{return s?document.querySelector(s):null}catch(e){return null}}
function optOf(n){var o=n&&n.closest&&n.closest("[data-vlx-option]");return o?String(o.getAttribute("data-vlx-option")).trim().slice(0,40):null}
function label(n){var t=(n.innerText||n.textContent||"").replace(/\s+/g," ").trim();if(!t)t=n.getAttribute("aria-label")||n.getAttribute("alt")||n.getAttribute("placeholder")||"";return t.slice(0,120)}
function place(b,n,pad){var r=n.getBoundingClientRect();b.style.left=(r.left-pad)+"px";b.style.top=(r.top-pad)+"px";b.style.width=(r.width+pad*2)+"px";b.style.height=(r.height+pad*2)+"px"}
function showHover(n){if(!box){box=el("div","position:fixed;border:2px solid "+ACC+";background:rgba(16,185,129,.08);border-radius:4px");root().appendChild(box)}box.style.display=n?"block":"none";if(n)place(box,n,2)}
function move(e){var n=e.target;if(ours(n)||n===document.documentElement||n===document.body){hover=null;showHover(null);return}hover=n;showHover(n)}
function click(e){e.preventDefault();e.stopPropagation();var n=hover||e.target;if(!n||ours(n)||n.nodeType!==1)return;send({type:"picked",selector:sel(n).slice(0,500),tag:n.tagName.toLowerCase(),text:label(n),option:optOf(n)})}
function block(e){e.preventDefault();e.stopPropagation()}
function key(e){if(e.key==="Escape"){e.preventDefault();send({type:"pickCancel"})}}
function setPick(on){if(on===picking)return;picking=on;var f=on?addEventListener:removeEventListener;f("mousemove",move,true);f("click",click,true);f("mousedown",block,true);f("mouseup",block,true);f("keydown",key,true);document.documentElement.style.cursor=on?"crosshair":"";if(!on){hover=null;showHover(null)}}
function drawPins(){pins.forEach(function(p){p.remove()});pins=[];marks.forEach(function(m){var n=find(m.selector);if(!n)return;var r=n.getBoundingClientRect();if(!m.n){var o=el("div","position:fixed;border:2px dashed "+ACC+";border-radius:4px");place(o,n,2);root().appendChild(o);pins.push(o)}var p=el("div","position:fixed;min-width:18px;height:18px;padding:0 5px;box-sizing:border-box;border-radius:9px;background:"+ACC+";color:#fff;font:600 11px/18px system-ui,sans-serif;text-align:center;box-shadow:0 1px 4px rgba(0,0,0,.3)");p.textContent=m.n?String(m.n):"+";p.style.left=Math.max(2,r.right-10)+"px";p.style.top=Math.max(2,r.top-9)+"px";root().appendChild(p);pins.push(p)})}
var raf=0;function redraw(){if(raf)return;raf=requestAnimationFrame(function(){raf=0;if(marks.length)drawPins();if(hover&&box)place(box,hover,2)})}
addEventListener("scroll",redraw,true);addEventListener("resize",redraw);
function options(){var seen={},list=[];Array.prototype.forEach.call(document.querySelectorAll("[data-vlx-option]"),function(n){var id=String(n.getAttribute("data-vlx-option")||"").trim().slice(0,40);if(!id||seen[id])return;seen[id]=1;var h=n.querySelector("[data-vlx-option-label],h1,h2,h3,h4,figcaption");var l=n.getAttribute("data-vlx-option-label")||(h?(h.getAttribute("data-vlx-option-label")||h.textContent||""):"");list.push({id:id,label:l.replace(/\s+/g," ").trim().slice(0,80)})});return list.slice(0,40)}
var last="";function report(){var o=options(),k=JSON.stringify(o);if(k!==last){last=k;send({type:"options",options:o})}}
var mt=0;function watch(){report();if(window.MutationObserver)new MutationObserver(function(ms){if(ms.every(function(m){return ours(m.target)}))return;clearTimeout(mt);mt=setTimeout(function(){report();if(marks.length)drawPins()},300)}).observe(document.documentElement,{subtree:true,childList:true,attributes:true,attributeFilter:["data-vlx-option","data-vlx-option-label"]})}
function reveal(n){if(!n)return;var r=n.getBoundingClientRect();scrollTo({top:scrollY+r.top-24,left:scrollX+r.left-24,behavior:"smooth"});var f=el("div","position:fixed;border:2px solid "+ACC+";border-radius:6px;box-shadow:0 0 0 4px rgba(16,185,129,.25);transition:opacity .6s");root().appendChild(f);var t0=Date.now();(function tick(){place(f,n,4);if(Date.now()-t0<900)requestAnimationFrame(tick);else{f.style.opacity="0";setTimeout(function(){f.remove()},700)}})()}
addEventListener("message",function(e){if(e.source!==parent)return;var m=e.data;if(!m||m.source!==H)return;if(m.type==="pick")setPick(!!m.on);else if(m.type==="marks"&&Array.isArray(m.marks)){marks=m.marks.slice(0,100).filter(function(x){return x&&typeof x.selector==="string"});drawPins()}else if(m.type==="reveal"){if(typeof m.option==="string")reveal(document.querySelector('[data-vlx-option="'+CSS.escape(m.option)+'"]'));else if(typeof m.selector==="string")reveal(find(m.selector))}});
if(document.readyState==="loading")addEventListener("DOMContentLoaded",watch);else watch();
})();</script>"##;

/// Insert the policy as a `<meta>` tag, then [`BOOTSTRAP`] and [`REVIEW`], as early as possible without pushing the doctype
/// off the first line, which would drop the page into quirks mode. The meta copy of the policy is a backstop
/// for proxies that drop response headers; both copies are identical, so enforcing both changes nothing.
fn inject_bootstrap(body: &[u8], policy: &str) -> Vec<u8> {
    let lower: Vec<u8> = body.iter().take(64 * 1024).map(|b| b.to_ascii_lowercase()).collect();
    let after_tag = |tag: &[u8]| -> Option<usize> {
        let start = lower.windows(tag.len()).position(|w| w == tag)?;
        // The tag name must end here (`<head>` or `<head ...>`, not `<header>`).
        let next = *lower.get(start + tag.len())?;
        if !(next == b'>' || next.is_ascii_whitespace()) {
            return None;
        }
        lower[start..].iter().position(|&b| b == b'>').map(|p| start + p + 1)
    };
    let at = after_tag(b"<head").or_else(|| after_tag(b"<html")).or_else(|| after_tag(b"<!doctype")).unwrap_or(0);
    let meta = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">");
    let mut out = Vec::with_capacity(body.len() + meta.len() + BOOTSTRAP.len() + REVIEW.len());
    out.extend_from_slice(&body[..at]);
    out.extend_from_slice(meta.as_bytes());
    out.extend_from_slice(BOOTSTRAP.as_bytes());
    out.extend_from_slice(REVIEW.as_bytes());
    out.extend_from_slice(&body[at..]);
    out
}

// ── Version snapshots ───────────────────────────────────────────────────────────────────────────────

/// Where one file's snapshots live: a directory named after a hash of its canonical path.
fn versions_dir(data_dir: &Path, file: &Path) -> PathBuf {
    let digest = Sha256::digest(file.to_string_lossy().as_bytes());
    let name: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    data_dir.join("artifacts").join(name)
}

fn canonical_file(path: &str) -> Result<PathBuf, String> {
    let file = Path::new(path).canonicalize().map_err(|_| "This file is no longer available.".to_string())?;
    if file.is_file() { Ok(file) } else { Err("This file is no longer available.".into()) }
}

/// Saved versions of `path`, oldest first.
pub fn versions(data_dir: &Path, path: &str) -> Result<Vec<VersionInfo>, String> {
    Ok(list_versions(&versions_dir(data_dir, &canonical_file(path)?)))
}

fn list_versions(dir: &Path) -> Vec<VersionInfo> {
    let mut out: Vec<VersionInfo> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let version = name.strip_prefix('v')?.strip_suffix(".html")?.parse::<u32>().ok()?;
            let meta = e.metadata().ok()?;
            let mtime_ms = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
            Some(VersionInfo { version, mtime_ms, size: meta.len() })
        })
        .collect();
    out.sort_by_key(|v| v.version);
    out
}

/// Save the entry page as a new version when it differs from the latest one, then prune to the limits.
pub fn snapshot(data_dir: &Path, path: &str) -> Result<Vec<VersionInfo>, String> {
    let file = canonical_file(path)?;
    let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
    let dir = versions_dir(data_dir, &file);
    if meta.len() > MAX_VERSION_BYTES {
        return Ok(list_versions(&dir));
    }
    let content = fs::read(&file).map_err(|e| format!("Failed to read file: {e}"))?;
    let mut list = list_versions(&dir);
    if let Some(last) = list.last() {
        if fs::read(dir.join(format!("v{}.html", last.version))).ok().as_deref() == Some(&content[..]) {
            return Ok(list);
        }
    }
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to save version: {e}"))?;
    let next = list.last().map_or(1, |v| v.version + 1);
    fs::write(dir.join(format!("v{next}.html")), &content).map_err(|e| format!("Failed to save version: {e}"))?;
    list = list_versions(&dir);
    // Drop the oldest versions until both limits hold; the newest is always kept.
    while list.len() > 1 && (list.len() > MAX_VERSIONS || list.iter().map(|v| v.size).sum::<u64>() > MAX_VERSION_BYTES) {
        let oldest = list.remove(0);
        let _ = fs::remove_file(dir.join(format!("v{}.html", oldest.version)));
    }
    Ok(list)
}

/// Write version `version` back over the entry page.
pub fn restore(data_dir: &Path, path: &str, version: u32) -> Result<(), String> {
    let file = canonical_file(path)?;
    let snap = versions_dir(data_dir, &file).join(format!("v{version}.html"));
    let content = fs::read(&snap).map_err(|_| "This version is no longer available.".to_string())?;
    fs::write(&file, content).map_err(|e| format!("Failed to restore version: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-artifact-{name}-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn grant(dir: &Path, page: &str) -> GrantInfo {
        let opts = IssueOptions { base: "vlxart://localhost/", net: false, version: None, data_dir: None, deny: None };
        issue(&dir.join(page).to_string_lossy(), opts).unwrap()
    }

    fn header<'a>(s: &'a Served, name: &str) -> Option<&'a str> {
        s.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    #[test]
    fn serves_entry_and_neighbours_with_policy() {
        let dir = temp_dir("serve");
        fs::write(dir.join("index.html"), "<!doctype html><html><head><title>x</title></head><body></body></html>").unwrap();
        fs::create_dir(dir.join("css")).unwrap();
        fs::write(dir.join("css/a b.css"), "body{}").unwrap();
        let g = grant(&dir, "index.html");
        let page = serve(&format!("{}/{}", g.grant_id, g.entry));
        assert_eq!(page.status, 200);
        let html = String::from_utf8(page.body.clone()).unwrap();
        assert!(html.starts_with("<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\""), "{html}");
        assert!(html.contains("\"><script>(function(){"), "{html}");
        let policy = header(&page, "content-security-policy").unwrap();
        assert!(policy.contains(&format!("default-src vlxart://localhost/{}/;", g.grant_id)));
        assert!(!policy.contains("https:"));
        let css = serve(&format!("{}/css/a%20b.css", g.grant_id));
        assert_eq!(css.status, 200);
        assert_eq!(header(&css, "content-type"), Some("text/css; charset=utf-8"));
        assert!(header(&css, "content-security-policy").is_none());
        assert_eq!(serve(&format!("{}/", g.grant_id)).status, 200);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_paths_outside_the_root() {
        let dir = temp_dir("escape");
        let inner = dir.join("site");
        fs::create_dir(&inner).unwrap();
        fs::write(inner.join("index.html"), "<html></html>").unwrap();
        fs::write(dir.join("secret.txt"), "secret").unwrap();
        let g = grant(&inner, "index.html");
        for rel in ["../secret.txt", "%2e%2e/secret.txt", "%2E%2E%2Fsecret.txt", "..%5csecret.txt", "/etc/passwd", "%2fetc%2fpasswd", "a%00b"] {
            assert_eq!(serve(&format!("{}/{rel}", g.grant_id)).status, 404, "{rel}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("secret.txt"), inner.join("link.txt")).unwrap();
            assert_eq!(serve(&format!("{}/link.txt", g.grant_id)).status, 404);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_and_revoked_grants_are_not_found() {
        let dir = temp_dir("revoke");
        fs::write(dir.join("p.html"), "<p>hi</p>").unwrap();
        assert_eq!(serve("nope/p.html").status, 404);
        let g = grant(&dir, "p.html");
        assert!(renew(&g.grant_id));
        revoke(&g.grant_id);
        assert!(!renew(&g.grant_id));
        assert_eq!(serve(&format!("{}/p.html", g.grant_id)).status, 404);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn denied_directory_is_never_served() {
        let dir = temp_dir("deny");
        fs::create_dir(dir.join("data")).unwrap();
        fs::write(dir.join("index.html"), "<html></html>").unwrap();
        fs::write(dir.join("data/db.sqlite"), "x").unwrap();
        let opts = IssueOptions { base: "https://h/api/artifact/", net: false, version: None, data_dir: None, deny: Some(dir.join("data")) };
        let g = issue(&dir.join("index.html").to_string_lossy(), opts).unwrap();
        assert_eq!(serve(&format!("{}/index.html", g.grant_id)).status, 200);
        assert_eq!(serve(&format!("{}/data/db.sqlite", g.grant_id)).status, 404);
        let opts = IssueOptions { base: "https://h/api/artifact/", net: false, version: None, data_dir: None, deny: Some(dir.join("data")) };
        assert!(issue(&dir.join("data/db.sqlite").to_string_lossy(), opts).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_bases_the_policy_cannot_quote() {
        let dir = temp_dir("base");
        fs::write(dir.join("p.html"), "<p></p>").unwrap();
        let path = dir.join("p.html").to_string_lossy().into_owned();
        for base in ["javascript:x/", "https://h/a", "https://h/; script-src *;/", "https://h/a b/", "file:///x/"] {
            let opts = IssueOptions { base, net: false, version: None, data_dir: None, deny: None };
            assert!(issue(&path, opts).is_err(), "{base}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn network_policy_follows_the_grant() {
        let policy = csp("vlxart://localhost/g/", true);
        assert!(policy.contains("connect-src vlxart://localhost/g/ data: blob: https: wss:;"));
        assert!(policy.contains("object-src 'none'"));
    }

    #[test]
    fn bootstrap_keeps_the_doctype_first() {
        let cases = [
            ("<!DOCTYPE html>\n<HTML lang=en><HEAD><meta charset=utf-8>", "<HEAD><meta http-equiv"),
            ("<!doctype html><header>x</header>", "<!doctype html><meta http-equiv"),
            ("<p>bare</p>", "<meta http-equiv"),
            ("<html><body>no head</body></html>", "<html><meta http-equiv"),
        ];
        for (input, expect) in cases {
            let out = String::from_utf8(inject_bootstrap(input.as_bytes(), "default-src 'none'")).unwrap();
            assert!(out.contains(expect), "{input} -> {out}");
            assert!(out.contains("content=\"default-src 'none'\"><script>"), "{out}");
        }
        assert!(String::from_utf8(inject_bootstrap(b"<p>bare</p>", "x")).unwrap().starts_with("<meta"));
    }

    #[test]
    fn snapshots_dedupe_prune_and_restore() {
        let dir = temp_dir("snap");
        let data = dir.join("data");
        let page = dir.join("p.html");
        let path = page.to_string_lossy().into_owned();
        fs::write(&page, "v1").unwrap();
        assert_eq!(snapshot(&data, &path).unwrap().len(), 1);
        assert_eq!(snapshot(&data, &path).unwrap().len(), 1, "unchanged content is not a new version");
        for i in 2..=(MAX_VERSIONS + 3) {
            fs::write(&page, format!("v{i}")).unwrap();
            snapshot(&data, &path).unwrap();
        }
        let list = versions(&data, &path).unwrap();
        assert_eq!(list.len(), MAX_VERSIONS);
        assert_eq!(list.last().unwrap().version, (MAX_VERSIONS + 3) as u32);
        let oldest = list[0].version;
        restore(&data, &path, oldest).unwrap();
        assert_eq!(fs::read_to_string(&page).unwrap(), format!("v{oldest}"));
        let opts = IssueOptions { base: "vlxart://localhost/", net: false, version: Some(oldest), data_dir: Some(&data), deny: None };
        let g = issue(&path, opts).unwrap();
        fs::write(&page, "current").unwrap();
        let served = String::from_utf8(serve(&format!("{}/p.html", g.grant_id)).body).unwrap();
        assert!(served.ends_with(&format!("v{oldest}")), "{served}");
        assert!(restore(&data, &path, 9999).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
