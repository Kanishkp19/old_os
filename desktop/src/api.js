import { invoke, convertFileSrc, isTauri } from '@tauri-apps/api/core';
export const native = isTauri();
let csrf = '';
export async function authorize(token) {
  if (!/^[A-Za-z0-9_=-]{16,256}$/.test(token)) throw new Error('Enter the local session key from your installed Home Hub.');
  const response = await fetch('/api/session', { method: 'POST', credentials: 'same-origin', headers: { 'X-HH-Local': token } });
  if (!response.ok) throw new Error('Home Hub did not accept this session key.');
  csrf = (await response.json()).csrf_token;
}
export async function api(path, method = 'GET', data) {
  let response, qr;
  if (native) {
    const reply = await invoke('hub_request', { path, method, body: data === undefined ? null : JSON.stringify(data) });
    const bytes = Uint8Array.from(atob(reply.body), c => c.charCodeAt(0));
    response = new Response(bytes, { status: reply.status === 204 ? 200 : reply.status, headers: { 'Content-Type': reply.content_type } });
    qr = reply.qr_svg;
  } else {
    response = await fetch(path, { method, credentials: 'same-origin', headers: { ...(data === undefined ? {} : { 'Content-Type': 'application/json' }), ...(csrf ? { 'X-HH-CSRF': csrf } : {}) }, body: data === undefined ? undefined : JSON.stringify(data) });
  }
  const text = await response.text();
  let value; try { value = text ? JSON.parse(text) : null; } catch { value = text; }
  if (!response.ok) {
    if (response.status === 401) csrf = '';
    throw new Error(response.status === 401 ? 'Open the installed app, or unlock this dashboard with your local session key.' : value?.error?.message || value?.message || `Action failed (${response.status}).`);
  }
  if (qr && value && typeof value === 'object') value.qr = `data:image/svg+xml;base64,${qr}`;
  return value;
}
export function command(name, args = {}) {
  if (!native) return Promise.reject(new Error('Open the installed Windows app to use private apps and browser controls.'));
  return invoke(name, args);
}
export function media(id, kind = 'content') {
  if (!/^[A-Za-z0-9]{26}$/.test(id)) return '';
  return native ? `${convertFileSrc(id, 'hhmedia')}/${kind}` : `/api/files/${id}/${kind}`;
}
export async function saveFile(file) {
  if (native) return command('save_hub_file', { id: file.id || file.file_id, name: file.name || 'photo' });
  const anchor = document.createElement('a'); anchor.href = media(file.id || file.file_id); anchor.download = file.name || 'download'; anchor.rel = 'noopener'; anchor.click(); return true;
}
export function exportText(name, text, type = 'text/plain') {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement('a'); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function items(value) { return Array.isArray(value) ? value : value?.items || []; }
export function json(value, fallback = {}) { try { return JSON.parse(value); } catch { return fallback; } }
export function bytes(value) { if (value == null || !Number.isFinite(Number(value))) return '—'; const units = ['B', 'KB', 'MB', 'GB', 'TB']; let n = Number(value), i = 0; while (n >= 1024 && i < 4) { n /= 1024; i++; } return `${n.toFixed(i ? 1 : 0)} ${units[i]}`; }
