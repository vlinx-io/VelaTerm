//! Which VelaTerm service the app talks to: the international site (velaterm.com with downloads on
//! dl.velaterm.com) or the mainland China mirror (velaterm.cn with downloads on dl.velaterm.cn).
//!
//! The China site is a reverse-proxy mirror of velaterm.com and dl.velaterm.cn mirrors the release bucket
//! under the same keys, so both regions serve identical content. Everything fetched from either one is
//! verified the same way (minisign signatures, SHA-256), so the region only changes where bytes come from.
//!
//! The choice is the cross-shell preference `vlx-server-region` in app_settings. It is mirrored in an atomic
//! here so main-thread callers such as the native menu can read it without touching SQLite: it is loaded
//! once when the database opens and updated whenever `set_app_settings` writes the key.
//!
//! Account features (sign-in, Remote, public sharing, mobile push) stay on velaterm.com in either region:
//! their sessions and relay tunnels are bound to that origin.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::db::{repo, Db};

/// app_settings / localStorage key holding `"global"` or `"cn"`.
pub const SETTING_KEY: &str = "vlx-server-region";

const GLOBAL_DL: &str = "https://dl.velaterm.com/";
const CHINA_DL: &str = "https://dl.velaterm.cn/";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Global,
    China,
}

impl Region {
    /// Parse the stored value; anything else is rejected so a typo cannot silently pick a region.
    pub fn parse(value: &str) -> Option<Region> {
        match value {
            "global" => Some(Region::Global),
            "cn" => Some(Region::China),
            _ => None,
        }
    }

    /// Website and API origin, without a trailing slash.
    pub fn site_origin(self) -> &'static str {
        match self {
            Region::Global => "https://velaterm.com",
            Region::China => "https://velaterm.cn",
        }
    }

    /// Release download root, without a trailing slash.
    pub fn dl_base(self) -> &'static str {
        match self {
            Region::Global => "https://dl.velaterm.com",
            Region::China => "https://dl.velaterm.cn",
        }
    }

    /// Point a download URL published on dl.velaterm.com at this region's mirror. Manifests always carry
    /// dl.velaterm.com addresses; other URLs are returned unchanged.
    pub fn localize_download_url(self, url: &str) -> String {
        match (self, url.strip_prefix(GLOBAL_DL)) {
            (Region::China, Some(rest)) => format!("{CHINA_DL}{rest}"),
            _ => url.to_string(),
        }
    }
}

/// 0 = global, 1 = China.
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The region currently selected. Pure memory read; safe on any thread.
pub fn current() -> Region {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Region::China,
        _ => Region::Global,
    }
}

pub fn set(region: Region) {
    CURRENT.store(if region == Region::China { 1 } else { 0 }, Ordering::Relaxed);
}

/// Load the saved choice after the database opens. A missing or unreadable value keeps the default.
pub fn load(db: &Db) {
    let Ok(conn) = db.conn.lock() else { return };
    if let Some(region) = repo::get_app_settings(&conn)
        .ok()
        .and_then(|map| map.get(SETTING_KEY).and_then(|v| Region::parse(v)))
    {
        set(region);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_known_values() {
        assert_eq!(Region::parse("global"), Some(Region::Global));
        assert_eq!(Region::parse("cn"), Some(Region::China));
        assert_eq!(Region::parse("CN"), None);
        assert_eq!(Region::parse(""), None);
    }

    #[test]
    fn localizes_only_release_urls_in_china() {
        let url = "https://dl.velaterm.com/0.3.0/VelaTerm_aarch64.app.tar.gz";
        assert_eq!(Region::China.localize_download_url(url), "https://dl.velaterm.cn/0.3.0/VelaTerm_aarch64.app.tar.gz");
        assert_eq!(Region::Global.localize_download_url(url), url);
        let other = "https://github.com/vlinx-io/VelaTerm/releases/x.dmg";
        assert_eq!(Region::China.localize_download_url(other), other);
        let lookalike = "https://dl.velaterm.com.evil.test/x";
        assert_eq!(Region::China.localize_download_url(lookalike), lookalike);
    }
}
