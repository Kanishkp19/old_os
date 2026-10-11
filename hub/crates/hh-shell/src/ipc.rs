//! Bounded framed client for the owner-only local broker.
use hh_core::local_ipc::{decode_frame, encode_frame, AppLaunchReq, LocalIpcError,
    LocalIpcRequest, LocalIpcResponse, LocalOp, NotificationDismissReq,
    NotificationRecord, QuickSettingsUpdate, ShellSnapshot, MAX_FRAME_SIZE};
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type MockHandler = Arc<dyn Fn(Vec<u8>) -> Result<Vec<u8>, IpcError> + Send + Sync>;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error(transparent)] Io(#[from] std::io::Error),
    #[error(transparent)] Protocol(#[from] LocalIpcError),
    #[error(transparent)] Json(#[from] serde_json::Error),
    #[error("broker operation failed: {0}")] Broker(String),
}

pub struct ShellIpcClient { socket: PathBuf, mock: Option<MockHandler> }
impl ShellIpcClient {
    pub fn new() -> Self {
        let socket = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("homehub-local.sock"))
            .unwrap_or_else(|| PathBuf::from("/run/homehub/local-broker.sock"));
        Self { socket, mock: None }
    }
    pub fn with_socket(socket: PathBuf) -> Self { Self { socket, mock: None } }
    pub fn with_mock(handler: MockHandler) -> Self { Self { socket: PathBuf::new(), mock: Some(handler) } }

    pub async fn request_shell_snapshot(&self) -> Result<ShellSnapshot, IpcError> {
        let request = LocalIpcRequest { id: "shell-snapshot".into(),
            op: LocalOp::ShellSnapshot, payload: serde_json::json!({}) };
        let response = self.exchange(request).await?;
        if !response.ok {
            return Err(IpcError::Broker(response.error.map_or_else(
                || "unknown broker error".into(), |error| error.to_string())));
        }
        let data = response.data.ok_or_else(|| IpcError::Broker("missing snapshot".into()))?;
        Ok(serde_json::from_value(data)?)
    }

    pub async fn request_app_launch(&self, app_id: &str) -> Result<(), IpcError> {
        self.request_unit(LocalOp::AppLaunch, serde_json::to_value(AppLaunchReq { app_id: app_id.into() })?).await
    }

    pub async fn request_notifications_list(&self) -> Result<Vec<NotificationRecord>, IpcError> {
        let response = self.exchange(LocalIpcRequest { id: "notifications-list".into(),
            op: LocalOp::NotificationsList, payload: serde_json::json!({}) }).await?;
        Self::check_response(&response)?;
        let data = response.data.ok_or_else(|| IpcError::Broker("missing notifications".into()))?;
        Ok(serde_json::from_value(data)?)
    }

    pub async fn request_notification_dismiss(&self, id: &str) -> Result<(), IpcError> {
        self.request_unit(LocalOp::NotificationDismiss,
            serde_json::to_value(NotificationDismissReq { id: id.into() })?).await
    }

    pub async fn request_quick_settings_set(&self, update: &QuickSettingsUpdate) -> Result<(), IpcError> {
        self.request_unit(LocalOp::QuickSettingsSet, serde_json::to_value(update)?).await
    }

    async fn request_unit(&self, op: LocalOp, payload: serde_json::Value) -> Result<(), IpcError> {
        let response = self.exchange(LocalIpcRequest { id: format!("shell-{}", serde_json::to_value(op)?.as_str().unwrap_or("request")),
            op, payload }).await?;
        Self::check_response(&response)
    }

    fn check_response(response: &LocalIpcResponse) -> Result<(), IpcError> {
        if response.ok { Ok(()) } else {
            Err(IpcError::Broker(response.error.as_ref().map_or_else(
                || "unknown broker error".into(), ToString::to_string)))
        }
    }

    async fn exchange(&self, request: LocalIpcRequest) -> Result<LocalIpcResponse, IpcError> {
        let outbound = encode_frame(&request)?;
        let inbound = if let Some(mock) = &self.mock { mock(outbound)? } else {
            let mut stream = tokio::net::UnixStream::connect(&self.socket).await?;
            stream.write_all(&outbound).await?;
            let mut header = [0u8; 4];
            stream.read_exact(&mut header).await?;
            let length = u32::from_be_bytes(header) as usize;
            if length > MAX_FRAME_SIZE { return Err(IpcError::Protocol(
                LocalIpcError::InvalidInput("frame exceeds 1 MiB".into()))); }
            let mut frame = Vec::with_capacity(length + 4);
            frame.extend_from_slice(&header);
            frame.resize(length + 4, 0);
            stream.read_exact(&mut frame[4..]).await?;
            frame
        };
        let response: LocalIpcResponse = decode_frame(&inbound)?;
        if response.id != request.id {
            return Err(IpcError::Protocol(LocalIpcError::InvalidInput("response ID mismatch".into())));
        }
        Ok(response)
    }
}
impl Default for ShellIpcClient { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn framed_snapshot_request_round_trip() {
        let snapshot = ShellSnapshot { timestamp: 60, hub_name: "Hub".into(),
            storage_available_bytes: 10, storage_healthy: true, connected_devices: 1,
            wifi_ssid: None, volume_pct: 20, battery_pct: None, is_charging: false, is_locked: false };
        let expected = snapshot.clone();
        let client = ShellIpcClient::with_mock(Arc::new(move |frame| {
            let request: LocalIpcRequest = decode_frame(&frame)?;
            assert_eq!(request.op, LocalOp::ShellSnapshot);
            Ok(encode_frame(&LocalIpcResponse { id: request.id, ok: true,
                data: Some(serde_json::to_value(&snapshot)?), error: None })?)
        }));
        assert_eq!(client.request_shell_snapshot().await.unwrap(), expected);
    }

    #[tokio::test]
    async fn framed_app_launch_round_trip() {
        let client = ShellIpcClient::with_mock(Arc::new(|frame| {
            let request: LocalIpcRequest = decode_frame(&frame)?;
            assert_eq!(request.op, LocalOp::AppLaunch);
            assert_eq!(serde_json::from_value::<AppLaunchReq>(request.payload)?,
                AppLaunchReq { app_id: "photos".into() });
            Ok(encode_frame(&LocalIpcResponse { id: request.id, ok: true,
                data: None, error: None })?)
        }));
        client.request_app_launch("photos").await.unwrap();
    }
}
