import { command, json } from '../api.js';

export function tracksForPlaylist(music, playlist) {
  return playlist ? (json(playlist.body, { ids: [] }).ids || []).map(id => music.find(file => file.id === id)).filter(Boolean) : music;
}
export async function createPlaylist(title) {
  return command('local_save', { id: null, kind: 'playlist', title, body: JSON.stringify({ ids: [] }) });
}
export async function changePlaylist(playlist, file, remove = false) {
  if (!playlist) return null;
  const old = json(playlist.body, { ids: [] }).ids || [];
  const ids = remove ? old.filter(id => id !== file.id) : [...new Set([...old, file.id])];
  return command('local_save', { id: playlist.id, kind: 'playlist', title: playlist.title, body: JSON.stringify({ ids }) });
}
export function advanceTrack({ playing, queuedTracks, tracks, shuffle, repeat }, direction = 1, automatic = false) {
  if (direction > 0 && queuedTracks.length) return { playing: queuedTracks[0], queuedTracks: queuedTracks.slice(1) };
  if (!tracks.length) return { playing: null, queuedTracks };
  const index = tracks.findIndex(row => row.id === playing?.id);
  if (automatic && !repeat && index === tracks.length - 1 && !shuffle) return { playing: null, queuedTracks };
  const others = tracks.filter(row => row.id !== playing?.id);
  return { playing: shuffle && others.length ? others[Math.floor(Math.random() * others.length)] : tracks[(index + direction + tracks.length) % tracks.length], queuedTracks };
}
