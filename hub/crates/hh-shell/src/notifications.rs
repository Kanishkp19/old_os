//! Owner notification history and visible banners.
use hh_core::local_ipc::{NotificationLevel, NotificationRecord};
use std::time::{Duration, Instant};

const MAX_BANNERS: usize = 3;
const MAX_HISTORY: usize = 50;
const BANNER_LIFETIME: Duration = Duration::from_secs(6);

#[derive(Debug, Default)]
pub struct NotificationCenter {
    pub active_banners: Vec<NotificationRecord>,
    pub history: Vec<NotificationRecord>,
    pub dnd_active: bool,
    shown_at: Vec<(String, Instant)>,
}

impl NotificationCenter {
    pub fn post_notification(&mut self, record: NotificationRecord) {
        self.expire_banners();
        self.active_banners.retain(|item| item.id != record.id);
        self.shown_at.retain(|(id, _)| id != &record.id);
        self.history.retain(|item| item.id != record.id);
        if !record.is_dismissed && (!self.dnd_active || record.level == NotificationLevel::Critical)
        {
            if self.active_banners.len() == MAX_BANNERS {
                let index = self
                    .active_banners
                    .iter()
                    .position(|item| item.level != NotificationLevel::Critical);
                if let Some(index) = index {
                    let removed = self.active_banners.remove(index);
                    self.shown_at.retain(|(id, _)| id != &removed.id);
                }
            }
            if self.active_banners.len() < MAX_BANNERS {
                self.shown_at.push((record.id.clone(), Instant::now()));
                self.active_banners.push(record.clone());
            }
        }
        self.history.push(record);
        while self.history.len() > MAX_HISTORY {
            let index = self
                .history
                .iter()
                .position(|item| item.level != NotificationLevel::Critical || item.is_dismissed);
            self.history.remove(index.unwrap_or(0));
        }
    }

    pub fn expire_banners(&mut self) {
        self.shown_at.retain(|(id, shown)| {
            self.active_banners.iter().any(|item| {
                item.id == *id
                    && (item.level == NotificationLevel::Critical
                        || shown.elapsed() < BANNER_LIFETIME)
            })
        });
        self.active_banners
            .retain(|item| self.shown_at.iter().any(|(id, _)| id == &item.id));
    }

    pub fn dismiss_banner(&mut self, id: &str) {
        self.active_banners.retain(|item| item.id != id);
        self.shown_at.retain(|(shown_id, _)| shown_id != id);
        if let Some(item) = self.history.iter_mut().find(|item| item.id == id) {
            item.is_dismissed = true;
        }
    }

    pub fn clear_all(&mut self) {
        self.active_banners
            .retain(|item| item.level == NotificationLevel::Critical && !item.is_dismissed);
        self.shown_at
            .retain(|(id, _)| self.active_banners.iter().any(|item| item.id == *id));
        self.history
            .retain(|item| item.level == NotificationLevel::Critical && !item.is_dismissed);
    }

    pub fn toggle_dnd(&mut self) -> bool {
        self.dnd_active = !self.dnd_active;
        if self.dnd_active {
            self.active_banners
                .retain(|item| item.level == NotificationLevel::Critical);
            self.shown_at
                .retain(|(id, _)| self.active_banners.iter().any(|item| item.id == *id));
        }
        self.dnd_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn notice(id: &str, level: NotificationLevel) -> NotificationRecord {
        NotificationRecord {
            id: id.into(),
            title: id.into(),
            message: String::new(),
            level,
            timestamp: 0,
            app_id: None,
            is_dismissed: false,
        }
    }
    #[test]
    fn banner_limit_and_history_bound() {
        let mut center = NotificationCenter::default();
        for index in 0..55 {
            center.post_notification(notice(&index.to_string(), NotificationLevel::Info));
        }
        assert_eq!(center.active_banners.len(), 3);
        assert_eq!(center.history.len(), 50);
    }
    #[test]
    fn dnd_keeps_critical_visible() {
        let mut center = NotificationCenter::default();
        center.toggle_dnd();
        center.post_notification(notice("info", NotificationLevel::Info));
        center.post_notification(notice("critical", NotificationLevel::Critical));
        assert_eq!(center.active_banners.len(), 1);
        assert_eq!(center.active_banners[0].id, "critical");
    }
    #[test]
    fn clear_preserves_unacknowledged_critical() {
        let mut center = NotificationCenter::default();
        center.post_notification(notice("info", NotificationLevel::Info));
        center.post_notification(notice("critical", NotificationLevel::Critical));
        center.clear_all();
        assert_eq!(center.history.len(), 1);
        assert_eq!(center.active_banners[0].id, "critical");
    }
    #[test]
    fn ordinary_banners_expire_but_critical_stays() {
        let mut center = NotificationCenter::default();
        center.post_notification(notice("info", NotificationLevel::Info));
        center.post_notification(notice("critical", NotificationLevel::Critical));
        for (_, shown) in &mut center.shown_at {
            *shown = Instant::now() - BANNER_LIFETIME;
        }
        center.expire_banners();
        assert_eq!(center.active_banners.len(), 1);
        assert_eq!(center.active_banners[0].id, "critical");
    }
}
