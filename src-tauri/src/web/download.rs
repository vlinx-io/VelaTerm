//! Ticketed file downloads: a short-lived URL the browser's own downloader can fetch.
//!
//! Everything else in this server travels over the authenticated WebSocket, but a download must not. Handing
//! bytes to JavaScript means JavaScript has to hold them: the page can only assemble the file in memory and
//! then offer it, which caps downloads at whatever the tab can hold and gives no speed readout, no pause, and
//! no resume. A plain HTTP response gives all of that for free, in every browser, because the browser's
//! download manager does the work.
//!
//! The obstacle was authentication. This server dropped cookies (see `auth.rs`), so credentials ride in an
//! `Authorization` header — and a browser navigating to a URL sends no such header. The fix is the same one
//! `/ws` already uses for the same reason: put a token in the URL. Not the session token, which is long-lived
//! and would end up in browser history and download records, but a ticket minted for one path, valid for a few
//! minutes.
//!
//! The ticket is issued over the authenticated WebSocket (`create_download_ticket` in `dispatch.rs`), where
//! the caller's origin is known and the remote data-directory ACL applies. By the time the URL is fetched, the
//! only thing it can do is read the one file it was minted for.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::RawQuery;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

/// How long a ticket stays usable.
///
/// It covers the gap between the click and the browser starting the request, plus a retry or two if the
/// connection drops early. It is not meant to cover a long download: the response body streams under the
/// single request that opened it, and a transfer already in flight is unaffected when its ticket expires.
/// A pause-and-resume hours later asks for a fresh URL, which is one more click.
const TICKET_TTL: Duration = Duration::from_secs(300);

/// Bytes per read while streaming. Large enough to keep syscalls down, small enough that a slow client does
/// not pin much memory per connection.
const STREAM_CHUNK: usize = 64 * 1024;

struct Ticket {
    path: PathBuf,
    issued: Instant,
}

fn tickets() -> &'static Mutex<HashMap<String, Ticket>> {
    static TICKETS: OnceLock<Mutex<HashMap<String, Ticket>>> = OnceLock::new();
    TICKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Mint a download URL path for one file. The caller has already applied the remote-path ACL.
///
/// Returns the query-carrying path rather than a bare token so the frontend never has to know the URL shape.
pub fn issue(path: &str) -> String {
    let token = uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string();
    if let Ok(mut map) = tickets().lock() {
        // Sweep on issue: tickets are few and short-lived, so this costs nothing and avoids a timer thread.
        map.retain(|_, t| t.issued.elapsed() < TICKET_TTL);
        map.insert(
            token.clone(),
            Ticket {
                path: PathBuf::from(path),
                issued: Instant::now(),
            },
        );
    }
    format!("/api/download?token={token}")
}

/// Resolve a ticket without consuming it.
///
/// Deliberately not single-use: a browser may retry the request after an early network hiccup, and a paused
/// download resumes with a fresh `Range` request against the same URL. Both are the same user fetching the
/// same file, and the short TTL is what bounds the exposure.
fn resolve(token: &str) -> Option<PathBuf> {
    let mut map = tickets().lock().ok()?;
    map.retain(|_, t| t.issued.elapsed() < TICKET_TTL);
    map.get(token).map(|t| t.path.clone())
}

/// Extract one query parameter's raw value.
fn query_value(query: Option<&str>, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    query?
        .split('&')
        .find_map(|pair| pair.strip_prefix(prefix.as_str()).map(str::to_string))
}

/// One byte range resolved against a known file size: inclusive start and end.
struct ByteRange {
    start: u64,
    end: u64,
}

/// Parse a single `bytes=` range against the file size.
///
/// Only one range is honoured; multipart ranges exist in the spec but no download manager needs them, and
/// answering a multi-range request with the whole file is a legal fallback. Returns None when the header is
/// absent or unusable (send the whole file), and Err when it is syntactically fine but unsatisfiable.
#[allow(clippy::result_unit_err)]
fn parse_range(headers: &HeaderMap, size: u64) -> Result<Option<ByteRange>, ()> {
    let Some(raw) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        return Ok(None);
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return Ok(None);
    };
    if spec.contains(',') {
        return Ok(None); // Multi-range: answer with the full body instead.
    }
    let (from, to) = spec.split_once('-').ok_or(())?;
    let (start, end) = match (from.trim(), to.trim()) {
        // `bytes=-N`: the last N bytes.
        ("", suffix) => {
            let n: u64 = suffix.parse().map_err(|_| ())?;
            if n == 0 {
                return Err(());
            }
            (size.saturating_sub(n), size.saturating_sub(1))
        }
        (start, "") => (start.parse().map_err(|_| ())?, size.saturating_sub(1)),
        (start, end) => (
            start.parse().map_err(|_| ())?,
            end.parse::<u64>().map_err(|_| ())?.min(size.saturating_sub(1)),
        ),
    };
    if size == 0 || start > end || start >= size {
        return Err(());
    }
    Ok(Some(ByteRange { start, end }))
}

