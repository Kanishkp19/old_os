//! Installed app manifest and bilingual launcher search.
use serde::Deserialize;
use std::{fs, path::Path};
use thiserror::Error;

pub const APP_MANIFEST_PATH: &str = "/usr/share/homehub/apps.json";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct App {
    pub id: String,
    pub label: String,
    pub available_in_trial: bool,
}

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    apps: Vec<App>,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("cannot read app manifest: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid app manifest: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported app manifest version {0}")]
    Version(u32),
}

pub fn load_app_manifest() -> Result<Vec<App>, AppError> {
    load_app_manifest_at(Path::new(APP_MANIFEST_PATH))
}

pub fn load_app_manifest_at(path: &Path) -> Result<Vec<App>, AppError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(builtin_apps());
        }
        Err(error) => return Err(error.into()),
    };
    let manifest: Manifest = serde_json::from_str(&source)?;
    if manifest.version != 1 {
        return Err(AppError::Version(manifest.version));
    }
    Ok(manifest.apps)
}

pub fn search<'a>(apps: &'a [App], query: &str) -> Vec<&'a App> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return apps.iter().collect();
    }
    apps.iter()
        .filter(|app| {
            app.label.to_lowercase().contains(&query)
                || app.id.to_lowercase().contains(&query)
                || synonyms(&app.id)
                    .iter()
                    .any(|word| word.to_lowercase().contains(&query))
        })
        .collect()
}

fn synonyms(id: &str) -> &'static [&'static str] {
    match id {
        "photos" => &["pictures", "images", "gallery", "फ़ोटो", "तस्वीरें", "चित्र"],
        "files" => &["documents", "folders", "फ़ाइलें", "दस्तावेज़"],
        "browser" => &["web", "internet", "ब्राउज़र", "इंटरनेट"],
        "youtube" => &["video", "videos", "यूट्यूब", "वीडियो"],
        "music" => &["audio", "songs", "संगीत", "गाने"],
        "notes" => &["memo", "writing", "नोट्स", "नोट"],
        "calculator" => &["calc", "math", "कैलकुलेटर", "गणना"],
        "devices" => &["phone", "connected", "डिवाइस", "फ़ोन"],
        "settings" => &["preferences", "options", "सेटिंग्स", "विकल्प"],
        _ => &[],
    }
}

fn builtin_apps() -> Vec<App> {
    const BUILTIN: [(&str, &str, bool); 9] = [
        ("photos", "Photos", true),
        ("files", "Files", true),
        ("browser", "Browser", false),
        ("youtube", "YouTube", false),
        ("music", "Music", false),
        ("notes", "Notes", true),
        ("calculator", "Calculator", true),
        ("devices", "Devices", false),
        ("settings", "Settings", true),
    ];
    BUILTIN
        .iter()
        .map(|(id, label, available_in_trial)| App {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
            available_in_trial: *available_in_trial,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_and_bilingual_search() {
        let apps = load_app_manifest_at(Path::new("/nonexistent/homehub/apps.json")).unwrap();
        assert_eq!(apps.len(), 9);
        assert_eq!(search(&apps, "तस्वीरें")[0].id, "photos");
        assert_eq!(search(&apps, "documents")[0].id, "files");
    }
}
