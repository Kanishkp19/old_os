use std::error::Error;
use std::fs;
use std::time::{Duration, Instant};

use log::{info, warn};
use smithay::wayland::compositor::compositor_init;
use smithay::wayland::seat::{Seat, XkbConfig};
use smithay::wayland::shell::xdg::{xdg_shell_init, XdgRequest};
use wayland_server::Display;

fn memory_kib() -> Option<(u64, u64)> {
    let smaps = fs::read_to_string("/proc/self/smaps_rollup").ok()?;
    let number = |key: &str| -> Option<u64> {
        smaps.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name == key).then(|| value.split_whitespace().next()?.parse().ok()).flatten()
        })
    };
    Some((number("Rss")?, number("Pss")?))
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut display = Display::new();
    compositor_init(&mut display, |_surface, _data| {}, None);
    let (mut seat, _global) = Seat::new(&mut display, "seat0".into(), None);
    let _keyboard = seat.add_keyboard(XkbConfig::default(), 200, 25, |_seat, focus| {
        info!("keyboard focus changed: {}", focus.is_some());
    })?;
    let _pointer = seat.add_pointer(|status| info!("pointer cursor status: {status:?}"));
    let (_shell, _, _) = xdg_shell_init(
        &mut display,
        |request, _data| match request {
            XdgRequest::NewToplevel { surface } => {
                info!("xdg toplevel created; initial configure sent");
                surface.send_configure();
            }
            XdgRequest::NewPopup { surface } => info!("xdg popup: {surface:?}"),
            XdgRequest::Resize { surface, edges, .. } => {
                info!("resize requested on {surface:?}, edges={edges:?}");
                surface.send_configure();
            }
            XdgRequest::Move { surface, .. } => info!("move requested on {surface:?}"),
            other => info!("xdg request: {other:?}"),
        },
        None,
    );
    let socket = display.add_socket_auto()?;
    info!("smithay Wayland socket {}", socket.to_string_lossy());
    warn!("physical keyboard/pointer, IME, clipboard and rendering require a guest backend; this socket is diagnostic only");
    let started = Instant::now();
    loop {
        display.dispatch(Duration::from_millis(50), &mut ())?;
        display.flush_clients(&mut ());
        if started.elapsed().as_secs() % 10 == 0 {
            if let Some((rss, pss)) = memory_kib() {
                info!("uptime_ms={} rss_kib={rss} pss_kib={pss}", started.elapsed().as_millis());
            } else {
                warn!("smaps_rollup unavailable");
            }
        }
    }
}
