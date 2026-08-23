//! Static-asset serving for the browser remote-access service: cache validators, cache-control
//! policy, and on-demand compression backed by an in-memory cache.
//!
//! Why this exists: assets used to be returned raw with nothing but a Content-Type. The entry bundle
//! is a little over 2 MB, so every visit pushed 2 MB across the wire, and with no validator the
//! browser could not reuse anything on the next visit either. Over a VPN link measured at ~60 KB/s
//! that meant roughly 38 seconds of blank page, repeated on every reload.
//!
//! Two fixes, both handled here:
//!
//! 1. Compression. Text-like assets are compressed on first request and the result is kept in
//!    memory, so each asset is compressed at most once per process. Brotli quality 9 is the chosen
//!    operating point: on the entry bundle it produced 560 KB in 0.06 s, whereas quality 11 produced
//!    512 KB but took 2.10 s — 35x the CPU to save 48 KB.
//! 2. Validators and cache policy. Every response carries a strong ETag derived from the embedded
//!    asset's SHA-256, and `If-None-Match` is answered with 304. Vite emits content-hashed filenames
//!    under `assets/`, which are therefore served `immutable` with a one-year lifetime; `index.html`
//!    and other unhashed files must be revalidated instead, since their names stay constant while
//!    their contents change.
//!
//! Memory: only requested assets are compressed, and the asset set is fixed at build time, so the
//! cache is bounded by the compressed size of `dist` (well under its ~32 MB raw size in practice).

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use super::Assets;

/// Brotli quality. See the module docs for the measurements behind this value.
const BROTLI_QUALITY: i32 = 9;
/// Brotli window size (log2). 22 is the standard maximum that browsers accept.
const BROTLI_WINDOW: i32 = 22;
/// Gzip level for the fallback codec; 9 costs measurably more CPU for well under 1% extra savings.
const GZIP_LEVEL: u32 = 6;
/// Assets below this size are sent as-is: framing overhead cancels out most of the gain, and the
/// transfer is dominated by round-trip latency rather than payload size.
const MIN_COMPRESS_BYTES: usize = 1024;

/// A content coding this server can produce.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Encoding {
    /// No transformation; the asset is sent exactly as embedded.
    Identity,
    Gzip,
    Brotli,
}

impl Encoding {
    /// The `Content-Encoding` token, also used to distinguish ETags between codings.
    fn token(self) -> &'static str {
        match self {
            Encoding::Identity => "identity",
            Encoding::Gzip => "gzip",
            Encoding::Brotli => "br",
        }
    }
}

/// One compressed representation, keyed by the source hash so a changed source invalidates it.
/// Debug builds read `dist` from disk, where files genuinely do change while the process runs.
struct CachedEncoding {
    /// SHA-256 of the *uncompressed* asset this was produced from.
    source_hash: [u8; 32],
    bytes: Vec<u8>,
}

/// Process-wide compressed-representation cache, keyed by request path and coding.
fn cache() -> &'static Mutex<HashMap<(String, Encoding), CachedEncoding>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, Encoding), CachedEncoding>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether this file type benefits from compression. Media and font formats are already compressed,
/// and running them through brotli costs CPU while typically making them slightly larger.
fn is_compressible(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "html" | "js" | "mjs" | "css" | "json" | "map" | "svg" | "txt" | "xml" | "wasm"
    )
}

/// Whether the path is a Vite content-hashed build artifact, whose bytes can never change under the
/// same name and which is therefore safe to cache permanently.
fn is_immutable_asset(path: &str) -> bool {
    path.starts_with("assets/")
}

