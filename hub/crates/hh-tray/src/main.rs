//! hh-tray: per-user tray app (FR-1.3, M3).
//!
//! Shows hub status at a glance (online/offline, free GB, device and transfer
//! counts), opens the local dashboard, opens a pairing window and surfaces
//! the manual code, and shows an "Update available" entry when the hub
//! reports one (W2.6 signed-manifest channel).
//!
//! All dashboard traffic is loopback-only and authenticated with the local
//! token the hub persists to `<data_dir>/dashboard_token` (same-user read).
//! The tray never talks to the LAN API and holds no secrets beyond that
//! local token.

use std::time::Duration;

use clap::Parser;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
use winit::window::Window;

#[derive(Parser)]
#[command(name = "hh-tray", about = "Home Hub tray app")]
struct Args {
    /// Dashboard base URL (default the hub's loopback listener).
    #[arg(long, default_value = "http://127.0.0.1:47801")]
    dashboard_url: String,
    /// Explicit local token (default: read `<data_dir>/dashboard_token`).
    #[arg(long)]
    token: Option<String>,
    /// Hub data dir holding `dashboard_token` (default: hh-core default).
    #[arg(long)]
    data_dir: Option<String>,
    /// Status poll interval in seconds.
    #[arg(long, default_value_t = 5)]
    poll_secs: u64,
}

/// Snapshot pushed from the poll thread to the UI thread.
#[derive(Clone, Default)]
struct Status {
    online: bool,
    hub_name: String,
    free_bytes: Option<u64>,
    devices: u64,
    transfers: u64,
    version: String,
    /// (version, url) when the hub reports an update (W2.6).
    update: Option<(String, String)>,
}

enum UserEvent {
    Status(Status),
}

struct Ids {
    status: tray_icon::menu::MenuId,
    pair: tray_icon::menu::MenuId,
    open: tray_icon::menu::MenuId,
    update: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

impl Ids {
    fn new() -> Self {
        Self {
            status: tray_icon::menu::MenuId::new("status"),
            pair: tray_icon::menu::MenuId::new("pair"),
            open: tray_icon::menu::MenuId::new("open"),
            update: tray_icon::menu::MenuId::new("update"),
            quit: tray_icon::menu::MenuId::new("quit"),
        }
    }
}

struct App {
    ctx: Ctx,
    proxy: EventLoopProxy<UserEvent>,
    ids: Ids,
    tray: Option<TrayIcon>,
    status: Status,
}

struct Ctx {
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Ctx {
    fn fetch_status(&self) -> Status {
        let mut s = Status { online: false, ..Default::default() };
        let url = format!("{}/api/overview", self.base);
        let resp = self.agent.get(&url).set("X-HH-Local", &self.token).call();
        if let Ok(mut resp) = resp {
            if let Ok(v) = resp.into_json::<serde_json::Value>() {
                s.online = true;
                s.hub_name = v["name"].as_str().unwrap_or("Home Hub").to_string();
                s.free_bytes = v["free_bytes"].as_u64();
                s.devices = v["devices"].as_u64().unwrap_or(0);
                s.transfers = v["active_transfers"].as_u64().unwrap_or(0);
                s.version = v["version"].as_str().unwrap_or("").to_string();
            }
        }
        // Update channel (W2.6): optional endpoint; absence is not an error.
        if s.online {
            if let Ok(mut resp) = self
                .agent
                .get(&format!("{}/api/update", self.base))
                .set("X-HH-Local", &self.token)
                .call()
            {
                if let Ok(v) = resp.into_json::<serde_json::Value>() {
                    if v["available"].as_bool().unwrap_or(false) {
                        s.update = Some((
                            v["latest_version"].as_str().unwrap_or("").to_string(),
                            v["url"].as_str().unwrap_or("").to_string(),
                        ));
                    }
                }
            }
        }
        s
    }

    /// Open a pairing window and return the manual code for the dialog.
    fn open_pairing(&self) -> Result<String, String> {
        self.agent
            .post(&format!("{}/api/pair/open", self.base))
            .set("X-HH-Local", &self.token)
            .call()
            .map_err(|e| format!("hub unreachable ({e})"))?;
        let mut resp = self
            .agent
            .get(&format!("{}/api/pair/qr", self.base))
            .set("X-HH-Local", &self.token)
            .call()
            .map_err(|e| format!("pairing window failed ({e})"))?;
        let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
        let code = v["manual_code"].as_str().unwrap_or("").to_string();
        let secs = v["expires_in"].as_i64().unwrap_or(0);
        if code.is_empty() {
            return Err("hub did not return a pairing code".into());
        }
        Ok(format!("Enter this code in the Home Hub mobile app:\n\n{code}\n\n(Valid for {secs} seconds)"))
    }
}

fn fmt_gb(n: Option<u64>) -> String {
    match n {
        Some(b) => format!("{:.1} GB", b as f64 / 1_073_741_824.0),
        None => "?".into(),
    }
}

impl App {
    fn tooltip(&self) -> String {
        if self.status.online {
            format!(
                "Home Hub — online · {} free · {} device(s) · {} transfer(s)",
                fmt_gb(self.status.free_bytes),
                self.status.devices,
                self.status.transfers
            )
        } else {
            "Home Hub — offline".to_string()
        }
    }

