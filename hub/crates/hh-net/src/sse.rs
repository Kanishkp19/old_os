//! Server-sent events bus (API_SPEC §5): transfer.progress,
//! transfer.completed, alert.created, device.revoked, storage.low,
//! hub.shutting_down.

use hh_core::types::HubEvent;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<HubEvent>,
}

impl EventBus {
    pub fn new(tx: broadcast::Sender<HubEvent>) -> Self {
        Self { tx }
    }

    pub fn emit(&self, event: &str, data: serde_json::Value) {
        let _ = self.tx.send(HubEvent { event: event.to_string(), data });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HubEvent> {
        self.tx.subscribe()
    }
}