/// Pick the best coding the client accepts, preferring brotli. Returns `Identity` when the header is
/// absent, unparseable, or lists nothing this server produces.
///
/// Handles the parts of RFC 9110 §12.5.3 that matter in practice: comma-separated tokens, optional
/// `q=` weights, and `q=0` meaning "not acceptable". Anything more exotic degrades to Identity,
/// which is always a valid response.
fn negotiate(headers: &HeaderMap) -> Encoding {
    let Some(raw) = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
    else {
        return Encoding::Identity;
    };

    let mut brotli_ok = false;
    let mut gzip_ok = false;
    for part in raw.split(',') {
        let mut bits = part.split(';');
        let token = bits.next().unwrap_or("").trim().to_ascii_lowercase();
        // A weight of zero is an explicit refusal of that coding.
        let refused = bits.any(|p| {
            let p = p.trim().to_ascii_lowercase();
            p.strip_prefix("q=")
                .and_then(|q| q.parse::<f32>().ok())
                .is_some_and(|q| q <= 0.0)
        });
        if refused {
            continue;
        }
        match token.as_str() {
            "br" => brotli_ok = true,
            "gzip" => gzip_ok = true,
            // A bare wildcard accepts anything this server can produce.
            "*" => {
                brotli_ok = true;
                gzip_ok = true;
            }
            _ => {}
        }
    }

    if brotli_ok {
        Encoding::Brotli
    } else if gzip_ok {
        Encoding::Gzip
    } else {
        Encoding::Identity
    }
}

/// Compress `data` with `encoding`, returning None if the codec fails or the result is not actually
/// smaller — in which case the caller sends the original bytes.
fn compress(data: &[u8], encoding: Encoding) -> Option<Vec<u8>> {
    let out = match encoding {
        Encoding::Identity => return None,
        Encoding::Brotli => {
            let params = brotli::enc::BrotliEncoderParams {
                quality: BROTLI_QUALITY,
                lgwin: BROTLI_WINDOW,
                size_hint: data.len(),
                ..Default::default()
            };
            let mut out = Vec::new();
            brotli::BrotliCompress(&mut &data[..], &mut out, &params).ok()?;
            out
        }
        Encoding::Gzip => {
            let mut enc =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(GZIP_LEVEL));
            enc.write_all(data).ok()?;
            enc.finish().ok()?
        }
    };
    (out.len() < data.len()).then_some(out)
}

/// Return the compressed representation for `path`, compressing and caching on first use.
///
/// Runs on the caller's thread and is therefore expected to be called from a blocking context; see
/// `serve`, which dispatches it through `spawn_blocking`.
fn compressed(path: &str, encoding: Encoding, data: &[u8], hash: [u8; 32]) -> Option<Vec<u8>> {
    let key = (path.to_owned(), encoding);
    if let Some(hit) = cache().lock().ok()?.get(&key) {
        if hit.source_hash == hash {
            return Some(hit.bytes.clone());
        }
    }

    let bytes = compress(data, encoding)?;
    if let Ok(mut guard) = cache().lock() {
        guard.insert(
            key,
            CachedEncoding {
                source_hash: hash,
                bytes: bytes.clone(),
            },
        );
    }
    Some(bytes)
}

/// Build the strong ETag for one representation. The coding is part of the tag because gzip and
/// brotli renderings of the same asset are different representations; `Vary: Accept-Encoding` keeps
/// intermediaries from mixing them up.
fn etag_for(hash: &[u8; 32], encoding: Encoding) -> String {
    // Half of SHA-256 is far beyond enough to distinguish build artifacts and keeps the header short.
    let hex: String = hash[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}-{}\"", encoding.token())
}

/// Whether `If-None-Match` matches this ETag, per RFC 9110 §13.1.2 (`*` matches any representation).
fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    let Some(raw) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    raw.split(',').any(|candidate| {
        let candidate = candidate.trim();
        // Weak validators compare equal to their strong counterpart for this purpose.
        let candidate = candidate.strip_prefix("W/").unwrap_or(candidate);
        candidate == "*" || candidate == etag
    })
}

/// Cache-Control for a path: content-hashed artifacts are immutable for a year, everything else must
/// be revalidated on each visit so a redeployed frontend is picked up immediately.
fn cache_control(path: &str) -> &'static str {
    if is_immutable_asset(path) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// Serve one embedded asset with negotiation, validators, and cache policy applied.
