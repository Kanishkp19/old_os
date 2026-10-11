//! Window policy model; compositor-specific actions belong in the session adapter.
use crate::theme;
use std::collections::HashMap;

pub const SPLIT_DIVIDER_WIDTH: u32 = 8;
pub const SPLIT_DRAG_HIT_AREA: u32 = 24;
pub const WINDOW_CONTROL_HIT_AREA: u32 = theme::GEOMETRY_DESKTOP_CONTROL_MIN as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
pub fn calculate_work_area(screen_w: u32, screen_h: u32) -> WorkArea {
    let margin = u32::from(theme::GEOMETRY_SAFE_EDGE);
    let top = u32::from(theme::GEOMETRY_STATUS_BAR_HEIGHT);
    let bottom = u32::from(theme::GEOMETRY_SHELF_HEIGHT + theme::GEOMETRY_SHELF_BOTTOM_OFFSET);
    WorkArea {
        x: margin,
        y: top,
        width: screen_w.saturating_sub(margin * 2),
        height: screen_h.saturating_sub(top + bottom),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workspace {
    Home,
    Focus,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowState {
    Floating,
    Maximized,
    Fullscreen,
    SplitLeft,
    SplitRight,
    Minimized,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowEntry {
    pub id: String,
    pub app_id: String,
    pub workspace: Workspace,
    pub state: WindowState,
    pub geometry: WorkArea,
}

pub fn floating_geometry(app_id: &str, area: WorkArea) -> WorkArea {
    let (wanted_w, wanted_h) = if app_id == "calculator" {
        (360, 480)
    } else {
        (960, 640)
    };
    let width = wanted_w.min(area.width);
    let height = wanted_h.min(area.height);
    WorkArea {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn split_geometry(area: WorkArea, left_fraction_third: bool) -> (WorkArea, WorkArea) {
    let available = area.width.saturating_sub(SPLIT_DIVIDER_WIDTH);
    let left_width = if left_fraction_third {
        available / 3
    } else {
        available / 2
    };
    let right_width = available - left_width;
    (
        WorkArea {
            width: left_width,
            ..area
        },
        WorkArea {
            x: area.x + left_width + SPLIT_DIVIDER_WIDTH,
            width: right_width,
            ..area
        },
    )
}

pub struct WindowManager {
    pub windows: HashMap<String, WindowEntry>,
    pub active_window_id: Option<String>,
    pub active_workspace: Workspace,
    pub work_area: WorkArea,
}

impl WindowManager {
    pub fn new(work_area: WorkArea) -> Self {
        Self {
            windows: HashMap::new(),
            active_window_id: None,
            active_workspace: Workspace::Home,
            work_area,
        }
    }
    pub fn register_window(&mut self, id: &str, app_id: &str, workspace: Workspace) {
        self.windows.insert(
            id.into(),
            WindowEntry {
                id: id.into(),
                app_id: app_id.into(),
                workspace,
                state: WindowState::Floating,
                geometry: floating_geometry(app_id, self.work_area),
            },
        );
    }
    pub fn focus_window(&mut self, id: &str) -> bool {
        if let Some(window) = self.windows.get(id) {
            self.active_workspace = window.workspace;
            self.active_window_id = Some(id.into());
            true
        } else {
            false
        }
    }
    pub fn minimize_window(&mut self, id: &str) -> bool {
        self.set_state(id, WindowState::Minimized)
    }
    pub fn maximize_window(&mut self, id: &str) -> bool {
        self.set_state(id, WindowState::Maximized)
    }
    pub fn restore_window(&mut self, id: &str) -> bool {
        self.set_state(id, WindowState::Floating)
    }
    fn set_state(&mut self, id: &str, state: WindowState) -> bool {
        if let Some(window) = self.windows.get_mut(id) {
            window.state = state;
            window.geometry = if state == WindowState::Floating {
                floating_geometry(&window.app_id, self.work_area)
            } else {
                self.work_area
            };
            if state == WindowState::Minimized && self.active_window_id.as_deref() == Some(id) {
                self.active_window_id = None;
            }
            true
        } else {
            false
        }
    }
    pub fn close_window(&mut self, id: &str) -> bool {
        if self.active_window_id.as_deref() == Some(id) {
            self.active_window_id = None;
        }
        self.windows.remove(id).is_some()
    }
    pub fn snap_window_split(&mut self, id: &str, left: bool, left_fraction_third: bool) -> bool {
        let (left_area, right_area) = split_geometry(self.work_area, left_fraction_third);
        if let Some(window) = self.windows.get_mut(id) {
            window.state = if left {
                WindowState::SplitLeft
            } else {
                WindowState::SplitRight
            };
            window.geometry = if left { left_area } else { right_area };
            true
        } else {
            false
        }
    }
    pub fn switch_workspace(&mut self, workspace: Workspace) {
        self.active_workspace = workspace;
        if self
            .active_window_id
            .as_ref()
            .and_then(|id| self.windows.get(id))
            .is_some_and(|window| window.workspace != workspace)
        {
            self.active_window_id = None;
        }
    }
    pub fn visible_windows(&self) -> Vec<&WindowEntry> {
        self.windows
            .values()
            .filter(|window| {
                window.workspace == self.active_workspace && window.state != WindowState::Minimized
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn work_areas_and_splits() {
        assert_eq!(
            calculate_work_area(1366, 768),
            WorkArea {
                x: 16,
                y: 40,
                width: 1334,
                height: 652
            }
        );
        let area = calculate_work_area(1920, 1080);
        assert_eq!(area.width, 1888);
        assert_eq!(area.height, 964);
        let (left, right) = split_geometry(area, false);
        assert_eq!(
            (left.width, right.width, right.x - left.x - left.width),
            (940, 940, 8)
        );
        let (left, right) = split_geometry(area, true);
        assert_eq!((left.width, right.width), (626, 1254));
    }
    #[test]
    fn floating_and_workspace_filtering() {
        let area = calculate_work_area(1366, 768);
        assert_eq!(
            (
                floating_geometry("calculator", area).width,
                floating_geometry("calculator", area).height
            ),
            (360, 480)
        );
        assert_eq!(
            (
                floating_geometry("files", area).width,
                floating_geometry("files", area).height
            ),
            (960, 640)
        );
        let mut manager = WindowManager::new(area);
        manager.register_window("a", "files", Workspace::Home);
        manager.register_window("b", "calculator", Workspace::Focus);
        assert_eq!(manager.visible_windows().len(), 1);
        manager.switch_workspace(Workspace::Focus);
        assert_eq!(manager.visible_windows()[0].id, "b");
    }
}
