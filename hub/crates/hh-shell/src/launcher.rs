//! Shelf and searchable Home grid models.
use crate::apps::{self, App};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SHELF_HEIGHT: u16 = crate::theme::GEOMETRY_SHELF_HEIGHT;
pub const SHELF_OFFSET_BOTTOM: u16 = crate::theme::GEOMETRY_SHELF_BOTTOM_OFFSET;
pub const SHELF_RADIUS: u16 = crate::theme::RADII_SHELF;
const PINS: [&str; 5] = ["home", "photos", "files", "browser", "music"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShelfItem {
    pub app_id: String, pub label: String, pub is_running: bool,
    pub is_pinned: bool, pub has_indicator_dot: bool, pub is_recents: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppState { Idle, Launching { started_at: u64 }, Running }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridCursor { pub row: u8, pub col: u8 }
impl GridCursor {
    pub fn move_up(&mut self) { self.row = self.row.saturating_sub(1); }
    pub fn move_down(&mut self) { self.row = (self.row + 1).min(2); }
    pub fn move_left(&mut self) { self.col = self.col.saturating_sub(1); }
    pub fn move_right(&mut self) { self.col = (self.col + 1).min(2); }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchCommand { pub app_id: String }

pub struct Launcher {
    pub all_apps: Vec<App>, pub shelf_items: Vec<ShelfItem>,
    pub app_states: HashMap<String, AppState>, pub cursor: GridCursor,
}

impl Launcher {
    pub fn new(all_apps: Vec<App>) -> Self {
        let mut launcher = Self { all_apps, shelf_items: Vec::new(),
            app_states: HashMap::new(), cursor: GridCursor::default() };
        launcher.update_running_apps(&[]);
        launcher
    }

    pub fn update_running_apps(&mut self, running_app_ids: &[String]) {
        let running: HashSet<&str> = running_app_ids.iter().map(String::as_str).collect();
        self.shelf_items = PINS.iter().map(|id| self.item(id, true, running.contains(id))).collect();
        let mut seen: HashSet<&str> = PINS.into_iter().collect();
        for id in running_app_ids {
            if seen.insert(id) { self.shelf_items.push(self.item(id, false, true)); }
        }
        self.shelf_items.push(ShelfItem { app_id: "recents".into(), label: "Recents".into(),
            is_running: false, is_pinned: false, has_indicator_dot: false, is_recents: true });
        self.app_states.retain(|_, state| !matches!(state, AppState::Running));
        for id in running_app_ids { self.app_states.insert(id.clone(), AppState::Running); }
    }

    fn item(&self, id: &str, pinned: bool, running: bool) -> ShelfItem {
        ShelfItem { app_id: id.into(), label: self.all_apps.iter().find(|app| app.id == id)
            .map_or_else(|| if id == "home" { "Home".into() } else { id.into() }, |app| app.label.clone()),
            is_running: running, is_pinned: pinned, has_indicator_dot: running, is_recents: false }
    }

    pub fn search(&self, query: &str) -> Vec<&App> { apps::search(&self.all_apps, query) }
    pub fn empty_text(hindi: bool) -> &'static str {
        if hindi { "कोई ऐप नहीं मिला" } else { "No apps found" }
    }
    pub fn launch(&mut self, app_id: &str) -> Option<LaunchCommand> {
        if app_id != "home" && !self.all_apps.iter().any(|app| app.id == app_id) { return None; }
        let started_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.app_states.insert(app_id.into(), AppState::Launching { started_at });
        Some(LaunchCommand { app_id: app_id.into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apps() -> Vec<App> { vec![App { id: "photos".into(), label: "Photos".into(), available_in_trial: true },
        App { id: "settings".into(), label: "Settings".into(), available_in_trial: true }] }
    #[test]
    fn shelf_preserves_pins_and_appends_running_before_recents() {
        let mut launcher = Launcher::new(apps());
        launcher.update_running_apps(&["settings".into(), "photos".into()]);
        assert_eq!(launcher.shelf_items.iter().map(|item| item.app_id.as_str()).collect::<Vec<_>>(),
            ["home", "photos", "files", "browser", "music", "settings", "recents"]);
        assert!(launcher.shelf_items[1].has_indicator_dot);
        assert!(launcher.shelf_items.last().unwrap().is_recents);
    }
    #[test]
    fn cursor_stays_in_three_by_three_grid() {
        let mut cursor = GridCursor::default();
        cursor.move_up(); cursor.move_left(); assert_eq!(cursor, GridCursor::default());
        for _ in 0..4 { cursor.move_down(); cursor.move_right(); }
        assert_eq!(cursor, GridCursor { row: 2, col: 2 });
    }
    #[test]
    fn search_accepts_hindi_and_english() {
        let launcher = Launcher::new(apps());
        assert_eq!(launcher.search("तस्वीरें")[0].id, "photos");
        assert_eq!(launcher.search("preferences")[0].id, "settings");
    }
}