///
/// `request_path` is the embedded-asset key to read; `policy_path` is the path whose cache policy
/// applies. They differ for the SPA fallback, which serves `index.html`'s bytes for an arbitrary
/// route and must apply `index.html`'s revalidate policy rather than the requested route's.
pub(crate) async fn serve(
    request_path: &str,
    policy_path: &str,
    mime: &'static str,
    headers: &HeaderMap,
) -> Option<Response> {
    let asset = Assets::get(request_path)?;
    let hash = asset.metadata.sha256_hash();
    let data = asset.data.into_owned();

    let mut encoding = negotiate(headers);
    if encoding != Encoding::Identity
        && (!is_compressible(request_path) || data.len() < MIN_COMPRESS_BYTES)
    {
        encoding = Encoding::Identity;
    }

    // Answer conditional requests before spending any CPU on compression.
    let etag = etag_for(&hash, encoding);
    let cc = cache_control(policy_path);
    if etag_matches(headers, &etag) {
        return Some(not_modified(&etag, cc));
    }

    let body = if encoding == Encoding::Identity {
        data
    } else {
        // Compression is CPU-bound and must not run on a Tokio worker; a 2 MB bundle takes ~60 ms.
        let path = request_path.to_owned();
        let compressed_body = tokio::task::spawn_blocking(move || {
            let out = compressed(&path, encoding, &data, hash);
            (out, data)
        })
        .await
        .ok()?;
        match compressed_body {
            (Some(bytes), _) => bytes,
            // Compression failed or did not pay off: fall back to the original representation, which
            // needs the identity ETag so the validator still describes what was actually sent.
            (None, original) => {
                let etag = etag_for(&hash, Encoding::Identity);
                if etag_matches(headers, &etag) {
                    return Some(not_modified(&etag, cc));
                }
                return Some(build(original, mime, Encoding::Identity, &etag, cc));
            }
        }
    };

    Some(build(body, mime, encoding, &etag, cc))
}

/// A 304 carrying the validator and policy headers required for the client to refresh its entry.
fn not_modified(etag: &str, cache_control: &str) -> Response {
    let mut res = StatusCode::NOT_MODIFIED.into_response();
    let h = res.headers_mut();
    insert(h, header::ETAG, etag);
    insert(h, header::CACHE_CONTROL, cache_control);
    insert(h, header::VARY, "Accept-Encoding");
    res
}

/// Assemble the full response for one representation.
fn build(
    body: Vec<u8>,
    mime: &'static str,
    encoding: Encoding,
    etag: &str,
    cache_control: &str,
) -> Response {
    let mut res = body.into_response();
    let h = res.headers_mut();
    insert(h, header::CONTENT_TYPE, mime);
    insert(h, header::ETAG, etag);
    insert(h, header::CACHE_CONTROL, cache_control);
    // Required whenever the body depends on Accept-Encoding, so a shared cache cannot hand a brotli
    // body to a client that never asked for one.
    insert(h, header::VARY, "Accept-Encoding");
    if encoding != Encoding::Identity {
        insert(h, header::CONTENT_ENCODING, encoding.token());
    }
    res
}

