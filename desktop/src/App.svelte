<script>
  import { onMount, tick } from 'svelte';
  import { listen, emit } from '@tauri-apps/api/event';
  import { api, authorize, bytes, command, exportText, items, json, media, native, saveFile } from './api.js';
  import { calculate } from './calculator.js';
  import { createHubState } from './state/hub.js';
  import { loadInterface, saveInterface, loadPrivateTrash as privateTrashList } from './state/private.js';
  import { tracksForPlaylist, createPlaylist, changePlaylist, advanceTrack } from './state/music.js';
  import { createNoteEditor } from './state/notes.js';
  import Photos from './apps/Photos.svelte';
  import Files from './apps/Files.svelte';
  import Notes from './apps/Notes.svelte';
  import Browser from './apps/Browser.svelte';
  import YouTube from './apps/YouTube.svelte';
  import Music from './apps/Music.svelte';
  import Calculator from './apps/Calculator.svelte';
  import Devices from './apps/Devices.svelte';
  import Settings from './apps/Settings.svelte';
  import { routes, translate } from './i18n.js';
  import Details from './Details.svelte';
  import StatusBar from './shell/StatusBar.svelte';
  import Dock from './shell/Dock.svelte';
  import Window from './shell/Window.svelte';
  import QuickSettings from './shell/QuickSettings.svelte';
  import Icon from './shell/Icon.svelte';
  import { apps, shellTranslate } from './shell/labels.js';
  import { openWindow, nextWindow } from './shell/windows.js';

  let route = routes.includes(location.hash.slice(1)) ? location.hash.slice(1) : 'Home';
  let windows = route === 'Home' ? [] : openWindow([], route, innerWidth, innerHeight);
  let quickOpen = false, dark = true, volume = 100, navigating = false;
  const savedGeometry = {};
  let views = Object.fromEntries(routes.map(app => [app, { query: '', sort: 'newest', category: '', cursor: null, address: app === 'YouTube' ? 'https://www.youtube.com' : 'https://www.wikipedia.org' }]));
  let language = 'en', large = false, contrast = false, reduced = true, notifications = true, preferenceId = null;
  let error = '', notice = '', busy = false, connected = false, authKey = '', authNeeded = !native;
  let connectionChecked = false;
  let saveProgress = null;
  let privateTrash = [];
  let overview = {}, settings = {}, caps = {}, data = {};
  let selected = null, previewError = false, photoZoom = 1, photoMonth = '', confirmation = null, form = null, formValue = '', formName = '';
  let setup = false, setupStep = 0, setupHardware = null, hubName = '', copyFolder = '', importFolders = '', importScan = null, importChoice = 'later';
  let qr = null, pending = null, pairingDeadline = 0, remaining = 0;
  let records = [], note = null, noteTitle = '', noteBody = '', noteTrash = false, saveState = '', saveTimer, saveQueue = Promise.resolve(), noteRevision = 0;
  let tabs = [], previousTabs = [], bookmarks = [], downloads = [], permissionTab = null, requestedPermission = '', websitePermissions = [], allowDownloads = false;
  let playlists = [], playlist = null, music = [], playing = null, queuedTracks = [], shuffle = false, repeat = false;
  let expression = '', answer = '', history = [], historyId = null;
  let remoteSessions = [], remoteStatus = {};
  let hotspotName = '', hotspotPassword = '', updateCheck = null;
  let loadGeneration = 0, mounted = false, alertIds = new Set(), alertsInitialized = false;
  $: t = key => shellTranslate(language, key, key => translate(language, key));
  $: document.documentElement.dataset.osTheme = contrast ? 'high-contrast' : dark ? 'dark' : 'light';
  $: free = bytes(overview.free_bytes);
  $: used = overview.total_bytes > 0 ? Math.max(0, Math.min(100, 100 * (1 - overview.free_bytes / overview.total_bytes))) : undefined;
  $: document.documentElement.lang = language;
  $: document.title = `${t(route)} · Home Hub`;
  $: document.documentElement.dataset.large = String(large);
  $: document.documentElement.dataset.contrast = String(contrast);
  $: document.documentElement.dataset.reduced = String(reduced);
  $: musicTracks = tracksForPlaylist(music, playlist);
  $: visibleNotes = records.filter(r => `${r.title}\n${r.body}`.toLowerCase().includes(views.Notes.query.toLowerCase()));
  const date = value => value ? new Date(value).toLocaleString(language === 'hi' ? 'hi-IN' : 'en-IN') : '—';
  const progress = row => row.size ? Math.min(100, Math.round(100 * row.bytes_verified / row.size)) : 0;
  const message = e => translate(language, String(e?.message || e));
  const hub = createHubState(key => translate(language, key));
  hub.state.subscribe(status => { busy = status.busy; error = status.error; notice = status.notice; });
  async function run(action, success = '') { return hub.run(action, success); }
  async function preference() {
    // The existing browser preference reader expects an object, not JSON null.
    if (!native) {
      try { if (json(localStorage.getItem('hh-interface'), {}) === null) localStorage.setItem('hh-interface', '{}'); } catch { /* Storage can be unavailable in private browsing. */ }
    }
    const value = await loadInterface();
    preferenceId = value.id; language = value.language; large = value.large;
    contrast = value.contrast; reduced = value.reduced; notifications = value.notifications;
  }
  async function savePreferences() {
    try { preferenceId = await saveInterface({ id: preferenceId, language, large, contrast, reduced, notifications }); }
    catch (e) { error = message(e); }
  }
  async function loadPrivateTrash() { privateTrash = await privateTrashList(); }
  async function load() {
    const generation = ++loadGeneration, current = route;
    const { query, sort, category } = views[current];
    const put = (key, value) => { if (generation === loadGeneration) data = { ...data, [key]: value }; };
    if (['Notes','Browser','YouTube','Calculator'].includes(current)) {
      if (!native && current !== 'Calculator') return;
      if (current === 'Notes') records = await command('local_list', { kind: 'note', query: '', trash: noteTrash });
      if (current === 'Calculator') await loadCalculatorHistory();
      if (current === 'Browser' || current === 'YouTube') { tabs = await command('browser_list'); previousTabs = await command('browser_previous'); bookmarks = await command('local_list', { kind: 'bookmark', query: '', trash: false }); downloads = await command('local_list', { kind: 'download', query: '', trash: false }); }
      return;
    }
    if (current === 'Home') {
      const snapshot = await hub.overview(); const values = [snapshot.overview, snapshot.settings, snapshot.alerts];
      if (generation !== loadGeneration) return;
      [overview, settings] = values; const alerts = items(values[2]); const fresh = alerts.find(a => !alertIds.has(a.id)); if (notifications && alertsInitialized && fresh) notice = fresh.message; alertIds = new Set(alerts.map(a => a.id)); alertsInitialized = true; put('alerts', alerts); connected = true; authNeeded = false;
      if (!settings.setup_complete && !mounted) { setup = true; hubName = settings.hub_name; copyFolder = settings.second_copy_root || ''; }
    } else if (current === 'Files' || current === 'Music') {
      const params = new URLSearchParams({ limit: '100', sort, ...(query ? { q: query } : {}), ...(current === 'Music' ? { category: 'music' } : category ? { category } : {}) });
      const value = await hub.files(params);
      if (generation !== loadGeneration) return;
      if (current === 'Files') put('files', items(value)); views[current].cursor = value.next_cursor;
      if (current === 'Music') { music = items(value); if (native) playlists = await command('local_list', { kind: 'playlist', query: '', trash: false }); }
    } else if (current === 'Photos') {
      const params = new URLSearchParams({ limit: '100' });
      if (photoMonth) { const [year, month] = photoMonth.split('-'); params.set('year', year); params.set('month', month); }
      const value = await api(`/api/photos/timeline?${params}`); put('photos', items(value)); views[current].cursor = value.next_cursor;
      put('years', await api('/api/photos/years'));
    } else if (current === 'Transfers') { put('transfers', items(await api('/api/transfers'))); put('jobs', items(await api('/api/jobs'))); }
    else if (current === 'Storage') { put('storage', await api('/api/storage')); put('jobs', items(await api('/api/jobs'))); put('trash', items(await api('/api/trash'))); put('duplicates', items(await api('/api/duplicates'))); put('similar', items(await api('/api/similar'))); }
    else if (current === 'Devices') { put('devices', items(await api('/api/devices'))); put('activity', items(await api('/api/activity?limit=50'))); pending = (await api('/api/pair/pending')).pending; }
    else if (current === 'Hardware') { put('hardware', await api('/api/hardware/audit')); put('network', await api('/api/network')); caps = await api('/api/capabilities'); }
    else if (current === 'Settings' || current === 'Remote') { if (current === 'Settings' && native) await loadPrivateTrash(); settings = await api('/api/settings'); hubName = settings.hub_name; copyFolder = settings.second_copy_root || ''; caps = await api('/api/capabilities'); if (current === 'Remote') { put('devices', items(await api('/api/devices'))); remoteStatus = await api('/api/remote/status'); remoteSessions = items(await api('/api/screen/sessions')); } }
    connected = true; connectionChecked = true; authNeeded = false;
  }
  async function refresh() { try { await load(); } catch (e) { error = message(e); if (!['Notes','Browser','YouTube','Calculator'].includes(route)) { connected = false; connectionChecked = true; } } }
  async function go(next) {
    if (busy || navigating || !routes.includes(next)) return;
    navigating = true;
    try {
      await flushNote();
      if (saveState === 'error') return;
      const existing = windows.some(w => w.app === next);
      if (next !== 'Home') windows = openWindow(windows, next, innerWidth, innerHeight, savedGeometry[next]);
      route = next; selected = null; confirmation = null; quickOpen = false;
      location.hash = next;
      if (!existing || next === 'Home') await run(load);
      await tick(); document.getElementById('content')?.focus();
    } finally { navigating = false; }
  }
  async function windowAction(app, close = false) {
    if (busy || navigating) return;
    navigating = true;
    try {
      await flushNote();
      if (saveState === 'error') return;
      if (close && app === 'Calculator') await saveCalculatorHistory();
      const win = windows.find(w => w.app === app);
      if (!win) return;
      savedGeometry[app] = { ...win.rect };
      windows = close ? windows.filter(w => w.app !== app) : windows.map(w => w.app === app ? { ...w, minimized: true } : w);
      if (app === route) { route = nextWindow(windows); location.hash = route; }
      await tick(); document.getElementById('content')?.focus();
    } catch (e) { error = message(e); } finally { navigating = false; }
  }
  async function lock() {
    await flushNote();
    if (saveState === 'error') return;
    try { if (native) await emit('hh-lock'); }
    catch (e) { error = message(e); return; }
    notice = t('Use your system lock shortcut. Secure lock is not connected to this desktop yet.');
  }
  function changeVolume(value) {
    volume = value;
    document.querySelectorAll('audio,video').forEach(player => { player.volume = value / 100; });
  }
  function mediaVolume(node) {
    const play = event => { if (event.target instanceof HTMLMediaElement) event.target.volume = volume / 100; };
    node.addEventListener('play', play, true);
    return { destroy: () => node.removeEventListener('play', play, true) };
  }
  async function more() {
    const current = route, { cursor, query, sort, category } = views[current];
    if (!cursor) return;
    const key = current === 'Photos' ? 'photos' : 'files';
    const params = new URLSearchParams({ cursor, limit: '100', sort, ...(query ? { q: query } : {}), ...(current === 'Music' ? { category: 'music' } : category ? { category } : {}) });
    if (current === 'Photos' && photoMonth) { const [year, month] = photoMonth.split('-'); params.set('year', year); params.set('month', month); }
    const value = await api(current === 'Photos' ? `/api/photos/timeline?${params}` : `/api/files?${params}`);
    if (current === 'Music') music = [...music, ...items(value)];
    else data = { ...data, [key]: [...(data[key] || []), ...items(value)] };
    views[current].cursor = value.next_cursor;
  }
  async function mutate(path, method = 'POST', body) { await api(path, method, body); await load(); }
  function confirm(title, action, detail = '') { confirmation = { title, action, detail }; }
  async function accept() { const value = confirmation; confirmation = null; await run(value.action, t('Saved')); }
  async function pair() { await api('/api/pair/open', 'POST'); qr = await api('/api/pair/qr'); pairingDeadline = Date.now() + qr.expires_in * 1000; remaining = qr.expires_in; }
  async function finishSetup() { await api('/api/setup', 'POST', { hub_name: hubName, second_copy_root: copyFolder, setup_complete: true, remote_enabled: false }); setup = false; await load(); }
  async function nextSetup() { if (setupStep === 3 && importScan && importChoice !== 'later') await startImport(); setupStep++; }
  async function saveSettings() { settings = await api('/api/settings', 'PATCH', { hub_name: hubName, second_copy_root: copyFolder, remote_enabled: settings.remote_enabled, screen_enabled: settings.screen_enabled, update_enabled: settings.update_enabled, update_manifest_url: settings.update_manifest_url || '', update_pubkey_hex: settings.update_pubkey_hex || '', backup_interval_minutes: Number(settings.backup_interval_minutes), scrub_interval_hours: Number(settings.scrub_interval_hours), second_copy_interval_minutes: Number(settings.second_copy_interval_minutes), wake_enabled: settings.wake_enabled, wake_time: settings.wake_time }); updateCheck = null; }
  function viewFile(file) { previewError = false; photoZoom = 1; selected = file; }
  async function shareSelected() {
    const file = selected; if (!file) return;
    if (!navigator.share || !file.size || file.size > 25 * 1024 * 1024) { await saveFile(file); notice = t('Saved a copy for sharing'); return; }
    try {
      const response = await fetch(media(file.id));
      if (!response.ok) throw new Error('Unable to read the file');
      const shared = new File([await response.blob()], file.name, { type: file.mime || 'application/octet-stream' });
      if (navigator.canShare && !navigator.canShare({ files: [shared] })) throw new Error('File sharing unavailable');
      await navigator.share({ files: [shared], title: file.name });
    } catch (e) {
      if (e?.name === 'AbortError') return;
      await saveFile(file); notice = t('Saved a copy for sharing');
    }
  }
  function fileForm(file, action) { form = { file, action }; formValue = action === 'Rename' ? file.name : ''; formName = file.name; }
  async function submitForm() {
    if (form.action === 'Rename') await api(`/api/files/${form.file.id}`, 'PATCH', { name: formValue });
    else await api(`/api/files/${form.file.id}/${form.action.toLowerCase()}`, 'POST', { rel_path: formValue, ...(form.action === 'Copy' ? { name: formName } : {}) });
    form = null; await load();
  }
  async function scanImport() { importScan = await api('/api/import/scan', 'POST', { paths: importFolders.split('\n').map(s => s.trim()).filter(Boolean) }); }
  async function startImport() { if (importChoice === 'later') { importScan = null; return; } await api('/api/import', 'POST', { paths: importFolders.split('\n').map(s => s.trim()).filter(Boolean), mode: importChoice === 'keep' ? 'keep' : 'copy' }); importScan = null; importFolders = ''; await load(); }
  const notes = createNoteEditor(
    () => ({ records, note, noteTitle, noteBody, noteTrash, saveState }),
    value => { if ('records' in value) records = value.records; if ('note' in value) note = value.note; if ('noteTitle' in value) noteTitle = value.noteTitle; if ('noteBody' in value) noteBody = value.noteBody; if ('noteTrash' in value) noteTrash = value.noteTrash; if ('saveState' in value) saveState = value.saveState; },
    load, key => t(key), e => { error = message(e); }
  );
  const { queueNote, flushNote, chooseNote, newNote, noteAction, noteFile } = notes;
  const importNote = notes.noteImport;
  async function website(url = views[route].address, isolated = route === 'YouTube') { const label = await command(isolated ? 'youtube_open' : 'browser_open', { url }); await load(); return label; }
  async function restoreTabs() { try { await command('browser_restore'); } finally { await load(); } }
  async function browserAction(tab, action) { await command('browser_action', { label: tab.label, action, url: action === 'navigate' ? views[route].address : null }); await load(); }
  async function bookmark(tab) { await command('local_save', { id: null, kind: 'bookmark', title: tab.title, body: JSON.stringify({ url: tab.url }) }); await load(); }
  function permissions(tab) { permissionTab = tab; requestedPermission = ''; websitePermissions = tab.permissions.map(p => p.split('|').pop()); allowDownloads = tab.downloads; }
  async function savePermissions() { await command('browser_permissions', { label: permissionTab.label, downloads: allowDownloads, permissions: websitePermissions }); permissionTab = null; await load(); }
  async function newPlaylist() {
    const row = await createPlaylist(formValue.trim() || t('New playlist'));
    playlists = [row, ...playlists]; playlist = row; form = null;
  }
  async function addTrack(file, remove = false) {
    if (!playlist) return;
    playlist = await changePlaylist(playlist, file, remove);
    playlists = playlists.map(row => row.id === playlist.id ? playlist : row);
  }
  function nextTrack(direction = 1, automatic = false) {
    ({ playing, queuedTracks } = advanceTrack({ playing, queuedTracks, tracks: musicTracks, shuffle, repeat }, direction, automatic));
  }
  async function loadCalculatorHistory() {
    const saved = native ? (await command('local_list', { kind: 'preference', query: '', trash: false })).find(row => row.title === 'calculator-history') : null;
    historyId = saved?.id || null;
    let raw = saved?.body;
    if (!native) { try { raw = localStorage.getItem('hh-calculator-history'); } catch { raw = null; } }
    const value = json(raw, []);
    history = Array.isArray(value) ? value.filter(row => typeof row?.expression === 'string' && row.expression.length <= 512 && typeof row.answer === 'string').slice(0, 30) : [];
  }
  async function saveCalculatorHistory() {
    if (native) historyId = (await command('local_save', { id: historyId, kind: 'preference', title: 'calculator-history', body: JSON.stringify(history) })).id;
    else localStorage.setItem('hh-calculator-history', JSON.stringify(history));
  }
  async function calc() {
    try { const value = calculate(expression); answer = String(value); history = [{ expression, answer }, ...history].slice(0, 30); error = ''; await saveCalculatorHistory(); }
    catch (e) { answer = ''; error = message(e); }
  }
  async function unlock() { await authorize(authKey); authKey = ''; authNeeded = false; await load(); }
  async function diagnostics() { const value = await api('/api/diagnostics'); exportText(`homehub-diagnostics-${Date.now()}.json`, JSON.stringify(value, null, 2), 'application/json'); }
  async function checkUpdates() { updateCheck = await api('/api/update'); }
  async function stopSession(id) { await api(`/api/screen/sessions/${id}`, 'DELETE'); await load(); }
  function changeScope(device, scope, enabled) {
    const old = Array.isArray(device.scopes) ? device.scopes : String(device.scopes || '').split(',').filter(Boolean);
    const scopes = enabled ? [...new Set([...old, scope])] : old.filter(s => s !== scope);
    confirm(t('Device permissions'), () => mutate(`/api/devices/${device.id}`, 'PATCH', { scopes }), `${device.name}: ${scope}`);
  }
  function dialogFocus(node) {
    const previous = document.activeElement;
    node.tabIndex = -1;
    queueMicrotask(() => (node.querySelector('input,button,select,textarea') || node).focus());
    function keydown(event) {
      const modals = [...document.querySelectorAll('.modal')];
      if (modals.at(-1) !== node) return;
      if (event.key === 'Escape' && !busy) {
        event.preventDefault();
        const title = node.getAttribute('aria-labelledby');
        if (title === 'setup-title') setup = false;
        else if (title === 'pair-title') qr = null;
        else if (title === 'pending-title') pending = null;
        else if (title === 'viewer-title') selected = null;
        else if (title === 'confirm-title') { confirmation = null; refresh(); }
        else if (title === 'form-title') form = null;
        else if (title === 'permissions-title') permissionTab = null;
        return;
      }
      if (event.key !== 'Tab') return;
      const focusable = [...node.querySelectorAll('a,button,input,select,textarea,[tabindex="0"]')].filter(el => !el.disabled && el.offsetParent !== null);
      const first = focusable[0], last = focusable.at(-1);
      if (!first) { event.preventDefault(); node.focus(); return; }
      if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
    node.addEventListener('keydown', keydown);
    return { destroy() { node.removeEventListener('keydown',keydown); if (previous?.isConnected) previous.focus(); } };
  }
  onMount(() => {
    let timer, statusTimer, unlisteners = [], cancelled = false;
    (async () => { try { await preference(); if (native) {
      unlisteners.push(await listen('shell-close-requested', async () => { try { await flushNote(); if (saveState !== 'error') await command('finish_close'); } catch (e) { error = message(e); } }));
      unlisteners.push(await listen('file-save-progress', event => { saveProgress = event.payload; }));
      unlisteners.push(await listen('file-save-finished', () => { saveProgress = null; }));
      unlisteners.push(await listen('website-permission-request', event => { const request = event.payload; const tab = tabs.find(item => item.label === request.label); if (tab && !permissionTab) { permissions(tab); requestedPermission = `${request.origin}: ${request.permission}`; } }));
      await command('shell_ready');
    } await refresh(); mounted = true; if (!cancelled) timer = setInterval(async () => { remaining = Math.max(0, Math.ceil((pairingDeadline - Date.now()) / 1000)); if (qr && !remaining) { qr = null; pairingDeadline = 0; } if (document.hidden || busy) return; if (['Home','Transfers','Storage','Devices','Remote'].includes(route)) await refresh(); if (qr) try { pending = (await api('/api/pair/pending')).pending; } catch (e) { error = message(e); } }, 5000); } catch (e) { error = message(e); } })();
    const status = async () => {
      if (document.hidden || busy || route === 'Home') return;
      try { overview = await api('/api/overview'); connected = true; connectionChecked = true; }
      catch { connected = false; connectionChecked = true; }
    };
    statusTimer = setInterval(status, 30000); void status();
    const change = () => { const next = location.hash.slice(1); if (routes.includes(next) && next !== route) go(next); };
    const hiding = () => { if (document.hidden) flushNote(); else if (!busy && ['Home','Transfers','Storage','Devices','Remote'].includes(route)) refresh(); };
    window.addEventListener('hashchange', change); document.addEventListener('visibilitychange', hiding);
    return () => { cancelled = true; clearInterval(timer); clearInterval(statusTimer); notes.dispose(); for (const unlisten of unlisteners) unlisten(); window.removeEventListener('hashchange', change); document.removeEventListener('visibilitychange', hiding); };
  });
</script>

<a class="skip" href="#content">{t('Skip to content')}</a>
<div class="os-desktop" use:mediaVolume>
  <StatusBar {t} {language} active={route} {connected} {connectionChecked} {overview} {free} {quickOpen} {go} toggleQuick={() => quickOpen = !quickOpen} toggleLanguage={() => { language = language === 'en' ? 'hi' : 'en'; savePreferences(); }} {lock}/>
  <main id="content" class="os-workspace" tabindex="-1">
    <section class="os-home" hidden={route !== 'Home'} aria-label={t('Home')}>
      <div class="os-home-copy"><p class="eyebrow">{t('PRIVATE BY DESIGN')}</p><h1>{t('Your space, at home.')}</h1><p>{t('Everything you love. Close to home.')}</p></div>
      <div class="os-launcher" aria-label={t('Open apps')}>
        {#each apps.slice(1) as app}<button on:click={() => go(app)}><span class="os-app-icon" data-app={app}><Icon name={app} size={29}/></span><span>{t(app)}</span></button>{/each}
      </div>
      <div class="os-home-footer"><span><i class="os-status-dot" class:online={connected}></i>{t(connected ? 'Connected locally' : 'Private apps ready')}</span><span>{settings.hub_name || 'Home Hub'}</span></div>
      <nav class="os-utilities" aria-label={t('System tools')}>{#each ['Transfers','Storage','Remote','Hardware'] as app}<button on:click={() => go(app)}>{t(app)}</button>{/each}<button disabled={busy || !connected} on:click={() => run(pair)}>{t('Pair a device')}</button></nav>
    </section>
    {#each windows as win, order (win.app)}
      {@const app = win.app}
      <Window bind:win {t} active={route === app} desktop={route === 'Home'} {order} busy={busy || navigating} focus={() => go(app)} minimize={() => windowAction(app)} close={() => windowAction(app, true)}>
        <div class="os-app-toolbar"><span>{t(app)}</span><div class="toolbar"><button disabled={busy} on:click={() => run(load)}>{t('Refresh')}</button>{#if app === 'Devices'}<button class="primary" disabled={busy} on:click={() => run(pair)}>{t('Pair a device')}</button>{/if}</div></div>
    {#if app === 'Files'}
      <Files bind:data bind:query={views[app].query} bind:sort={views[app].sort} bind:category={views[app].category} bind:cursor={views[app].cursor} {t} {run} {load} {more} {mutate} {confirm} {viewFile} {fileForm}/>
    {:else if app === 'Photos'}
      <Photos route={app} bind:busy bind:connected bind:overview bind:settings bind:data bind:cursor={views[app].cursor} bind:photoMonth {t} {date} {run} {load} {more} {mutate} {viewFile} {go} {confirm}/>
    {:else if app === 'Transfers'}
      <section class="card"><h2>{t('Transfers')}</h2>{#each data.transfers || [] as transfer}<article class="transfer"><div class="row"><div><strong>{transfer.name}</strong><small>{transfer.device} · {t(transfer.status)}{transfer.error_code ? ` · ${transfer.error_code}` : ''}</small></div>{#if ['open','verifying'].includes(transfer.status)}<button on:click={() => confirm(t('Cancel'), () => mutate(`/api/transfers/${transfer.id}`,'DELETE'),transfer.name)}>{t('Cancel')}</button>{/if}</div><progress max="100" value={progress(transfer)} aria-label={transfer.name}></progress><small>{bytes(transfer.bytes_verified)} / {bytes(transfer.size)}{transfer.rate_bps ? ` · ${bytes(transfer.rate_bps)}/s` : ''}{transfer.eta_secs != null ? ` · ${transfer.eta_secs} s` : ''}</small></article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
    {:else if app === 'Storage'}
      <div class="metrics"><div class="card"><span>{t('Free space')}</span><strong>{bytes(data.storage?.free_bytes)}</strong><small>{bytes(data.storage?.total_bytes)}</small></div><div class="card"><h2>{t('Protect your memories')}</h2><p>{t('A single drive is not a backup.')}</p><div class="toolbar"><button disabled={busy} on:click={() => run(() => mutate('/api/second-copy/run'))}>{t('Create second copy')}</button><button disabled={busy} on:click={() => run(() => mutate('/api/storage/scrub'))}>{t('Run integrity check')}</button></div></div></div>
      <section class="card"><h2>{t('Storage')}</h2><Details {t} value={data.storage}/></section>
      <section class="card"><h2>{t('Import a copy')}</h2><p>{t('Original files stay where they are.')}</p><label>{t('Folder paths (one per line)')}<textarea rows="3" bind:value={importFolders}></textarea></label><button disabled={busy || !importFolders.trim()} on:click={() => run(scanImport)}>{t('Scan folders')}</button>{#if importScan}<Details {t} value={importScan}/><fieldset><legend>{t('Import a copy')}</legend>{#each [['later','Later'],['keep','Keep in place'],['copy','Import a copy']] as choice}<label class="check"><input type="radio" bind:group={importChoice} value={choice[0]}/>{t(choice[1])}</label>{/each}</fieldset><button on:click={() => run(startImport)}>{t('Confirm')}</button>{/if}</section>
      <section class="card"><h2>{t('Move library')}</h2><p>{t('Original files stay where they are.')}</p><label>{t('Library folder')}<input bind:value={formValue}/></label><button disabled={!formValue.trim() || busy} on:click={() => confirm(t('Move library'),() => mutate('/api/library/move','POST',{path:formValue}),formValue)}>{t('Move library')}</button></section>
      <section class="card"><h2>{t('Trash')}</h2>{#each data.trash || [] as file}<div class="row"><span>{file.name} · {bytes(file.size)}</span><div class="toolbar"><button on:click={() => run(() => mutate(`/api/trash/${file.id}/restore`))}>{t('Restore')}</button><button class="danger" on:click={() => confirm(t('Delete permanently'), () => mutate(`/api/trash/${file.id}`,'DELETE'),file.name)}>{t('Delete permanently')}</button></div></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
      <section class="card"><h2>{t('Review duplicates')}</h2>{#each data.duplicates || [] as group}<article><Details {t} value={group}/>{#each group.file_ids || [] as fileId}<button on:click={() => confirm(t('Review duplicates'), () => mutate(`/api/duplicates/${group.id}/resolve`,'POST',{keep_file_id:fileId}),fileId)}>{t('Keep in place')}: {fileId}</button>{/each}</article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
      <section class="card"><h2>{t('Similar photos')}</h2><p>{t('These are suggestions only. No photos are removed automatically.')}</p><div class="toolbar"><button disabled={busy} on:click={() => run(() => mutate('/api/duplicates/scan'))}>{t('Scan for similar photos')}</button><button disabled={busy} on:click={() => run(load)}>{t('Refresh')}</button></div>{#each data.similar || [] as group}<article>{#each group.files || [] as file}<div class="row"><img src={`/api/files/${file.id}/thumb`} alt={file.name} width="64" height="64" loading="lazy"/><span>{file.name}</span></div>{/each}</article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
    {:else if app === 'Notes'}
      <Notes bind:error bind:busy bind:query={views[app].query} bind:note bind:noteTitle bind:noteBody bind:noteTrash bind:saveState {t} {visibleNotes} {date} {run} {load} {confirm} {queueNote} {flushNote} {chooseNote} {newNote} {noteAction} {noteFile} {importNote}/>
    {:else if app === 'Browser' || app === 'YouTube'}
      {#if app === 'Browser'}<Browser bind:address={views[app].address} bind:tabs bind:previousTabs bind:bookmarks bind:downloads {t} {run} {load} {confirm} {website} {restoreTabs} {browserAction} {bookmark} {permissions}/>{:else}<YouTube bind:address={views[app].address} bind:tabs bind:previousTabs bind:bookmarks bind:downloads {t} {run} {load} {confirm} {website} {restoreTabs} {browserAction} {bookmark} {permissions}/>{/if}
    {:else if app === 'Music'}
      <Music bind:error bind:cursor={views[app].cursor} bind:playlists bind:playlist bind:music bind:playing bind:queuedTracks bind:shuffle bind:repeat bind:form bind:formValue {t} {musicTracks} {run} {load} {more} {confirm} {addTrack} {nextTrack}/>
    {:else if app === 'Calculator'}
      <Calculator bind:error bind:expression bind:answer bind:history {t} {calc} {saveCalculatorHistory} {message}/>
    {:else if app === 'Devices'}
      <Devices bind:data bind:form bind:formValue {t} {date} {mutate} {confirm} {changeScope}/>
    {:else if app === 'Hardware'}
      <section class="card"><h2>{t('Hardware')}</h2><button disabled={busy} on:click={() => run(async () => { data = { ...data, hardware: await api('/api/hardware/audit/run','POST') }; })}>{t('Run hardware check')}</button><Details {t} value={data.hardware}/></section><section class="card"><h2>{t('Connected locally')}</h2><Details {t} value={data.network}/><p>{t('Wake support varies by laptop. Files stay queued when the Hub is asleep.')}</p><Details {t} value={caps}/></section>{#if caps.hotspot}<section class="card"><h2>{t('Windows hotspot')}</h2><p>{t('Available through the Windows session helper.')}</p><label>{t('Network name')}<input bind:value={hotspotName} maxlength="32" autocomplete="off"/></label><label>{t('Password')}<input type="password" bind:value={hotspotPassword} minlength="8" maxlength="63" autocomplete="new-password"/></label><div class="toolbar"><button disabled={busy || !hotspotName || hotspotPassword.length < 8} on:click={() => run(async () => { await api('/api/network/hotspot','POST',{enable:true,ssid:hotspotName,passphrase:hotspotPassword}); hotspotPassword=''; })}>{t('Turn on hotspot')}</button><button disabled={busy} on:click={() => run(() => api('/api/network/hotspot','POST',{enable:false}))}>{t('Turn off hotspot')}</button></div></section>{/if}
    {:else if app === 'Remote'}
      <section class="card"><h2>{t('Remote')}</h2><p>{t('Remote is off until you grant a device permission.')}</p><label class="check"><input type="checkbox" bind:checked={settings.remote_enabled} disabled={caps.helper_available === false} on:change={() => run(saveSettings)}/>{t('Enable remote control')}</label><Details {t} value={remoteStatus}/><button on:click={() => go('Devices')}>{t('Device permissions')}</button><p>{t('Wake support varies by laptop. Files stay queued when the Hub is asleep.')}</p></section><section class="card"><h2>{t('Remote sessions')}</h2><p>{t('Start Cast on your phone to show its screen on this laptop. Video only; the phone keeps control.')}</p>{#each remoteSessions as session}<div class="row"><span>{session.device_name || session.device_id || session.id} · {t(session.direction)} · {t(session.state || session.status)}</span><button on:click={() => run(() => stopSession(session.id))}>{t('Stop session')}</button></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
    {:else if app === 'Settings'}
      <Settings bind:language bind:large bind:contrast bind:reduced bind:notifications bind:busy bind:settings bind:setup bind:setupStep bind:hubName bind:copyFolder bind:privateTrash bind:updateCheck {t} {run} {load} {confirm} {saveSettings} {flushNote} {savePreferences} {diagnostics} {checkUpdates} {loadPrivateTrash}/>
    {/if}

    {#if app === 'Settings' && native}<section class="card"><h2>{t('Private app trash')}</h2>{#each privateTrash as row}<div class="row"><span>{row.title} · {t(row.kind)}</span><div class="toolbar"><button on:click={() => run(async () => { await command('local_restore',{id:row.id}); await loadPrivateTrash(); })}>{t('Restore')}</button><button class="danger" on:click={() => confirm(t('Delete permanently'),async () => { await command('local_purge',{id:row.id}); await loadPrivateTrash(); },row.title)}>{t('Delete permanently')}</button></div></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>{/if}
    {#if ['Transfers','Storage'].includes(app)}<section class="card"><h2>{t('Jobs')}</h2>{#each data.jobs || [] as job}<article class="transfer"><div class="row"><strong>{t(job.kind)} · {t(job.status)}</strong>{#if ['queued','running'].includes(job.status)}<button disabled={job.cancel_requested} on:click={() => confirm(t('Cancel'),() => mutate(`/api/jobs/${job.id}`,'DELETE'),job.kind)}>{t('Cancel')}</button>{/if}</div><progress max={Math.max(1,job.total)} value={job.done} aria-label={job.kind}></progress><small>{job.done} / {job.total}{job.error ? ` · ${job.error}` : ''}</small>{#if job.result}<Details {t} value={job.result}/>{/if}</article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>{/if}
      </Window>
    {/each}
  </main>
  <div class="os-notifications">
    {#if data.alerts?.length}<details class="card os-alerts"><summary>{t('Alerts')} · {data.alerts.length}</summary>{#each data.alerts || [] as alert}<div class="row"><div><strong>{alert.message}</strong><small>{date(alert.created_at)}</small></div><button on:click={() => run(() => mutate(`/api/alerts/${alert.id}/ack`))}>{t('Confirm')}</button></div>{:else}<p class="muted">{t('Nothing here yet.')}</p>{/each}</details>{/if}
    {#if error}<div class="banner error" role="alert"><span>{error}</span><button on:click={() => run(load)} disabled={busy}>{t('Retry')}</button><button on:click={() => error = ''} aria-label={t('Close')}>×</button></div>{/if}
    {#if notice}<div class="banner success" role="status"><span>{notice}</span><button on:click={() => notice = ''} aria-label={t('Close')}>×</button></div>{/if}
    {#if busy}<p class="busy" role="status">{t('Working…')}</p>{/if}
    {#if saveProgress}<section class="card" role="status"><div class="row"><span>{saveProgress.name} · {bytes(saveProgress.done)} / {bytes(saveProgress.total)}</span><button on:click={() => command('cancel_save',{id:saveProgress.id}).catch(e => error = message(e))}>{t('Cancel')}</button></div><progress max={saveProgress.total || 1} value={saveProgress.done} aria-label={saveProgress.name}></progress></section>{/if}
    {#if authNeeded && !native}<section class="card auth"><h2>{t('Unlock dashboard')}</h2><p>{t('No secret is included in this page. Use the installed app, or enter its local session key.')}</p><form on:submit|preventDefault={() => run(unlock)}><label>{t('Local session key')}<input type="password" autocomplete="off" bind:value={authKey} required /></label><button class="primary" disabled={busy}>{t('Unlock dashboard')}</button></form></section>{/if}

  </div>
  <Dock {t} {windows} active={route} {go}/>
  {#if quickOpen}<QuickSettings {t} {connected} {overview} {free} {used} bind:volume bind:dark bind:contrast {busy} pair={() => run(pair)} {go} close={() => quickOpen = false} preference={savePreferences} {changeVolume}/>{/if}
</div>

{#if setup}
  <div class="overlay"><div use:dialogFocus class="modal" role="dialog" aria-modal="true" aria-labelledby="setup-title">
    <h2 id="setup-title">{t('Welcome to Home Hub')}</h2><p>{setupStep + 1} / 5</p>
    {#if setupStep === 0}
      <label>{t('Hub name')}<input bind:value={hubName} maxlength="120"/></label>
      <label>{t('Library folder')}<input value={settings.library_root || ''} readonly/></label>
      <p>{t('Original files stay where they are.')}</p>
      <button on:click={() => { setup = false; go('Storage'); }}>{t('Choose a different library folder in Storage')}</button>
    {:else if setupStep === 1}
      <h3>{t('Protect your memories')}</h3><p>{t('A single drive is not a backup.')}</p>
      <label>{t('Second-copy folder')} · {t('Optional')}<input bind:value={copyFolder}/></label>
      <p>{t('Choose an external drive for a second copy.')}</p><p>{t('Turn on BitLocker in Windows to protect data at rest.')}</p>
    {:else if setupStep === 2}
      <h3>{t('Hardware')}</h3><p>{t('Check this laptop before enabling hardware features.')}</p>
      <button disabled={busy} on:click={() => run(async () => { setupHardware = await api('/api/hardware/audit/run','POST'); })}>{t('Run hardware check')}</button>
      {#if setupHardware}<Details {t} value={setupHardware}/>{/if}
    {:else if setupStep === 3}
      <h3>{t('Existing files')}</h3><p>{t('Original files stay where they are.')}</p>
      <label>{t('Folder paths (one per line)')}<textarea rows="3" bind:value={importFolders} on:input={() => importScan = null}></textarea></label>
      <button disabled={busy || !importFolders.trim()} on:click={() => run(scanImport)}>{t('Scan folders')}</button>
      {#if importScan}<Details {t} value={importScan}/><fieldset><legend>{t('Existing files')}</legend>{#each [['later','Later'],['keep','Keep in place'],['copy','Import a copy']] as choice}<label class="check"><input type="radio" bind:group={importChoice} value={choice[0]}/>{t(choice[1])}</label>{/each}</fieldset>{/if}
      <p>{t('Later is selected unless you choose an import option.')}</p>
    {:else}
      <h3>{t('Pair a device')}</h3><p>{t('Remote is off until you grant a device permission.')}</p>
      <button on:click={() => run(pair)}>{t('Pair a device')}</button>
    {/if}
    <div class="toolbar">
      {#if setupStep > 0}<button on:click={() => setupStep--}>{t('Back')}</button>{/if}
      {#if setupStep < 4}<button class="primary" disabled={busy || !hubName.trim() || (setupStep === 3 && !!importFolders.trim() && !importScan)} on:click={() => run(nextSetup)}>{t('Next')}</button>
      {:else}<button class="primary" disabled={busy} on:click={() => run(finishSetup)}>{t('Finish setup')}</button>{/if}
      <button on:click={() => setup = false}>{t('Later')}</button>
    </div>
  </div></div>
{/if}
{#if qr}<div class="overlay"><div use:dialogFocus class="modal pairing" role="dialog" aria-modal="true" aria-labelledby="pair-title"><h2 id="pair-title">{t('Pair a device')}</h2>{#if qr.qr}<img src={qr.qr} alt={t('Pairing QR code')}/>{/if}<p>{t('Manual code')}: <strong>{qr.manual_code}</strong></p><p>{t('Pairing expires in')} {remaining} {t('seconds')}</p><button on:click={() => qr = null}>{t('Close')}</button></div></div>{/if}
{#if pending}<div class="overlay"><div use:dialogFocus class="modal" role="dialog" aria-modal="true" aria-labelledby="pending-title"><h2 id="pending-title">{t('Device permissions')}</h2><Details {t} value={{ name:pending.device_name || pending.name,platform:pending.platform }}/><div class="toolbar"><button class="primary" on:click={() => run(async () => { await api('/api/pair/confirm','POST',{request_id:pending.request_id,allow:true}); pending = null; })}>{t('Approve')}</button><button on:click={() => run(async () => { await api('/api/pair/confirm','POST',{request_id:pending.request_id,allow:false}); pending = null; })}>{t('Deny')}</button></div></div></div>{/if}
{#if selected}<div class="overlay"><div use:dialogFocus class="modal viewer" role="dialog" aria-modal="true" aria-labelledby="viewer-title"><div class="row"><h2 id="viewer-title">{selected.name}</h2><button on:click={() => selected = null}>{t('Close')}</button></div>{#if selected.mime?.startsWith('image/')}<div class="photo-canvas"><img src={media(selected.id)} alt={selected.name} style:transform={`scale(${photoZoom})`} on:error={() => previewError = true}/></div><div class="toolbar"><button on:click={() => photoZoom = Math.max(1, photoZoom - 0.5)} disabled={photoZoom === 1} aria-label={t('Zoom out')}>−</button><span>{Math.round(photoZoom * 100)}%</span><button on:click={() => photoZoom = Math.min(6, photoZoom + 0.5)} disabled={photoZoom === 6} aria-label={t('Zoom in')}>+</button></div>{:else if selected.mime?.startsWith('video/')}<video controls autoplay src={media(selected.id)} aria-label={selected.name} on:error={() => previewError = true}><track kind="captions" src={selected.captions_id ? media(selected.captions_id) : undefined} srclang={language} label={selected.captions_id ? t('Captions') : t('Captions unavailable')}/></video>{#if !selected.captions_id}<p class="muted">{t('Captions are unavailable for this video.')}</p>{/if}{:else if selected.mime?.startsWith('audio/')}<audio controls src={media(selected.id)} aria-label={selected.name} on:error={() => previewError = true}></audio>{:else}<Details {t} value={{category:selected.category,size:bytes(selected.size),created:date(selected.created_at)}}/>{/if}{#if previewError}<p role="status">{t('This format cannot be previewed here. Download it to open in another app.')}</p>{/if}{#if selected.taken_at}<Details {t} value={{taken:date(selected.taken_at),dimensions:selected.width && selected.height ? `${selected.width} × ${selected.height}` : null,camera:[selected.camera_make,selected.camera_model].filter(Boolean).join(" ") || null,duration:selected.duration_ms ? `${Math.round(selected.duration_ms / 1000)} s` : null}}/>{/if}<div class="toolbar"><button on:click={() => run(shareSelected)}>{t('Share')}</button><button on:click={() => run(() => saveFile(selected))}>{t('Download')}</button></div></div></div>{/if}
{#if confirmation}<div class="overlay"><div use:dialogFocus class="modal" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title"><h2 id="confirm-title">{confirmation.title}</h2><p>{confirmation.detail}</p><p>{t('This action changes stored data.')}</p><div class="toolbar"><button class="primary" disabled={busy} on:click={accept}>{t('Confirm')}</button><button on:click={() => { confirmation = null; refresh(); }}>{t('Cancel')}</button></div></div></div>{/if}
{#if form}<div class="overlay"><div use:dialogFocus class="modal" role="dialog" aria-modal="true" aria-labelledby="form-title"><h2 id="form-title">{t(form.action === 'playlist' ? 'New playlist' : form.action === 'device' ? 'Rename' : form.action)}</h2><form on:submit|preventDefault={() => run(form.action === 'playlist' ? newPlaylist : form.action === 'device' ? async () => { await api(`/api/devices/${form.file.id}`,'PATCH',{name:formValue}); form = null; await load(); } : submitForm)}><label>{t(['Move','Copy'].includes(form.action) ? 'Library folder' : 'Name')}<input bind:value={formValue} required/></label>{#if form.action === 'Copy'}<label>{t('Name')}<input bind:value={formName} required/></label>{/if}<div class="toolbar"><button class="primary">{t('Save')}</button><button type="button" on:click={() => form = null}>{t('Cancel')}</button></div></form></div></div>{/if}
{#if permissionTab}<div class="overlay"><div use:dialogFocus class="modal" role="dialog" aria-modal="true" aria-labelledby="permissions-title"><h2 id="permissions-title">{t('Website permissions')}</h2><p>{permissionTab.url}</p>{#if requestedPermission}<p>{t("Website requested")}: {requestedPermission}. {t("Grant permission and retry the website action.")}</p>{/if}<p>{t('Changing permissions resets this tab’s website session and reloads the page.')}</p><fieldset><legend>{t('Website permissions')}</legend>{#each ['camera','microphone','location','notifications','protected_media'] as permission}<label class="check"><input type="checkbox" bind:group={websitePermissions} value={permission}/>{t(permission)}</label>{/each}<label class="check"><input type="checkbox" bind:checked={allowDownloads}/>{t('Allow downloads')}</label></fieldset><div class="toolbar"><button class="primary" on:click={() => run(savePermissions)}>{t('Save')}</button><button on:click={() => permissionTab = null}>{t('Cancel')}</button></div></div></div>{/if}