    fn rebuild_menu(&mut self) {
        let status_label = if self.status.online {
            format!(
                "{} · {} free · {} devices · {} transfers",
                self.status.hub_name,
                fmt_gb(self.status.free_bytes),
                self.status.devices,
                self.status.transfers
            )
        } else {
            "Home Hub — offline".to_string()
        };
        let menu = Menu::new();
        let _ = menu.append(&MenuItem::with_id(self.ids.status.clone(), status_label, false, None));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(self.ids.pair.clone(), "Pair a device…", true, None));
        let _ = menu.append(&MenuItem::with_id(self.ids.open.clone(), "Open dashboard", true, None));
        if let Some((ver, _url)) = &self.status.update {
            if !ver.is_empty() {
                let _ = menu.append(&MenuItem::with_id(
                    self.ids.update.clone(),
                    format!("Update available — v{ver}"),
                    true,
                    None,
                ));
            }
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(self.ids.quit.clone(), "Quit Home Hub tray", true, None));
        if let Some(tray) = &self.tray {
            let _ = tray.set_menu(Some(Box::new(menu)));
            let _ = tray.set_tooltip(Some(self.tooltip()));
        }
    }

    fn handle_menu(&mut self, elwt: &EventLoopWindowTarget<UserEvent>, id: &tray_icon::menu::MenuId) {
        if *id == self.ids.quit {
            elwt.exit();
        } else if *id == self.ids.open || *id == self.ids.status {
            let _ = open::that(&self.ctx.base);
        } else if *id == self.ids.update {
            if let Some((_, url)) = self.status.update.clone() {
                if !url.is_empty() {
                    let _ = open::that(&url);
                }
            }
        } else if *id == self.ids.pair {
            match self.ctx.open_pairing() {
                Ok(msg) => info_dialog("Pair a device", &msg),
                Err(e) => info_dialog("Pair a device", &format!("Could not open a pairing window:\n{e}")),
            }
        }
    }
}

fn info_dialog(title: &str, msg: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(msg)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_level(rfd::MessageLevel::Info)
        .show();
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &EventLoopWindowTarget<UserEvent>) {
        let icon = app_icon();
        match TrayIconBuilder::new()
            .with_menu(Box::new(Menu::new()))
            .with_tooltip("Home Hub — starting…")
            .with_icon(icon)
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => {
                eprintln!("hh-tray: failed to create tray icon: {e}");
            }
        }
        self.rebuild_menu();
    }

    fn user_event(&mut self, elwt: &EventLoopWindowTarget<UserEvent>, event: UserEvent) {
        match event {
            UserEvent::Status(s) => {
                self.status = s;
                self.rebuild_menu();
            }
        }
        drain_menu_events(self, elwt);
    }

    fn about_to_wait(&mut self, elwt: &EventLoopWindowTarget<UserEvent>) {
        drain_menu_events(self, elwt);
    }
}

fn drain_menu_events(app: &mut App, elwt: &EventLoopWindowTarget<UserEvent>) {
    let receiver = MenuEvent::receiver();
    while let Ok(ev) = receiver.try_recv() {
        app.handle_menu(elwt, &ev.id);
    }
}

/// A 32×32 house glyph so the tray has a real icon without shipping assets.
fn app_icon() -> Icon {
    const W: usize = 32;
    let mut rgba = vec![0u8; W * W * 4];
    let put = |x: usize, y: usize, c: [u8; 3]| {
        let i = (y * W + x) * 4;
        rgba[i] = c[0];
        rgba[i + 1] = c[1];
        rgba[i + 2] = c[2];
        rgba[i + 3] = 255;
    };
    let roof = [226u8, 98, 44]; // warm orange
    let body = [238u8, 238, 238];
    // Roof: filled triangle from (2,14) to (29,14) apex (15,2).
    for y in 2..=14usize {
        let half = ((y - 2) * 14 / 12).clamp(0, 14);
        for x in (15 - half)..=(15 + half) {
            if x < W {
                put(x, y, roof);
            }
        }
    }
    // Body: rectangle (6,15)..(25,29) with a door notch.
    for y in 15..30usize {
        for x in 6..26usize {
            let door = x >= 13 && x <= 18 && y >= 21;
            if door {
                put(x, y, [40, 44, 52]);
            } else {
                put(x, y, body);
            }
        }
    }
    Icon::from_rgba(rgba, W as u32, W as u32).expect("icon rgba size matches")
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_ansi(false)
        .init();

    let args = Args::parse();
    let token = match args.token {
        Some(t) => t,
        None => {
            let mut dir = match &args.data_dir {
                Some(d) => std::path::PathBuf::from(d),
                None => hh_core::Config::default().data_dir,
            };
            dir.push("dashboard_token");
            std::fs::read_to_string(&dir)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        }
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout(Duration::from_secs(4))
        .build();
    let ctx = Ctx { base: args.dashboard_url.trim_end_matches('/').to_string(), token, agent };

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();

    // Status poll thread: pushes snapshots into the UI thread. Runs even when
    // the hub is down so the tooltip flips to "offline".
    {
        let proxy = proxy.clone();
        let ctx_poll = Ctx { base: ctx.base.clone(), token: ctx.token.clone(), agent: ctx.agent.clone() };
        std::thread::spawn(move || {
            loop {
                let s = ctx_poll.fetch_status();
                if proxy.send_event(UserEvent::Status(s)).is_err() {
                    return; // UI gone
                }
                std::thread::sleep(Duration::from_secs(args.poll_secs.max(1)));
            }
        });
    }

    let mut app = App { ctx, proxy, ids: Ids::new(), tray: None, status: Status::default() };
    let _: Option<Window> = None;
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("hh-tray: event loop error: {e}");
        std::process::exit(1);
    }
}
