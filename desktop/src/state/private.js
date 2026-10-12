import { command, json, native } from '../api.js';

export async function loadInterface() {
  let row = null, value = null;
  if (native) {
    row = (await command('local_list', { kind: 'preference', query: '', trash: false })).find(item => item.title === 'interface');
    value = json(row?.body, {});
  } else {
    try { value = json(localStorage.getItem('hh-interface'), {}); } catch { value = {}; }
  }
  return { id: row?.id || null, language: value.language === 'hi' ? 'hi' : 'en', large: !!value.large, contrast: !!value.contrast, reduced: value.reduced !== false, notifications: value.notifications !== false };
}
export async function saveInterface(preferences) {
  const { id, ...value } = preferences;
  if (native) return (await command('local_save', { id, kind: 'preference', title: 'interface', body: JSON.stringify(value) })).id;
  localStorage.setItem('hh-interface', JSON.stringify(value));
  return null;
}
export async function loadPrivateTrash() {
  return (await Promise.all(['note', 'playlist', 'bookmark'].map(kind => command('local_list', { kind, query: '', trash: true })))).flat();
}
export function exportPrivate() { return command('local_export'); }
export function importPrivate() { return command('local_import'); }
