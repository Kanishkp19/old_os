//! Semantic nodes and localized announcements for native accessibility bridges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    StatusBar,
    Shelf,
    Launcher,
    AppGrid,
    Tile,
    Button,
    Window,
    Indicator,
    SearchField,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessibleState {
    pub focused: bool,
    pub selected: bool,
    pub running: bool,
    pub busy: bool,
    pub disabled: bool,
    pub expanded: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibleNode {
    pub id: String,
    pub role: Role,
    pub name: String,
    pub description: Option<String>,
    pub state: AccessibleState,
    pub children: Vec<String>,
}
impl AccessibleNode {
    pub fn new(id: impl Into<String>, role: Role, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            name: name.into(),
            description: None,
            state: AccessibleState::default(),
            children: Vec::new(),
        }
    }
    pub fn add_child(&mut self, child: &AccessibleNode) {
        self.children.push(child.id.clone());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Hindi,
}
pub fn app_launched(name: &str, language: Language) -> String {
    match language {
        Language::English => format!("{name} launched"),
        Language::Hindi => format!("{name} शुरू हुआ"),
    }
}
pub fn window_focused(title: &str, workspace: &str, language: Language) -> String {
    match language {
        Language::English => format!("{title} focused, {workspace} workspace"),
        Language::Hindi => format!("{title} फ़ोकस हुआ"),
    }
}
pub fn storage_alert(language: Language) -> &'static str {
    match language {
        Language::English => "Critical warning: storage disk degraded",
        Language::Hindi => "महत्वपूर्ण चेतावनी: स्टोरेज डिस्क में समस्या",
    }
}
pub fn battery_warning(pct: u8, language: Language) -> String {
    match language {
        Language::English => format!("Battery low: {pct}% remaining"),
        Language::Hindi => format!("बैटरी कम: {pct}% शेष"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_and_announcements() {
        let mut root = AccessibleNode::new("launcher", Role::Launcher, "Home");
        let tile = AccessibleNode::new("photos", Role::Tile, "Photos");
        root.add_child(&tile);
        assert_eq!(root.children, ["photos"]);
        assert_eq!(app_launched("Photos", Language::Hindi), "Photos शुरू हुआ");
        assert_eq!(
            window_focused("Files", "Home", Language::English),
            "Files focused, Home workspace"
        );
        assert!(storage_alert(Language::Hindi).contains("चेतावनी"));
        assert_eq!(
            battery_warning(12, Language::English),
            "Battery low: 12% remaining"
        );
    }
}
