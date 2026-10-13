<script>
  import {
    api,
    items,
    bytes,
    native,
    albumsList,
    albumsCreate,
    albumsUpdate,
    albumsDelete,
    albumsItemsList,
    albumsItemsAdd,
    albumsItemsRemove,
    backupSummary,
    media
  } from '../api.js';
  import { onMount } from 'svelte';
  import Details from '../Details.svelte';
  import PhotoTile from '../PhotoTile.svelte';

  export let route;
  export let busy;
  export let connected;
  export let overview;
  export let settings;
  export let data;
  export let cursor;
  export let photoMonth;
  export let t;
  export let date;
  export let run;
  export let load;
  export let more;
  export let mutate;
  export let viewFile;
  export let go;
  export let confirm;

  // Preserve exported props contracts
  $: void [route, busy, connected, overview, settings, mutate, go];

  // Local UI State
  let albums = [];
  let album = null;
  let albumItems = [];
  let albumCursor = null;
  let albumListCursor = null;
  let albumError = '';
  let showNewAlbum = false;
  let albumName = '';
  let renaming = false;
  let renameValue = '';
  let backup = null;
  let showBackupPopover = false;

  // Gallery Display Controls
  let mediaFilter = 'all'; // 'all' | 'photos' | 'videos'
  let layoutMode = 'moments'; // 'moments' | 'grid'
  let gridSize = 190; // Zoom slider (130px - 320px)
  let activeMenuPhotoId = null;
  let showAddPhotosModal = false;

  const alphabet = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

  function newAlbumId() {
    let time = Date.now(), id = '';
    for (let i = 0; i < 10; i++) {
      id = alphabet[time % 32] + id;
      time = Math.floor(time / 32);
    }
    const random = crypto.getRandomValues(new Uint8Array(16));
    return id + Array.from(random, value => alphabet[value % 32]).join('');
  }

  $: libraryPhotos = Array.isArray(data?.photos) ? data.photos : [];
  $: monthSummaries = items(data?.years);
  $: photoMap = new Map(libraryPhotos.map(p => [p.file_id, p]));

  $: albumPhotos = (albumItems || []).map(item => {
    const existing = photoMap.get(item.file_id);
    if (existing) return existing;
    return {
      file_id: item.file_id,
      id: item.file_id,
      name: 'Photo',
      taken_at: item.added_at,
      thumb_status: 'ready',
      type_: 'photo',
      mime: 'image/jpeg'
    };
  });

  $: basePhotos = album ? albumPhotos : libraryPhotos;

  $: filteredPhotos = basePhotos.filter(p => {
    const isVideo = p.type_ === 'video' || p.mime?.startsWith('video/');
    if (mediaFilter === 'videos') return isVideo;
    if (mediaFilter === 'photos') return !isVideo;
    return true;
  });

  $: photoCount = basePhotos.filter(p => p.type_ !== 'video' && !p.mime?.startsWith('video/')).length;
  $: videoCount = basePhotos.filter(p => p.type_ === 'video' || p.mime?.startsWith('video/')).length;

  // Group photos by date for authentic Apple Photos "Moments" view
  $: groupedPhotos = groupPhotosByDate(filteredPhotos);

  function groupPhotosByDate(photos) {
    if (!photos || photos.length === 0) return [];
    const groups = [];
    const map = new Map();

    for (const p of photos) {
      const d = p.taken_at ? new Date(p.taken_at) : null;
      let label = t('Undated');
      let sublabel = '';
      if (d && !isNaN(d.getTime())) {
        label = d.toLocaleDateString(undefined, {
          month: 'long',
          day: 'numeric',
          year: 'numeric'
        });
        sublabel = d.toLocaleDateString(undefined, { weekday: 'long' });
      }

      if (!map.has(label)) {
        const group = { label, sublabel, items: [] };
        map.set(label, group);
        groups.push(group);
      }
      map.get(label).items.push(p);
    }
    return groups;
  }

  function formatMonthLabel(year, month) {
    if (!year || !month) return '';
    try {
      const d = new Date(Number(year), Number(month) - 1, 1);
      return d.toLocaleDateString(undefined, { month: 'short', year: 'numeric' });
    } catch {
      return `${year} / ${month}`;
    }
  }

  function formatShortDate(val) {
    if (!val) return '—';
    try {
      const d = new Date(val);
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
    } catch {
      return String(val);
    }
  }

  function formatTime(val) {
    if (!val) return '';
    try {
      const d = new Date(val);
      return d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
    } catch {
      return '';
    }
  }

  async function refreshAlbums() {
    if (!native) return;
    try {
      const page = await albumsList();
      albums = items(page);
      albumListCursor = page.next_cursor;
      albumError = '';
    } catch (e) {
      albumError = String(e?.message || e);
    }
  }

  async function openAlbum(row) {
    album = row;
    renaming = false;
    activeMenuPhotoId = null;
    photoMonth = '';
    try {
      const page = await albumsItemsList(row.id);
      albumItems = items(page);
      albumCursor = page.next_cursor;
      albumError = '';
    } catch (e) {
      albumError = String(e?.message || e);
    }
  }

  async function albumAction(action) {
    try {
      await action();
      await refreshAlbums();
      if (album) {
        const refreshed = albums.find(a => a.id === album.id) || album;
        await openAlbum(refreshed);
      }
    } catch (e) {
      albumError = String(e?.message || e);
    }
  }

  async function createAlbum() {
    const name = albumName.trim();
    if (!name) return;
    await albumAction(async () => {
      const newId = newAlbumId();
      await albumsCreate(newId, name);
      albumName = '';
      showNewAlbum = false;
    });
  }

  function startRename(target) {
    if (!target) return;
    album = target;
    renameValue = target.name;
    renaming = true;
  }

  async function saveRename() {
    if (!album || !renameValue.trim()) return;
    await albumAction(async () => {
      album = await albumsUpdate(album.id, album.name, renameValue.trim());
      renaming = false;
    });
  }

  function deleteAlbum(target) {
    if (!target) return;
    confirm(
      t('Delete album? Photos stay in your library.'),
      async () => {
        await albumsDelete(target.id);
        if (album?.id === target.id) {
          album = null;
          albumItems = [];
        }
        await refreshAlbums();
      },
      target.name
    );
  }

  async function loadMoreAlbums() {
    if (!albumListCursor) return;
    try {
      const page = await albumsList(albumListCursor);
      albums = [...albums, ...items(page)];
      albumListCursor = page.next_cursor;
    } catch (e) {
      albumError = String(e?.message || e);
    }
  }

  async function loadMoreAlbum() {
    if (!album || !albumCursor) return;
    try {
      const page = await albumsItemsList(album.id, albumCursor);
      albumItems = [...albumItems, ...items(page)];
      albumCursor = page.next_cursor;
    } catch (e) {
      albumError = String(e?.message || e);
    }
  }

  async function addPhotoToAlbum(albumId, fileId) {
    if (!fileId || !albumId) return;
    await albumAction(() => albumsItemsAdd(albumId, [fileId]));
  }

  async function removePhotoFromAlbum(albumId, fileId) {
    if (!fileId || !albumId) return;
    await albumAction(() => albumsItemsRemove(albumId, [fileId]));
  }

  onMount(() => {
    if (route !== 'Photos') return;
    if (native) {
      void refreshAlbums();
      backupSummary().then(value => backup = value).catch(e => albumError = String(e?.message || e));
    }
  });