/// Set a header, silently skipping values that cannot be encoded. Every value here is ASCII built by
/// this module, so the fallible path is unreachable in practice.
fn insert(headers: &mut HeaderMap, name: header::HeaderName, value: &str) {
    if let Ok(v) = HeaderValue::from_str(value) {
        headers.insert(name, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (name, value) in pairs {
            h.insert(name.clone(), HeaderValue::from_str(value).unwrap());
        }
        h
    }

    #[test]
    fn negotiate_prefers_brotli_then_gzip() {
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "gzip, deflate, br")])),
            Encoding::Brotli
        );
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "gzip, deflate")])),
            Encoding::Gzip
        );
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "deflate")])),
            Encoding::Identity
        );
    }

    #[test]
    fn negotiate_without_header_is_identity() {
        assert_eq!(negotiate(&HeaderMap::new()), Encoding::Identity);
    }

    #[test]
    fn negotiate_honors_zero_weight_refusal() {
        // br is listed but explicitly refused, so gzip must win rather than br.
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "br;q=0, gzip")])),
            Encoding::Gzip
        );
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "br;q=0.9, gzip")])),
            Encoding::Brotli
        );
    }

    #[test]
    fn negotiate_accepts_wildcard() {
        assert_eq!(
            negotiate(&headers(&[(header::ACCEPT_ENCODING, "*")])),
            Encoding::Brotli
        );
    }

    #[test]
    fn compressible_covers_text_but_not_media() {
        assert!(is_compressible("assets/index-abc.js"));
        assert!(is_compressible("index.html"));
        assert!(is_compressible("velaterm-dark.svg"));
        assert!(!is_compressible("assets/font-abc.woff2"));
        assert!(!is_compressible("icon.png"));
    }

    #[test]
    fn hashed_assets_are_immutable_and_others_revalidate() {
        assert_eq!(
            cache_control("assets/index-CuuwpZcZ.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("velaterm-dark.svg"), "no-cache");
    }

    #[test]
    fn etag_distinguishes_encodings_of_one_asset() {
        let hash = [7u8; 32];
        assert_ne!(
            etag_for(&hash, Encoding::Brotli),
            etag_for(&hash, Encoding::Identity)
        );
        // Same bytes and same coding must always yield the same validator.
        assert_eq!(
            etag_for(&hash, Encoding::Brotli),
            etag_for(&hash, Encoding::Brotli)
        );
    }

    #[test]
    fn if_none_match_matches_exact_and_wildcard() {
        let etag = etag_for(&[1u8; 32], Encoding::Brotli);
        assert!(etag_matches(
            &headers(&[(header::IF_NONE_MATCH, &etag)]),
            &etag
        ));
        assert!(etag_matches(&headers(&[(header::IF_NONE_MATCH, "*")]), &etag));
        // A weak validator for the same representation still counts as a match.
        assert!(etag_matches(
            &headers(&[(header::IF_NONE_MATCH, &format!("W/{etag}"))]),
            &etag
        ));
        assert!(!etag_matches(
            &headers(&[(header::IF_NONE_MATCH, "\"other\"")]),
            &etag
        ));
        assert!(!etag_matches(&HeaderMap::new(), &etag));
    }

    #[test]
    fn compression_shrinks_repetitive_text() {
        let data = "export const value = 1;\n".repeat(500).into_bytes();
        for encoding in [Encoding::Brotli, Encoding::Gzip] {
            let out = compress(&data, encoding).expect("compressible input must shrink");
            assert!(out.len() < data.len());
        }
    }

    #[test]
    fn compression_declines_incompressible_input() {
        // High-entropy bytes cannot shrink, so the caller must fall back to identity. The generator is
        // an LCG read from its top byte, which carries none of the low-bit periodicity that would let
        // a compressor find structure here.
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let data: Vec<u8> = (0..65536)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (state >> 56) as u8
            })
            .collect();
        assert!(compress(&data, Encoding::Gzip).is_none());
        assert!(compress(&data, Encoding::Brotli).is_none());
    }

    #[test]
    fn cache_recompresses_when_source_hash_changes() {
        let first = b"const a = 1;".repeat(200);
        let second = b"const b = 2;".repeat(200);
        let path = "test/cache-invalidation.js";

        let a = compressed(path, Encoding::Gzip, &first, [1u8; 32]).unwrap();
        // Same key, same hash: must come back from cache unchanged.
        let a_again = compressed(path, Encoding::Gzip, &first, [1u8; 32]).unwrap();
        assert_eq!(a, a_again);
        // New hash means the file changed on disk (debug builds), so the entry must be rebuilt.
        let b = compressed(path, Encoding::Gzip, &second, [2u8; 32]).unwrap();
        assert_ne!(a, b);
    }
}