/// Build a `Content-Disposition` value that names the file safely.
///
/// Two forms are emitted because they cover different clients: a quoted ASCII fallback with everything
/// unusual stripped, and RFC 5987's `filename*` carrying the real UTF-8 name percent-encoded. Header
/// injection is the thing to prevent here — a file named with a CR or LF would otherwise let its own name
/// forge response headers — and both forms are built from filtered characters only.
fn content_disposition(name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '[' | ']') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let ascii = if ascii.trim().is_empty() { "download".to_string() } else { ascii };
    let mut encoded = String::new();
    for b in name.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
            encoded.push(*b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

/// Serve the file a ticket names, honouring `Range` so downloads can pause, resume, and retry.
pub async fn handler(RawQuery(query): RawQuery, headers: HeaderMap) -> Response {
    let Some(token) = query_value(query.as_deref(), "token") else {
        return (StatusCode::UNAUTHORIZED, "Missing download token").into_response();
    };
    // An expired or unknown ticket is indistinguishable from a forged one, and both get the same answer.
    let Some(path) = resolve(&token) else {
        return (StatusCode::UNAUTHORIZED, "Invalid or expired download token").into_response();
    };

    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) if m.is_file() => m,
        Ok(_) => return (StatusCode::NOT_FOUND, "Not a regular file").into_response(),
        Err(e) => return (StatusCode::NOT_FOUND, format!("Failed to open file: {e}")).into_response(),
    };
    let size = meta.len();

    let range = match parse_range(&headers, size) {
        Ok(r) => r,
        Err(()) => {
            // 416 must state the current size so the client can retry with a range that fits.
            let mut res = (StatusCode::RANGE_NOT_SATISFIABLE, "Range not satisfiable").into_response();
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
                res.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            return res;
        }
    };

    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => return (StatusCode::NOT_FOUND, format!("Failed to open file: {e}")).into_response(),
    };
    let (start, length) = match &range {
        Some(r) => (r.start, r.end - r.start + 1),
        None => (0, size),
    };
    if start > 0 {
        use tokio::io::AsyncSeekExt;
        if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to seek file: {e}")).into_response();
        }
    }

    // Stream the body: the file is read as the client drains it, so a multi-gigabyte download never
    // materialises in this process's memory any more than it does in the browser's.
    use tokio::io::AsyncReadExt;
    let stream = tokio_util::io::ReaderStream::with_capacity(file.take(length), STREAM_CHUNK);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let status = if range.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    let mut res = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_DISPOSITION, content_disposition(&name))
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    if let Some(r) = &range {
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {}-{}/{}", r.start, r.end, size)) {
            res.headers_mut().insert(header::CONTENT_RANGE, v);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::{content_disposition, issue, parse_range, query_value, resolve};
    use axum::http::{header, HeaderMap, HeaderValue};

    fn range_headers(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::RANGE, HeaderValue::from_str(v).unwrap());
        h
    }

    #[test]
    fn a_ticket_resolves_to_its_path_and_nothing_else() {
        let url = issue("/tmp/some-file.bin");
        let token = url.strip_prefix("/api/download?token=").unwrap();
        assert_eq!(resolve(token).unwrap().to_string_lossy(), "/tmp/some-file.bin");
        // A ticket is bound to one path, so a forged token resolves to nothing at all.
        assert!(resolve("not-a-real-token").is_none());
        // Resolving does not consume it: a browser retry or a resumed range must still work.
        assert!(resolve(token).is_some());
    }

    #[test]
    fn ranges_are_parsed_against_the_file_size() {
        // No header: send everything.
        assert!(parse_range(&HeaderMap::new(), 100).unwrap().is_none());
        let r = parse_range(&range_headers("bytes=10-19"), 100).unwrap().unwrap();
        assert_eq!((r.start, r.end), (10, 19));
        // Open-ended range runs to the last byte.
        let r = parse_range(&range_headers("bytes=90-"), 100).unwrap().unwrap();
        assert_eq!((r.start, r.end), (90, 99));
        // Suffix range counts back from the end.
        let r = parse_range(&range_headers("bytes=-10"), 100).unwrap().unwrap();
        assert_eq!((r.start, r.end), (90, 99));
        // An end past the file is clamped rather than rejected, which is what clients expect.
        let r = parse_range(&range_headers("bytes=50-999"), 100).unwrap().unwrap();
        assert_eq!((r.start, r.end), (50, 99));
        // Starting at or past the end is unsatisfiable, and so is a range on an empty file.
        assert!(parse_range(&range_headers("bytes=100-"), 100).is_err());
        assert!(parse_range(&range_headers("bytes=0-"), 0).is_err());
        // Garbage and multi-range both fall back to the whole file rather than failing the download.
        assert!(parse_range(&range_headers("items=1-2"), 100).unwrap().is_none());
        assert!(parse_range(&range_headers("bytes=0-1,5-6"), 100).unwrap().is_none());
    }

    #[test]
    fn the_file_name_cannot_forge_response_headers() {
        // A name carrying CR/LF must not survive into the header in any form.
        let v = content_disposition("evil\r\nX-Injected: 1.txt");
        assert!(!v.contains('\r') && !v.contains('\n'), "got: {v}");
        // Non-ASCII names keep their real spelling in the RFC 5987 form and degrade in the fallback.
        let v = content_disposition("报告.pdf");
        assert!(v.contains("filename*=UTF-8''"), "got: {v}");
        assert!(v.contains("filename=\"__.pdf\""), "got: {v}");
        // An empty or blank name still yields something a client can save under.
        assert!(content_disposition("").contains("filename=\"download\""));
        assert!(content_disposition("   ").contains("filename=\"download\""));
    }

    #[test]
    fn the_token_is_read_from_anywhere_in_the_query() {
        assert_eq!(query_value(Some("token=abc"), "token").unwrap(), "abc");
        assert_eq!(query_value(Some("a=1&token=abc&b=2"), "token").unwrap(), "abc");
        assert!(query_value(Some("a=1"), "token").is_none());
        assert!(query_value(None, "token").is_none());
    }
}