</script>

<div
  class="photos-app"
  style="--grid-size: {gridSize}px;"
>
  <!-- Sleek macOS Photos Sidebar (210px) -->
  <aside class="photos-sidebar">
    <div class="sidebar-top">
      <div class="app-brand">
        <div class="brand-flower-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="12" cy="12" r="3" />
            <path d="M12 2a3.5 3.5 0 0 0-3.5 3.5C8.5 7.5 12 12 12 12s3.5-4.5 3.5-6.5A3.5 3.5 0 0 0 12 2z" />
            <path d="M12 22a3.5 3.5 0 0 0 3.5-3.5c0-2-3.5-6.5-3.5-6.5s-3.5 4.5-3.5 6.5A3.5 3.5 0 0 0 12 22z" />
            <path d="M2 12a3.5 3.5 0 0 0 3.5 3.5c2 0 6.5-3.5 6.5-3.5S7.5 8.5 5.5 8.5A3.5 3.5 0 0 0 2 12z" />
            <path d="M22 12a3.5 3.5 0 0 0-3.5-3.5c-2 0-6.5 3.5-6.5 3.5s4.5 3.5 6.5 3.5A3.5 3.5 0 0 0 22 12z" />
          </svg>
        </div>
        <span class="app-name">{t('Photos')}</span>
      </div>
    </div>

    <div class="sidebar-nav-scroll">
      <!-- Section: Library -->
      <div class="nav-group">
        <div class="group-header">{t('PHOTOS')}</div>
        <button
          type="button"
          class="nav-row"
          class:active={!album && !photoMonth && mediaFilter === 'all'}
          on:click={() => { album = null; photoMonth = ''; mediaFilter = 'all'; run(load); }}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="row-svg">
            <rect x="3" y="3" width="18" height="18" rx="3" />
            <path d="M3 9h18M9 21V9" />
          </svg>
          <span class="row-label">{t('Library')}</span>
          <span class="row-count">{libraryPhotos.length}</span>
        </button>

        <button
          type="button"
          class="nav-row"
          class:active={!album && !photoMonth && mediaFilter === 'videos'}
          on:click={() => { album = null; photoMonth = ''; mediaFilter = 'videos'; }}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="row-svg">
            <polygon points="23 7 16 12 23 17 23 7" />
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
          </svg>
          <span class="row-label">{t('Videos')}</span>
          <span class="row-count">{videoCount}</span>
        </button>
      </div>

      <!-- Section: Timeline Months -->
      {#if monthSummaries.length > 0}
        <div class="nav-group">
          <div class="group-header">{t('TIMELINE')}</div>
          {#each monthSummaries as m}
            {@const mKey = `${m.year}-${m.month}`}
            <button
              type="button"
              class="nav-row sub-row"
              class:active={!album && photoMonth === mKey}
              on:click={() => { album = null; photoMonth = mKey; mediaFilter = 'all'; run(load); }}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="row-svg">
                <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
                <line x1="16" y1="2" x2="16" y2="6" />
                <line x1="8" y1="2" x2="8" y2="6" />
                <line x1="3" y1="10" x2="21" y2="10" />
              </svg>
              <span class="row-label">{formatMonthLabel(m.year, m.month)}</span>
              <span class="row-count">{m.count}</span>
            </button>
          {/each}
        </div>
      {/if}

      <!-- Section: Albums -->
      {#if native}
        <div class="nav-group">
          <div class="group-header-with-action">
            <span>{t('MY ALBUMS')}</span>
            <button
              type="button"
              class="add-album-plus"
              title={t('New Album')}
              disabled={busy}
              on:click={() => showNewAlbum = !showNewAlbum}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14M5 12h14" />
              </svg>
            </button>
          </div>

          {#if showNewAlbum}
            <form class="inline-create-album" on:submit|preventDefault={createAlbum}>
              <input
                type="text"
                bind:value={albumName}
                placeholder={t('Album name...')}
                class="album-input-field"
                required
              />
              <div class="create-actions">
                <button type="submit" class="btn-xs primary" disabled={busy || !albumName.trim()}>{t('Create')}</button>
                <button type="button" class="btn-xs" on:click={() => { showNewAlbum = false; albumName = ''; }}>{t('Cancel')}</button>
              </div>
            </form>
          {/if}

          <div class="albums-list">
            {#each albums as a (a.id)}
              <div class="album-item-row" class:active={album?.id === a.id}>
                <button
                  type="button"
                  class="album-btn"
                  on:click={() => openAlbum(a)}
                >
                  <div class="album-cover-icon">
                    {#if a.cover_file_id}
                      <img src={media(a.cover_file_id, 'thumb')} alt="" class="album-cover-img" />
                    {:else}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                        <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
                      </svg>
                    {/if}
                  </div>
                  <span class="row-label" title={a.name}>{a.name}</span>
                  <span class="row-count">{a.item_count}</span>
                </button>
                <div class="album-quick-actions">
                  <button
                    type="button"
                    class="icon-action-btn"
                    title={t('Rename')}
                    disabled={busy}
                    on:click|stopPropagation={() => startRename(a)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="icon-action-btn danger"
                    title={t('Delete')}
                    disabled={busy}
                    on:click|stopPropagation={() => deleteAlbum(a)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="3 6 5 6 21 6" />
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                  </button>
                </div>
              </div>
            {:else}
              {#if !showNewAlbum}
                <div class="no-albums-note">
                  <span>{t('No custom albums')}</span>
                </div>
              {/if}
            {/each}

            {#if albumListCursor}
              <button type="button" class="more-albums-btn" disabled={busy} on:click={loadMoreAlbums}>
                {t('Load more')}
              </button>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <!-- Bottom: Calm Status Bar (No admin dashboard clutter) -->
    <div class="sidebar-status-bar">
      <div class="sync-status-indicator">
        <span class="sync-dot" class:online={connected}></span>
        <span class="sync-text">{connected ? t('Cloud Synced') : t('Local Only')}</span>
      </div>
      {#if backup}
        <button
          type="button"
          class="backup-info-btn"
          title={t('Sync & Backup Status')}
          on:click={() => showBackupPopover = !showBackupPopover}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="16" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12.01" y2="8" />
          </svg>
        </button>
      {/if}
    </div>

    <!-- Backup Popover Modal (Lightweight info on demand) -->
    {#if showBackupPopover && backup}
      <div class="backup-popover" role="dialog" aria-modal="false">
        <div class="popover-header">
          <span class="popover-title">{t('Sync & Verification')}</span>
          <button type="button" class="popover-close" on:click={() => showBackupPopover = false}>✕</button>
        </div>
        <div class="popover-body">
          <div class="popover-stat">
            <span class="stat-label">{t('Verified Photos')}</span>
            <span class="stat-val">{backup.verified_items ?? 0}</span>
          </div>
          {#if (backup.pending_items || 0) > 0}
            <div class="popover-stat">
              <span class="stat-label">{t('Pending Copies')}</span>
              <span class="stat-val warning">{backup.pending_items}</span>
            </div>
          {/if}
          {#if backup.last_backup_at}
            <div class="popover-stat">
              <span class="stat-label">{t('Last Synced')}</span>
              <span class="stat-val">{date(backup.last_backup_at)}</span>
            </div>
          {/if}
        </div>
        {#if go}
          <button type="button" class="popover-link" on:click={() => { showBackupPopover = false; go('Storage'); }}>
            {t('Manage Hub Storage')} →
          </button>
        {/if}
      </div>
    {/if}
  </aside>

  <!-- Main Photo Gallery Canvas -->
  <main class="photos-canvas">
    <!-- Clean Apple Photos Top Bar -->
    <header class="gallery-topbar">
      <!-- Left: Title & Subtitle -->
      <div class="topbar-left">
        <h1 class="gallery-title">
          {#if album}
            {album.name}
          {:else if photoMonth}
            {formatMonthLabel(...photoMonth.split('-').map(Number))}
          {:else}
            {t('Library')}
          {/if}
        </h1>
        <span class="gallery-subtitle">
          {filteredPhotos.length} {filteredPhotos.length === 1 ? t('item') : t('items')}
        </span>
      </div>

      <!-- Center: Segmented Media Filter (All | Photos | Videos) -->
      <div class="topbar-center">
        <div class="segmented-control" role="tablist">
          <button
            type="button"
            class="segment-btn"
            class:active={mediaFilter === 'all'}
            on:click={() => mediaFilter = 'all'}
          >
            {t('All')}
          </button>
          <button
            type="button"
            class="segment-btn"
            class:active={mediaFilter === 'photos'}
            on:click={() => mediaFilter = 'photos'}
          >
            {t('Photos')}
          </button>
          <button
            type="button"
            class="segment-btn"
            class:active={mediaFilter === 'videos'}
            on:click={() => mediaFilter = 'videos'}
          >
            {t('Videos')}
          </button>
        </div>
      </div>

      <!-- Right: Album Actions & Zoom Slider -->
      <div class="topbar-right">
        {#if album}
          <div class="album-header-actions">
            <button
              type="button"
              class="action-pill primary"
              disabled={busy}
              on:click={() => showAddPhotosModal = true}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
                <path d="M12 5v14M5 12h14" />
              </svg>
              <span>{t('Add Photos')}</span>
            </button>
            <button
              type="button"
              class="action-pill"
              disabled={busy}
              on:click={() => startRename(album)}
            >
              <span>{t('Rename')}</span>
            </button>
            <button
              type="button"
              class="action-pill danger"
              disabled={busy}
              on:click={() => deleteAlbum(album)}
            >
              <span>{t('Delete')}</span>
            </button>
          </div>
        {/if}

        <!-- Layout Mode Toggle (Moments vs Flat Grid) -->
        <div class="layout-toggle" role="toolbar" aria-label={t('Layout mode')}>
          <button
            type="button"
            class="layout-btn"
            class:active={layoutMode === 'moments'}
            title={t('Group by Date')}
            on:click={() => layoutMode = 'moments'}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="3" y1="6" x2="21" y2="6" />
              <line x1="3" y1="12" x2="21" y2="12" />
              <line x1="3" y1="18" x2="21" y2="18" />
            </svg>
          </button>
          <button
            type="button"
            class="layout-btn"
            class:active={layoutMode === 'grid'}
            title={t('Square Grid')}
            on:click={() => layoutMode = 'grid'}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" /><rect x="3" y="14" width="7" height="7" />
            </svg>
          </button>
        </div>

        <!-- Zoom Slider -->
        <div class="zoom-slider-wrap" title={t('Zoom size')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="zoom-svg small">
            <rect x="6" y="6" width="12" height="12" rx="2" />
          </svg>
          <input
            type="range"
            min="120"
            max="300"
            step="15"
            bind:value={gridSize}
            class="zoom-slider"
            aria-label={t('Zoom')}
          />
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="zoom-svg large">
            <rect x="3" y="3" width="18" height="18" rx="2" />
          </svg>
        </div>
      </div>
    </header>

    <!-- Inline Rename Bar -->
    {#if renaming && album}
      <form class="inline-rename-strip" on:submit|preventDefault={saveRename}>
        <span class="rename-prompt">{t('Rename Album')}:</span>
        <input
          type="text"
          bind:value={renameValue}
          maxlength="120"
          class="rename-input"
          required
        />
        <button type="submit" class="btn-xs primary" disabled={busy || !renameValue.trim()}>{t('Save')}</button>
        <button type="button" class="btn-xs" on:click={() => renaming = false}>{t('Cancel')}</button>
      </form>
    {/if}

    <!-- Error Alert Banner -->
    {#if albumError}
      <div class="error-strip" role="alert">
        <span>{albumError}</span>
        <button type="button" class="error-close" on:click={() => albumError = ''}>✕</button>
      </div>
    {/if}

    <!-- Gallery Scrollable Canvas -->
    <div class="gallery-scroll">
      {#if filteredPhotos.length > 0}
        <!-- MOMENTS VIEW (Grouped by Date) -->
        {#if layoutMode === 'moments'}
          <div class="moments-container">
            {#each groupedPhotos as group}
              <section class="moment-section">
                <div class="moment-header">
                  <div class="moment-title-group">
                    <h2 class="moment-title">{group.label}</h2>
                    {#if group.sublabel}
                      <span class="moment-sublabel">{group.sublabel}</span>
                    {/if}
                  </div>
                  <span class="moment-count">{group.items.length}</span>
                </div>

                <div class="moment-photo-grid">
                  {#each group.items as photo (photo.file_id || photo.id)}
                    <div class="photo-tile-card">
                      <!-- Hitbox: click opens full lightbox viewFile -->
                      <button
                        type="button"
                        class="tile-hitbox"
                        aria-label={photo.name || date(photo.taken_at)}
                        on:click={() => viewFile({
                          ...photo,
                          id: photo.file_id || photo.id,
                          mime: photo.mime || (photo.type_ === 'video' ? 'video/mp4' : 'image/jpeg'),
                          name: photo.name || date(photo.taken_at)
                        })}
                      >
                        <div class="tile-media">
                          <PhotoTile {photo} label="" {t}/>
                        </div>

                        <!-- Video indicator badge -->
                        {#if photo.type_ === 'video' || photo.mime?.startsWith('video/')}
                          <div class="video-pill">
                            <svg viewBox="0 0 24 24" fill="currentColor" class="play-icon">
                              <path d="M8 5v14l11-7z" />
                            </svg>
                            <span>VIDEO</span>
                          </div>
                        {/if}

                        <!-- Hover vignette overlay -->
                        <div class="tile-vignette">
                          <span class="vignette-date">{formatShortDate(photo.taken_at)}{photo.taken_at ? ` · ${formatTime(photo.taken_at)}` : ''}</span>
                        </div>
                      </button>

                      <!-- Top-right contextual action -->
                      {#if album}
                        <button
                          type="button"
                          class="tile-action-btn remove"
                          title={t('Remove from album')}
                          disabled={busy}
                          on:click|stopPropagation={() => removePhotoFromAlbum(album.id, photo.file_id || photo.id)}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
                            <path d="M18 6L6 18M6 6l12 12" />
                          </svg>
                        </button>
                      {:else if native && albums.length > 0}
                        <button
                          type="button"
                          class="tile-action-btn add"
                          title={t('Add to album')}
                          disabled={busy}
                          on:click|stopPropagation={() => activeMenuPhotoId = (activeMenuPhotoId === (photo.file_id || photo.id) ? null : (photo.file_id || photo.id))}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
                            <path d="M12 5v14M5 12h14" />
                          </svg>
                        </button>
                      {/if}

                      <!-- Quick Add-to-Album Floating Menu -->
                      {#if activeMenuPhotoId === (photo.file_id || photo.id)}
                        <div class="album-menu-popover" role="menu">
                          <div class="menu-popover-header">
                            <span>{t('Add to Album')}</span>
                            <button type="button" class="menu-popover-close" on:click={() => activeMenuPhotoId = null}>✕</button>
                          </div>
                          <div class="menu-popover-list">
                            {#each albums as a}
                              <button
                                type="button"
                                class="menu-popover-item"
                                role="menuitem"
                                disabled={busy}
                                on:click={() => {
                                  addPhotoToAlbum(a.id, photo.file_id || photo.id);
                                  activeMenuPhotoId = null;
                                }}
                              >
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="menu-folder-svg">
                                  <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
                                </svg>
                                <span class="menu-album-title">{a.name}</span>
                                <span class="menu-album-count">{a.item_count}</span>
                              </button>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              </section>
            {/each}
          </div>
        {:else}
          <!-- FLAT SQUARE GRID VIEW -->
          <div class="flat-photo-grid">
            {#each filteredPhotos as photo (photo.file_id || photo.id)}
              <div class="photo-tile-card">
                <button
                  type="button"
                  class="tile-hitbox"
                  aria-label={photo.name || date(photo.taken_at)}
                  on:click={() => viewFile({
                    ...photo,
                    id: photo.file_id || photo.id,
                    mime: photo.mime || (photo.type_ === 'video' ? 'video/mp4' : 'image/jpeg'),
                    name: photo.name || date(photo.taken_at)
                  })}
                >
                  <div class="tile-media">
                    <PhotoTile {photo} label="" {t}/>
                  </div>

                  {#if photo.type_ === 'video' || photo.mime?.startsWith('video/')}
                    <div class="video-pill">
                      <svg viewBox="0 0 24 24" fill="currentColor" class="play-icon">
                        <path d="M8 5v14l11-7z" />
                      </svg>
                      <span>VIDEO</span>
                    </div>
                  {/if}

                  <div class="tile-vignette">
                    <span class="vignette-date">{formatShortDate(photo.taken_at)}{photo.taken_at ? ` · ${formatTime(photo.taken_at)}` : ''}</span>
                  </div>
                </button>

                {#if album}
                  <button
                    type="button"
                    class="tile-action-btn remove"
                    title={t('Remove from album')}
                    disabled={busy}
                    on:click|stopPropagation={() => removePhotoFromAlbum(album.id, photo.file_id || photo.id)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
                      <path d="M18 6L6 18M6 6l12 12" />
                    </svg>
                  </button>
                {:else if native && albums.length > 0}
                  <button
                    type="button"
                    class="tile-action-btn add"
                    title={t('Add to album')}
                    disabled={busy}
                    on:click|stopPropagation={() => activeMenuPhotoId = (activeMenuPhotoId === (photo.file_id || photo.id) ? null : (photo.file_id || photo.id))}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
                      <path d="M12 5v14M5 12h14" />
                    </svg>
                  </button>
                {/if}

                {#if activeMenuPhotoId === (photo.file_id || photo.id)}
                  <div class="album-menu-popover" role="menu">
                    <div class="menu-popover-header">
                      <span>{t('Add to Album')}</span>
                      <button type="button" class="menu-popover-close" on:click={() => activeMenuPhotoId = null}>✕</button>
                    </div>
                    <div class="menu-popover-list">
                      {#each albums as a}
                        <button
                          type="button"
                          class="menu-popover-item"
                          role="menuitem"
                          disabled={busy}
                          on:click={() => {
                            addPhotoToAlbum(a.id, photo.file_id || photo.id);
                            activeMenuPhotoId = null;
                          }}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="menu-folder-svg">
                            <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
                          </svg>
                          <span class="menu-album-title">{a.name}</span>
                          <span class="menu-album-count">{a.item_count}</span>
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        <!-- Pagination Load More -->
        {#if !album && cursor}
          <div class="pagination-bar">
            <button type="button" class="load-more-pill" disabled={busy} on:click={() => run(more)}>
              <span>{t('Load older photos')}</span>
            </button>
          </div>
        {:else if album && albumCursor}
          <div class="pagination-bar">
            <button type="button" class="load-more-pill" disabled={busy} on:click={loadMoreAlbum}>
              <span>{t('Load more in album')}</span>
            </button>
          </div>
        {/if}

        <!-- Signature macOS Photos Footer -->
        <footer class="gallery-signature-footer">
          <div class="signature-count">
            {photoCount} {photoCount === 1 ? t('Photo') : t('Photos')}, {videoCount} {videoCount === 1 ? t('Video') : t('Videos')}
          </div>
          <div class="signature-status">
            <span class="hub-status-dot" class:online={connected}></span>
            <span>{connected ? t('Synced with Home Hub • All originals verified') : t('Local library active')}</span>
          </div>
        </footer>
      {:else}
        <!-- Empty State Canvas -->
        <div class="empty-gallery-state">
          <div class="empty-halo">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="3" y="3" width="18" height="18" rx="4" />
              <circle cx="8.5" cy="8.5" r="1.5" />
              <path d="M21 15l-5-5L5 21" />
            </svg>
          </div>
          <h2 class="empty-heading">
            {#if album}
              {t('Album is Empty')}
            {:else if mediaFilter === 'videos'}
              {t('No Videos Found')}
            {:else if photoMonth}
              {t('No Photos in This Timeline')}
            {:else}
              {t('No Photos in Library')}
            {/if}
          </h2>
          <p class="empty-detail">
            {#if album}
              {t('Add photos from your library to keep your memories organized in this album.')}
            {:else if mediaFilter === 'videos'}
              {t('Videos synced from your phone or imported to Home Hub will appear here.')}
            {:else}
              {t('Photos and videos synced from your connected phones or imported folders will appear here automatically.')}
            {/if}
          </p>
          {#if album}
            <button type="button" class="empty-action-pill" disabled={busy} on:click={() => showAddPhotosModal = true}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14M5 12h14" />
              </svg>
              <span>{t('Add Photos to Album')}</span>
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </main>

  <!-- Modal: Add Photos to Album -->
  {#if showAddPhotosModal && album}
    <div class="modal-backdrop">
      <button
        type="button"
        class="backdrop-dismiss"
        aria-label={t('Close')}
        on:click={() => showAddPhotosModal = false}
      ></button>
      <div
        class="add-photos-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-add-title"
      >
        <div class="dialog-header">
          <div>
            <h3 id="modal-add-title" class="dialog-title">{t('Add Photos to')} {album.name}</h3>
            <p class="dialog-subtitle">{t('Click photos from your library to toggle them in this album.')}</p>
          </div>
          <button type="button" class="dialog-close-btn" on:click={() => showAddPhotosModal = false}>✕</button>
        </div>

        <div class="dialog-scroll">
          <div class="dialog-photo-grid">
            {#each libraryPhotos as p (p.file_id || p.id)}
              {@const inAlbum = (albumItems || []).some(item => item.file_id === p.file_id)}
              <button
                type="button"
                class="dialog-photo-card"
                class:selected={inAlbum}
                disabled={busy}
                aria-label={p.name || date(p.taken_at)}
                on:click={() => {
                  if (inAlbum) {
                    removePhotoFromAlbum(album.id, p.file_id);
                  } else {
                    addPhotoToAlbum(album.id, p.file_id);
                  }
                }}
              >
                <div class="dialog-media">
                  <PhotoTile photo={p} label="" {t}/>
                </div>
                <div class="dialog-check-overlay" class:checked={inAlbum}>
                  <div class="dialog-check-circle" class:checked={inAlbum}>
                    {#if inAlbum}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    {:else}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                        <path d="M12 5v14M5 12h14" />
                      </svg>
                    {/if}
                  </div>
                </div>
              </button>
            {:else}
              <p class="dialog-empty-msg">{t('No library photos available to add.')}</p>
            {/each}
          </div>
        </div>

        <div class="dialog-footer">
          <span class="dialog-count-info">
            {(albumItems || []).length} {t('photos currently in album')}
          </span>
          <button type="button" class="dialog-done-btn" on:click={() => showAddPhotosModal = false}>
            {t('Done')}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .photos-app {
    display: flex;
    flex-direction: row;
    height: 100%;
    width: 100%;
    background: #090B0E;
    color: #e2e8f0;
    font-family: inherit;
    overflow: hidden;
    position: relative;
    user-select: none;
    box-sizing: border-box;
  }

  /* ---------------- macOS Photos Minimalist Sidebar (210px) ---------------- */
  .photos-sidebar {
    width: 210px;
    min-width: 210px;
    max-width: 210px;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: rgba(14, 17, 23, 0.88);
    backdrop-filter: blur(24px);
    border-right: 1px solid rgba(255, 255, 255, 0.07);
    overflow: hidden;
    z-index: 5;
    position: relative;
    box-sizing: border-box;
  }

  .sidebar-top {
    padding: 16px 14px 12px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
  }

  .app-brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .brand-flower-icon {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: linear-gradient(135deg, #10b981, #059669);
    display: grid;
    place-items: center;
    color: #fff;
    box-shadow: 0 2px 8px rgba(16, 185, 129, 0.3);
  }

  .brand-flower-icon svg {
    width: 16px;
    height: 16px;
  }

  .app-name {
    font-size: 13.5px;
    font-weight: 650;
    color: #f8fafc;
    letter-spacing: -0.01em;
  }

  .sidebar-nav-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 10px 8px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .group-header {
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #526071;
    padding: 4px 8px 3px;
    text-transform: uppercase;
  }

  .group-header-with-action {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #526071;
    padding: 4px 6px 3px 8px;
    text-transform: uppercase;
  }

  .add-album-plus {
    background: transparent;
    border: 0;
    color: #818f9f;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    display: grid;
    place-items: center;
    transition: color 0.15s ease, background 0.15s ease;
  }

  .add-album-plus:hover {
    color: #10b981;
    background: rgba(16, 185, 129, 0.12);
  }

  .add-album-plus svg {
    width: 13px;
    height: 13px;
  }

  .nav-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: transparent;
    border: 0;
    border-radius: 6px;
    color: #94a3b8;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: all 0.15s ease;
    box-sizing: border-box;
  }

  .nav-row:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #f1f5f9;
  }

  .nav-row.active {
    background: rgba(16, 185, 129, 0.14);
    color: #10b981;
    font-weight: 600;
  }

  .nav-row.sub-row {
    padding-left: 12px;
    font-size: 11.5px;
  }

  .row-svg {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    color: inherit;
    opacity: 0.85;
  }

  .row-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-count {
    font-size: 10.5px;
    font-weight: 550;
    color: #526071;
    padding: 1px 5px;
    border-radius: 8px;
  }

  .nav-row.active .row-count {
    color: #10b981;
  }

  /* Album row items */
  .albums-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .album-item-row {
    display: flex;
    align-items: center;
    width: 100%;
    border-radius: 6px;
    position: relative;
    transition: background 0.15s ease;
  }

  .album-item-row:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .album-item-row.active {
    background: rgba(16, 185, 129, 0.14);
  }

  .album-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    background: transparent;
    border: 0;
    color: #94a3b8;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
  }

  .album-item-row:hover .album-btn {
    color: #f1f5f9;
  }

  .album-item-row.active .album-btn {
    color: #10b981;
    font-weight: 600;
  }

  .album-cover-icon {
    width: 17px;
    height: 17px;
    border-radius: 4px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.06);
  }

  .album-cover-icon svg {
    width: 12px;
    height: 12px;
    color: #10b981;
  }

  .album-cover-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .album-quick-actions {
    display: none;
    align-items: center;
    gap: 2px;
    padding-right: 4px;
  }

  .album-item-row:hover .album-quick-actions {
    display: flex;
  }

  .icon-action-btn {
    background: transparent;
    border: 0;
    color: #64748b;
    cursor: pointer;
    padding: 3px;
    border-radius: 4px;
    display: grid;
    place-items: center;
  }

  .icon-action-btn svg {
    width: 12px;
    height: 12px;
  }

  .icon-action-btn:hover {
    color: #f8fafc;
    background: rgba(255, 255, 255, 0.1);
  }

  .icon-action-btn.danger:hover {
    color: #f43f5e;
    background: rgba(244, 63, 94, 0.15);
  }

  .inline-create-album {
    padding: 6px 8px;
    margin: 3px 0 5px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .album-input-field {
    width: 100%;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 5px;
    color: #fff;
    font-size: 11.5px;
    padding: 4px 7px;
    outline: none;
    box-sizing: border-box;
  }

  .album-input-field:focus {
    border-color: #10b981;
  }

  .create-actions {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }

  .btn-xs {
    padding: 3px 7px;
    font-size: 10.5px;
    font-weight: 550;
    border-radius: 4px;
    border: 0;
    cursor: pointer;
    background: transparent;
    color: #94a3b8;
  }

  .btn-xs.primary {
    background: #10b981;
    color: #032516;
  }

  .btn-xs.primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .no-albums-note {
    font-size: 10.5px;
    color: #526071;
    padding: 6px 8px;
  }

  .more-albums-btn {
    background: transparent;
    border: 0;
    color: #94a3b8;
    font-size: 10.5px;
    padding: 4px 8px;
    cursor: pointer;
    text-align: left;
  }

  .more-albums-btn:hover {
    color: #10b981;
  }

  /* Bottom status bar in sidebar */
  .sidebar-status-bar {
    padding: 10px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    color: #64748b;
    margin-top: auto;
  }

  .sync-status-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .sync-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #526071;
  }

  .sync-dot.online {
    background: #10b981;
    box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
  }

  .sync-text {
    font-size: 10.5px;
    font-weight: 500;
  }

  .backup-info-btn {
    background: transparent;
    border: 0;
    color: #64748b;
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    display: grid;
    place-items: center;
    transition: color 0.15s ease;
  }

  .backup-info-btn:hover {
    color: #10b981;
  }

  .backup-info-btn svg {
    width: 14px;
    height: 14px;
  }

  /* Backup info popover */
  .backup-popover {
    position: absolute;
    bottom: 42px;
    left: 8px;
    right: 8px;
    background: #141a23;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    padding: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .popover-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    font-weight: 600;
    color: #f1f5f9;
  }

  .popover-close {
    background: transparent;
    border: 0;
    color: #64748b;
    font-size: 11px;
    cursor: pointer;
    padding: 2px;
  }

  .popover-close:hover {
    color: #fff;
  }

  .popover-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .popover-stat {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: #94a3b8;
  }

  .stat-val {
    font-weight: 600;
    color: #f8fafc;
  }

  .stat-val.warning {
    color: #fbbf24;
  }

  .popover-link {
    background: transparent;
    border: 0;
    color: #10b981;
    font-size: 10px;
    cursor: pointer;
    padding: 2px 0 0;
    text-align: left;
  }

  .popover-link:hover {
    text-decoration: underline;
  }

  /* ---------------- Main Gallery Canvas ---------------- */
  .photos-canvas {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #090B0E;
    overflow: hidden;
    position: relative;
    box-sizing: border-box;
  }

  /* Top Bar */
  .gallery-topbar {
    padding: 10px 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(9, 11, 14, 0.75);
    backdrop-filter: blur(16px);
    flex-shrink: 0;
    z-index: 4;
  }

  .topbar-left {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .gallery-title {
    font-size: 17px;
    font-weight: 700;
    color: #f8fafc;
    margin: 0;
    letter-spacing: -0.015em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .gallery-subtitle {
    font-size: 11.5px;
    color: #64748b;
    font-weight: 500;
    white-space: nowrap;
  }

  /* Segmented control: All | Photos | Videos */
  .topbar-center {
    display: flex;
    justify-content: center;
  }

  .segmented-control {
    display: flex;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 7px;
    padding: 2px;
    gap: 2px;
  }

  .segment-btn {
    background: transparent;
    border: 0;
    color: #818f9f;
    font-size: 11px;
    font-weight: 550;
    padding: 4px 10px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .segment-btn:hover {
    color: #f8fafc;
  }

  .segment-btn.active {
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  }

  .topbar-right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }

  .album-header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .action-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 550;
    cursor: pointer;
    border: 0;
    background: rgba(255, 255, 255, 0.07);
    color: #e2e8f0;
    transition: all 0.15s ease;
  }

  .action-pill svg {
    width: 12px;
    height: 12px;
  }

  .action-pill:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .action-pill.primary {
    background: #10b981;
    color: #032516;
    font-weight: 600;
  }

  .action-pill.primary:hover {
    background: #34d399;
  }

  .action-pill.danger {
    color: #f43f5e;
    background: rgba(244, 63, 94, 0.12);
  }

  .action-pill.danger:hover {
    background: rgba(244, 63, 94, 0.22);
  }

  /* Layout toggle buttons */
  .layout-toggle {
    display: flex;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 6px;
    padding: 2px;
    gap: 2px;
  }

  .layout-btn {
    background: transparent;
    border: 0;
    color: #64748b;
    padding: 4px 6px;
    border-radius: 4px;
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: all 0.15s ease;
  }

  .layout-btn svg {
    width: 13px;
    height: 13px;
  }

  .layout-btn:hover {
    color: #f1f5f9;
  }

  .layout-btn.active {
    background: rgba(255, 255, 255, 0.12);
    color: #10b981;
  }

  /* Zoom Slider */
  .zoom-slider-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 6px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
  }

  .zoom-svg {
    width: 13px;
    height: 13px;
    color: #64748b;
  }

  .zoom-svg.small {
    width: 11px;
    height: 11px;
  }

  .zoom-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 65px;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.16);
    outline: none;
    cursor: pointer;
  }

  .zoom-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #e2e8f0;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
    transition: transform 0.1s ease;
  }

  .zoom-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
    background: #10b981;
  }

  /* Inline rename strip */
  .inline-rename-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 20px;
    background: rgba(244, 63, 94, 0.08);
    border-bottom: 1px solid rgba(244, 63, 94, 0.2);
    flex-shrink: 0;
  }

  .rename-prompt {
    font-size: 11.5px;
    font-weight: 600;
    color: #f43f5e;
  }

  .rename-input {
    flex: 1;
    max-width: 260px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    color: #fff;
    font-size: 11.5px;
    padding: 3px 8px;
    outline: none;
  }

  /* Error alert strip */
  .error-strip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 20px;
    background: rgba(239, 68, 68, 0.15);
    border-bottom: 1px solid rgba(239, 68, 68, 0.3);
    color: #fca5a5;
    font-size: 11.5px;
    flex-shrink: 0;
  }

  .error-close {
    background: transparent;
    border: 0;
    color: #fca5a5;
    cursor: pointer;
    font-size: 12px;
  }

  /* Gallery Scroll Area */
  .gallery-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 16px 20px 32px;
    display: flex;
    flex-direction: column;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  /* ---------------- Apple Photos "Moments" Layout ---------------- */
  .moments-container {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .moment-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .moment-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding-bottom: 4px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .moment-title-group {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .moment-title {
    font-size: 13.5px;
    font-weight: 650;
    color: #f1f5f9;
    margin: 0;
    letter-spacing: -0.01em;
  }

  .moment-sublabel {
    font-size: 11px;
    color: #64748b;
  }

  .moment-count {
    font-size: 11px;
    color: #526071;
  }

  .moment-photo-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--grid-size, 190px), 1fr));
    gap: 8px;
  }

  /* Flat square grid layout */
  .flat-photo-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--grid-size, 190px), 1fr));
    gap: 8px;
  }

  /* ---------------- Photo Tile Card ---------------- */
  .photo-tile-card {
    aspect-ratio: 1 / 1;
    border-radius: 6px;
    overflow: hidden;
    position: relative;
    background: #12161e;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 0.2s cubic-bezier(0.2, 0, 0, 1), box-shadow 0.2s ease;
    box-sizing: border-box;
  }

  .photo-tile-card:hover {
    transform: scale(1.025);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
    z-index: 10;
  }

  .tile-hitbox {
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    position: absolute;
    inset: 0;
    display: block;
    text-align: left;
  }

  .tile-media {
    width: 100%;
    height: 100%;
    position: absolute;
    inset: 0;
  }

  .tile-media :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: filter 0.2s ease;
  }

  .photo-tile-card:hover .tile-media :global(img) {
    filter: brightness(1.04);
  }

  .tile-media :global(> span:last-child) {
    display: none;
  }

  .tile-media :global(.photo-placeholder) {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px;
    background: rgba(255, 255, 255, 0.03);
    color: #526071;
    font-size: 10px;
    text-align: center;
  }

  /* Video Badge */
  .video-pill {
    position: absolute;
    bottom: 6px;
    left: 6px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(6px);
    border-radius: 4px;
    padding: 2px 5px;
    font-size: 9px;
    font-weight: 700;
    color: #fff;
    letter-spacing: 0.04em;
    z-index: 2;
  }

  .play-icon {
    width: 8px;
    height: 8px;
  }

  /* Vignette Overlay */
  .tile-vignette {
    position: absolute;
    inset: auto 0 0 0;
    padding: 20px 8px 6px;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.8) 0%, transparent 100%);
    display: flex;
    flex-direction: column;
    opacity: 0;
    transition: opacity 0.18s ease;
    pointer-events: none;
    z-index: 3;
  }

  .photo-tile-card:hover .tile-vignette,
  .photo-tile-card:focus-within .tile-vignette {
    opacity: 1;
  }

  .vignette-date {
    font-size: 10px;
    font-weight: 500;
    color: #f1f5f9;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Top Right Action Button */
  .tile-action-btn {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.2);
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0;
    transform: scale(0.85);
    transition: opacity 0.18s ease, transform 0.18s ease, background 0.15s ease;
    z-index: 5;
  }

  .photo-tile-card:hover .tile-action-btn,
  .photo-tile-card:focus-within .tile-action-btn {
    opacity: 1;
    transform: scale(1);
  }

  .tile-action-btn.add {
    background: rgba(16, 185, 129, 0.85);
    color: #022516;
  }

  .tile-action-btn.add:hover {
    background: #10b981;
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.5);
  }

  .tile-action-btn.remove {
    background: rgba(244, 63, 94, 0.85);
    color: #fff;
  }

  .tile-action-btn.remove:hover {
    background: #f43f5e;
  }

  .tile-action-btn svg {
    width: 12px;
    height: 12px;
  }

  /* Album popover menu */
  .album-menu-popover {
    position: absolute;
    top: 34px;
    right: 6px;
    width: 170px;
    max-height: 180px;
    background: rgba(18, 23, 31, 0.96);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.7);
    display: flex;
    flex-direction: column;
    z-index: 25;
    overflow: hidden;
  }

  .menu-popover-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 8px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 10px;
    font-weight: 700;
    color: #818f9f;
  }

  .menu-popover-close {
    background: transparent;
    border: 0;
    color: #64748b;
    font-size: 10px;
    cursor: pointer;
  }

  .menu-popover-close:hover {
    color: #fff;
  }

  .menu-popover-list {
    overflow-y: auto;
    padding: 3px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .menu-popover-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 7px;
    background: transparent;
    border: 0;
    border-radius: 5px;
    color: #e2e8f0;
    font-size: 11px;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s ease;
  }

  .menu-popover-item:hover {
    background: rgba(16, 185, 129, 0.18);
    color: #10b981;
  }

  .menu-folder-svg {
    width: 12px;
    height: 12px;
    color: #10b981;
    flex-shrink: 0;
  }

  .menu-album-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu-album-count {
    font-size: 9.5px;
    color: #64748b;
  }

  /* Pagination Bar */
  .pagination-bar {
    display: flex;
    justify-content: center;
    padding: 20px 0 10px;
  }

  .load-more-pill {
    padding: 7px 18px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    color: #e2e8f0;
    font-size: 11.5px;
    font-weight: 550;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .load-more-pill:hover {
    background: rgba(16, 185, 129, 0.15);
    border-color: rgba(16, 185, 129, 0.35);
    color: #10b981;
  }

  /* Signature Apple Photos Footer */
  .gallery-signature-footer {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 32px 0 16px;
    gap: 6px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    margin-top: 24px;
  }

  .signature-count {
    font-size: 12.5px;
    font-weight: 600;
    color: #94a3b8;
    letter-spacing: -0.01em;
  }

  .signature-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #526071;
  }

  .hub-status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #526071;
  }

  .hub-status-dot.online {
    background: #10b981;
  }

  /* Empty State Canvas */
  .empty-gallery-state {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 380px;
    padding: 60px 20px;
  }

  .empty-halo {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: rgba(16, 185, 129, 0.08);
    display: grid;
    place-items: center;
    color: #10b981;
    margin-bottom: 16px;
  }

  .empty-halo svg {
    width: 28px;
    height: 28px;
  }

  .empty-heading {
    font-size: 16px;
    font-weight: 650;
    color: #f8fafc;
    margin: 0 0 6px;
  }

  .empty-detail {
    font-size: 12px;
    line-height: 1.5;
    color: #64748b;
    margin: 0 0 18px;
  }

  .empty-action-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: #10b981;
    color: #032516;
    border: 0;
    padding: 7px 16px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .empty-action-pill:hover {
    background: #34d399;
  }

  .empty-action-pill svg {
    width: 13px;
    height: 13px;
  }

  /* ---------------- Modal: Add Photos to Album ---------------- */
  .modal-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(10px);
    display: grid;
    place-items: center;
    z-index: 100;
    padding: 24px;
  }

  .backdrop-dismiss {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    background: transparent;
    border: 0;
    padding: 0;
    margin: 0;
    cursor: default;
    z-index: 0;
  }

  .add-photos-dialog {
    width: 100%;
    max-width: 640px;
    max-height: 78vh;
    background: #11151d;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
    z-index: 1;
  }

  .dialog-header {
    padding: 14px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
  }

  .dialog-title {
    font-size: 15px;
    font-weight: 700;
    color: #f8fafc;
    margin: 0 0 3px;
  }

  .dialog-subtitle {
    font-size: 11px;
    color: #818f9f;
    margin: 0;
  }

  .dialog-close-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 0;
    color: #cbd5e1;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    cursor: pointer;
    display: grid;
    place-items: center;
    font-size: 12px;
  }

  .dialog-close-btn:hover {
    background: rgba(255, 255, 255, 0.16);
    color: #fff;
  }

  .dialog-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.12) transparent;
  }

  .dialog-photo-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(105px, 1fr));
    gap: 8px;
  }

  .dialog-photo-card {
    aspect-ratio: 1 / 1;
    border-radius: 6px;
    overflow: hidden;
    position: relative;
    background: #18202c;
    border: 2px solid transparent;
    cursor: pointer;
    transition: transform 0.12s ease, border-color 0.12s ease;
    padding: 0;
  }

  .dialog-photo-card:hover {
    transform: scale(1.02);
  }

  .dialog-photo-card.selected {
    border-color: #10b981;
  }

  .dialog-media {
    width: 100%;
    height: 100%;
  }

  .dialog-media :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .dialog-media :global(> span:last-child) {
    display: none;
  }

  .dialog-media :global(.photo-placeholder) {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    color: #526071;
  }

  .dialog-check-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    justify-content: flex-end;
    align-items: flex-start;
    padding: 5px;
    background: transparent;
    transition: background 0.15s ease;
  }

  .dialog-check-overlay.checked {
    background: rgba(16, 185, 129, 0.25);
  }

  .dialog-check-circle {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    border: 1.5px solid rgba(255, 255, 255, 0.4);
    display: grid;
    place-items: center;
    color: #fff;
    transition: all 0.15s ease;
  }

  .dialog-check-circle.checked {
    background: #10b981;
    border-color: #10b981;
    color: #022516;
  }

  .dialog-check-circle svg {
    width: 11px;
    height: 11px;
  }

  .dialog-empty-msg {
    font-size: 11.5px;
    color: #64748b;
    text-align: center;
    padding: 24px;
  }

  .dialog-footer {
    padding: 12px 18px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(9, 11, 14, 0.7);
  }

  .dialog-count-info {
    font-size: 11.5px;
    color: #818f9f;
  }

  .dialog-done-btn {
    background: #10b981;
    color: #032516;
    border: 0;
    padding: 6px 16px;
    border-radius: 5px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .dialog-done-btn:hover {
    background: #34d399;
  }
</style>
