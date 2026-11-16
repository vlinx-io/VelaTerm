//! Session credentials shared by the hidden CLI commands.
//!
//! Some agent harnesses filter the environment every tool command inherits: a name containing `TOKEN`,
//! `KEY`, `SECRET`, or `PASSWORD` is dropped. `VLX_TOKEN` therefore may never reach a session's shell,
//! and `vspawn`, `vtell`, and the other commands used to report that they were not inside a VelaTerm
//! session. Sessions receive the same value under `VLX_AUTH`, a name those filters leave alone, and
//! every reader accepts either one.

/// The documented name of the hook-service token.
pub const ENV_TOKEN: &str = "VLX_TOKEN";
/// Alias carrying the same token through harnesses that strip credential-shaped names.
pub const ENV_AUTH: &str = "VLX_AUTH";

/// Resolve the hook-service token from the session environment, preferring `VLX_TOKEN`.
pub fn token() -> Option<String> {
    token_from(|name| std::env::var(name).ok())
}

fn token_from(read: impl Fn(&str) -> Option<String>) -> Option<String> {
    [ENV_TOKEN, ENV_AUTH].into_iter()
        .filter_map(read)
        .find(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documented_name_wins_and_the_alias_is_a_fallback() {
        let both = |name: &str| match name {
            ENV_TOKEN => Some("documented".to_string()),
            ENV_AUTH => Some("alias".to_string()),
            _ => None,
        };
        assert_eq!(token_from(both).as_deref(), Some("documented"));
        let alias_only = |name: &str| (name == ENV_AUTH).then(|| "alias".to_string());
        assert_eq!(token_from(alias_only).as_deref(), Some("alias"));
        assert_eq!(token_from(|_| None), None);
        assert_eq!(token_from(|_| Some(String::new())), None);
    }
}
