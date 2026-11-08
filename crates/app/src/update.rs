//! Update check against the latest GitHub release.

use serde::Deserialize;

const LATEST: &str = "https://api.github.com/repos/riclenatin/open-msi-control/releases/latest";
/// Checks at most once a day.
pub const INTERVAL_SECS: u64 = 24 * 3600;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
}

impl Release {
    /// Version without the leading `v`.
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }
}

/// `major.minor.patch` as numbers; pre-release suffixes (`-rc.1`) and missing parts count as 0.
fn parse(v: &str) -> [u64; 3] {
    let core = v.trim_start_matches('v').split(['-', '+']).next().unwrap_or("");
    let mut out = [0; 3];
    for (slot, part) in out.iter_mut().zip(core.split('.')) {
        *slot = part.parse().unwrap_or(0);
    }
    out
}

/// Whether `tag` is newer than `current`.
pub fn is_newer(tag: &str, current: &str) -> bool {
    parse(tag) > parse(current)
}

/// The latest release if it is newer than this build. Blocking; run off the UI thread.
pub fn check() -> std::io::Result<Option<Release>> {
    let (status, body) = crate::platform::http_get(LATEST)?;
    match status {
        200..300 => {}
        // The repository has no published release yet.
        404 => return Ok(None),
        _ => return Err(std::io::Error::other(format!("HTTP {status}"))),
    }
    let release: Release = serde_json::from_slice(&body).map_err(std::io::Error::other)?;
    Ok(is_newer(&release.tag_name, env!("CARGO_PKG_VERSION")).then_some(release))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("v0.10.0", "0.9.3"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
        assert!(!is_newer("v0.2.0-rc.1", "0.2.0"), "a release candidate of the current version is not newer");
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn reads_github_release_json() {
        let json =
            r#"{"tag_name":"v0.2.0","html_url":"https://github.com/x/y/releases/tag/v0.2.0","assets":[]}"#;
        let r: Release = serde_json::from_str(json).expect("valid release JSON");
        assert_eq!(r.version(), "0.2.0");
        assert!(r.html_url.ends_with("/v0.2.0"));
    }
}
