// Pure geometry/lifecycle helpers keep window management independent of app stores.
export function fit(rect, width, height) {
  const area = { width: Math.max(0, width - 32), height: Math.max(0, height - 144) };
  const w = Math.min(rect.width, area.width), h = Math.min(rect.height, area.height);
  return { width: w, height: h, x: Math.max(16, Math.min(rect.x, width - w - 16)), y: Math.max(56, Math.min(rect.y, height - h - 88)) };
}
export function openWindow(windows, app, width, height, saved) {
  const existing = windows.find(w => w.app === app);
  const rect = saved || { width: app === 'Calculator' ? 360 : 960, height: app === 'Calculator' ? 480 : 640, x: (width - (app === 'Calculator' ? 360 : 960)) / 2 + windows.length * 20, y: 64 + windows.length * 20 };
  return [...windows.filter(w => w.app !== app), existing ? { ...existing, minimized: false } : { app, minimized: false, maximized: false, rect: fit(rect, width, height) }];
}
export function nextWindow(windows) {
  return [...windows].reverse().find(w => !w.minimized)?.app || 'Home';
}
