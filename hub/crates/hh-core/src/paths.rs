//! Server-side path safety. Hard rules (AGENTS.md §2.5, SECURITY §10):
//! never trust client paths — canonicalize, jail to root, reject traversal
//! and reserved Windows names.

use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Windows reserved device names (case-insensitive, with or without extension).
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const MAX_COMPONENT_LEN: usize = 200;
const MAX_REL_PATH_LEN: usize = 900; // stay under 260-with-prefix / \\?\ limits

/// Validate a single path component received from a client.
pub fn sanitize_component(raw: &str) -> Result<String> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return Err(Error::BadRequest("empty path component".into()));
    }
    // Reject path separators, ADS colon, control chars, and angle/pipe/quote/wildcards.
    if s.chars().any(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(Error::BadRequest(format!("illegal characters in name: {raw:?}")));
    }
    // Trailing dots and spaces are stripped by Windows → ambiguity; reject.
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() {
        return Err(Error::BadRequest("name reduces to empty".into()));
    }
    let stem = s.split('.').next().unwrap_or(&s).to_uppercase();
    if RESERVED_NAMES.contains(&stem.as_str()) {
        return Err(Error::BadRequest(format!("reserved name: {raw}")));
    }
    if s == "." || s == ".." {
        return Err(Error::BadRequest("traversal component".into()));
    }
    if s.len() > MAX_COMPONENT_LEN {
        return Err(Error::TooLarge(format!("path component too long: {raw:?}")));
    }
    Ok(s)
}

/// Validate and normalize a client-supplied relative path ("a/b/c"). Returns
/// forward-slash relative path with every component sanitized.
pub fn sanitize_rel_path(raw: &str) -> Result<String> {
    let mut parts = Vec::new();
    for part in raw.split(['/', '\\']) {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(Error::BadRequest("path traversal rejected".into()));
        }
        parts.push(sanitize_component(part)?);
    }
    let joined = parts.join("/");
    if joined.len() > MAX_REL_PATH_LEN {
        return Err(Error::TooLarge("relative path too long".into()));
    }
    Ok(joined)
}

/// Join a sanitized relative path onto a root and verify the result stays
/// inside the root (defense in depth; components are already sanitized).
pub fn jail_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(Error::BadRequest("absolute path rejected".into()));
    }
    let mut out = root.to_path_buf();
    for comp in rel_path.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            _ => return Err(Error::BadRequest("non-normal component rejected".into())),
        }
    }
    if !out.starts_with(root) {
        return Err(Error::BadRequest("path escapes root".into()));
    }
    // Resolve every existing ancestor, including symlinks/junctions. A new
    // destination may not exist yet, so validate its nearest existing parent.
    let canonical_root = root.canonicalize()?;
    let mut existing = out.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| Error::BadRequest("invalid destination".into()))?;
    }
    if !existing.canonicalize()?.starts_with(&canonical_root) {
        return Err(Error::BadRequest("path escapes root".into()));
    }
    Ok(out)
}

/// Map MIME type to a library category (TRD §7.1).
pub fn category_for_mime(mime: Option<&str>) -> &'static str {
    let mime = mime.unwrap_or("");
    if mime.starts_with("image/") {
        "photo"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        "music"
    } else if mime.starts_with("text/")
        || mime.contains("pdf")
        || mime.contains("document")
        || mime.contains("spreadsheet")
        || mime.contains("presentation")
        || mime.contains("opendocument")
    {
        "document"
    } else {
        "download"
    }
}

/// Physical directory for a category (TRD §7.1).
pub fn category_dir(category: &str, taken_at_ms: Option<i64>) -> String {
    match category {
        "photo" | "video" => {
            let (y, m) = crate::time::year_month(taken_at_ms.unwrap_or_else(crate::time::now_ms));
            let base = if category == "photo" { "Photos" } else { "Videos" };
            format!("{base}/{y:04}/{m:02}")
        }
        "document" => "Documents".into(),
        "music" => "Music".into(),
        "backup" => "Backups".into(),
        _ => "Downloads".into(),
    }
}

/// Resolve a name collision in `dir` by appending " (n)" before the extension.
pub fn dedupe_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 2..10_000u32 {
        let candidate = format!("{stem} ({n}){ext}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    // Practically unreachable; fall back to a ULID suffix.
    format!("{stem}-{}{ext}", ulid::Ulid::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(sanitize_rel_path("../etc/passwd").is_err());
        assert!(sanitize_rel_path("a/../../b").is_err());
        assert!(sanitize_rel_path("..").is_err());
    }

    #[test]
    fn rejects_windows_reserved() {
        for bad in ["CON", "con", "NUL.txt", "COM1", "lpt3.png"] {
            assert!(sanitize_component(bad).is_err(), "should reject {bad}");
        }
    }

    #[test]
    fn rejects_ads_and_controls() {
        assert!(sanitize_component("file:stream").is_err());
        assert!(sanitize_component("a\\b").is_err());
        assert!(sanitize_component("a\u{0}b").is_err());
        assert!(sanitize_component("name.").is_ok()); // trailing dot stripped
        assert_eq!(sanitize_component("name.").unwrap(), "name");
    }

    #[test]
    fn accepts_unicode_and_emoji() {
        assert_eq!(sanitize_component("शादी 📷.jpg").unwrap(), "शादी 📷.jpg");
        assert_eq!(sanitize_rel_path("family/दीवाली/pic.jpg").unwrap(), "family/दीवाली/pic.jpg");
    }

    #[test]
    fn jail_blocks_escape() {
        let root = Path::new("/lib");
        assert!(jail_join(root, "Photos/2026/10/x.jpg").is_ok());
        assert!(jail_join(root, "/etc/passwd").is_err());
    }

    #[test]
    fn mime_categories() {
        assert_eq!(category_for_mime(Some("image/jpeg")), "photo");
        assert_eq!(category_for_mime(Some("video/quicktime")), "video");
        assert_eq!(category_for_mime(Some("application/pdf")), "document");
        assert_eq!(category_for_mime(Some("audio/mpeg")), "music");
        assert_eq!(category_for_mime(Some("application/zip")), "download");
    }

    #[test]
    fn collision_names() {
        let tmp = std::env::temp_dir().join(format!("hh-test-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.jpg"), b"x").unwrap();
        assert_eq!(dedupe_name(&tmp, "a.jpg"), "a (2).jpg");
        assert_eq!(dedupe_name(&tmp, "b.jpg"), "b.jpg");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
