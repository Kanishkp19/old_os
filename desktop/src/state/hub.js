import { api, items } from '../api.js';

// Keep request status and route data in one observable store for the host.
import { writable } from 'svelte/store';
export function createHubState(translate) {
  const state = writable({ connected: false, authNeeded: false, busy: false, error: '', notice: '', overview: {}, settings: {}, files: [], alerts: [] });
  async function run(action, successMessage = '') {
    let busy = false;
    state.update(s => { busy = s.busy; return busy ? s : { ...s, busy: true, error: '', notice: '' }; });
    if (busy) return;
    try { const result = await action(); state.update(s => ({ ...s, notice: successMessage })); return result; }
    catch (error) { state.update(s => ({ ...s, error: translate(String(error?.message || error)) })); }
    finally { state.update(s => ({ ...s, busy: false })); }
  }
  async function overview() {
    const [summary, settings, alerts] = await Promise.all([api('/api/overview'), api('/api/settings'), api('/api/alerts')]);
    state.update(s => ({ ...s, connected: true, authNeeded: false, overview: summary, settings, alerts: items(alerts) }));
    return { overview: summary, settings, alerts: items(alerts) };
  }
  async function files(params) {
    const page = await api(`/api/files?${params}`);
    state.update(s => ({ ...s, files: items(page) }));
    return page;
  }
  return { state, run, overview, files };
}
