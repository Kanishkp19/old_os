<script>
  import { api, saveFile, bytes, media } from '../api.js';

  export let data;
  export let query = '';
  export let sort = 'newest';
  export let category = '';
  export let cursor = null;
  export let t = k => k;
  export let run;
  export let load;
  export let more;
  export let mutate;
  export let confirm;
  export let viewFile;
  export let fileForm;

  // Local UI state
  let viewMode = 'grid'; // 'grid' | 'list'
  let filterNeedsBackup = false;
  let selectedFile = null;
  let failedThumbs = {};

  $: files = Array.isArray(data?.files) ? data.files : [];

  $: categoryCounts = {
    all: files.length,
    photo: files.filter(f => f.category === 'photo').length,
    video: files.filter(f => f.category === 'video').length,
    document: files.filter(f => f.category === 'document').length,
    music: files.filter(f => f.category === 'music').length,
    backup: files.filter(f => f.category === 'backup').length,
    download: files.filter(f => f.category === 'download').length,
  };

  $: needsBackupCount = files.filter(f => f.second_copy_status !== 'verified').length;

  $: displayedFiles = filterNeedsBackup
    ? files.filter(f => f.second_copy_status !== 'verified')
    : files;

  $: totalDisplayedBytes = displayedFiles.reduce((acc, f) => acc + (Number(f.size) || 0), 0);
  $: totalLibraryBytes = files.reduce((acc, f) => acc + (Number(f.size) || 0), 0);

  $: if (selectedFile && !files.some(f => f.id === selectedFile.id)) {
    selectedFile = null;
  }

  const categories = [
    { id: 'photo', label: 'Photos & Images', key: 'Photos' },
    { id: 'video', label: 'Videos', key: 'Videos' },
    { id: 'document', label: 'Documents', key: 'Documents' },
    { id: 'music', label: 'Music & Audio', key: 'Music' },
    { id: 'backup', label: 'Backups & Archives', key: 'Backups' },
    { id: 'download', label: 'Downloads', key: 'Downloads' },
  ];

  function getCategoryLabel(cat) {
    if (!cat) return t('All Files');
    const found = categories.find(c => c.id === cat);
    return found ? t(found.key) : cat;
  }

  function selectAll() {
    filterNeedsBackup = false;
    category = '';
    run(load);
  }

  function selectRecent() {
    filterNeedsBackup = false;
    sort = 'newest';
    run(load);
  }

  function toggleNeedsBackup() {
    filterNeedsBackup = !filterNeedsBackup;
    if (filterNeedsBackup) {
      selectedFile = null;
    }
  }

  function selectCategory(catId) {
    filterNeedsBackup = false;
    category = catId;
    run(load);
  }

  function onSortChange(e) {
    sort = e.target.value;
    run(load);
  }

  function handleSearchSubmit() {
    run(load);
  }

  function clearSearch() {
    query = '';
    run(load);
  }

  function getThumbSrc(fileId) {
    return media(fileId, 'thumb') || `/api/files/${fileId}/thumb`;
  }

  function formatDate(val) {
    if (!val) return '—';
    try {
      const d = new Date(val);
      if (isNaN(d.getTime())) return '—';
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
    } catch {
      return '—';
    }
  }

  function getSecondCopyLabel(status) {
    if (status === 'verified') return t('Verified');
    if (status === 'disconnected') return t('Drive disconnected');
    return t('Needs copy');
  }

  function getFileIconType(file) {
    if (file.category) return file.category;
    if (file.mime?.startsWith('image/')) return 'photo';
    if (file.mime?.startsWith('video/')) return 'video';
    if (file.mime?.startsWith('audio/')) return 'music';
    if (file.mime?.includes('pdf') || file.mime?.includes('text')) return 'document';
    if (file.mime?.includes('zip') || file.mime?.includes('tar') || file.mime?.includes('archive')) return 'backup';
    return 'document';
  }
</script>

<div class="files-app">
  <!-- Left Sidebar (230px, Frosted Glass) -->
  <aside class="files-sidebar">
    <div class="sidebar-header">
      <div class="sidebar-brand">
        <div class="folder-badge">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="brand-svg">
            <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
          </svg>
        </div>
        <div class="brand-text">
          <span class="brand-title">{t('Files')}</span>
          <span class="brand-subtitle">{t('File Organiser')}</span>
        </div>
      </div>
      <span class="total-badge" title={t('Total items')}>{files.length}</span>
    </div>

    <div class="sidebar-scroll">
      <!-- DIVISION 1: QUICK ACCESS -->
      <div class="nav-section">
        <div class="nav-section-title">{t('QUICK ACCESS')}</div>
        <nav class="nav-list">
          <button
            type="button"
            class="nav-item"
            class:active={!filterNeedsBackup && category === ''}
            on:click={selectAll}
          >
            <span class="nav-icon blue">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <path d="M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z"/>
              </svg>
            </span>
            <span class="nav-label">{t('All Files')}</span>
            <span class="count-pill">{categoryCounts.all}</span>
          </button>

          <button
            type="button"
            class="nav-item"
            class:active={!filterNeedsBackup && sort === 'newest' && category === ''}
            on:click={selectRecent}
          >
            <span class="nav-icon amber">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <circle cx="12" cy="12" r="10"/>
                <polyline points="12 6 12 12 16 14"/>
              </svg>
            </span>
            <span class="nav-label">{t('Recent Files')}</span>
          </button>

          <button
            type="button"
            class="nav-item"
            class:active={filterNeedsBackup}
            on:click={toggleNeedsBackup}
          >
            <span class="nav-icon rose">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                <line x1="12" y1="8" x2="12" y2="12"/>
                <line x1="12" y1="16" x2="12.01" y2="16"/>
              </svg>
            </span>
            <span class="nav-label">{t('Needs Backup')}</span>
            {#if needsBackupCount > 0}
              <span class="count-pill alert">{needsBackupCount}</span>
            {/if}
          </button>
        </nav>
      </div>

      <!-- DIVISION 2: FILE CATEGORIES (MEDIA DIVISIONS) -->
      <div class="nav-section">
        <div class="nav-section-title">{t('FILE CATEGORIES')}</div>
        <nav class="nav-list">
          {#each categories as cat}
            <button
              type="button"
              class="nav-item"
              class:active={!filterNeedsBackup && category === cat.id}
              on:click={() => selectCategory(cat.id)}
            >
              <span class="nav-icon cat-{cat.id}">
                {#if cat.id === 'photo'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
                {:else if cat.id === 'video'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="4" width="20" height="16" rx="2.18"/><polygon points="10 8 16 12 10 16 10 8"/></svg>
                {:else if cat.id === 'document'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
                {:else if cat.id === 'music'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                {:else if cat.id === 'backup'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
                {:else if cat.id === 'download'}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                {/if}
              </span>
              <span class="nav-label">{t(cat.key)}</span>
              <span class="count-pill">{categoryCounts[cat.id] || 0}</span>
            </button>
          {/each}
        </nav>
      </div>

      <!-- DIVISION 3: STORAGE VOLUME -->
      <div class="nav-section storage-division">
        <div class="storage-card">
          <div class="storage-icon-wrap">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <ellipse cx="12" cy="5" rx="9" ry="3"/>
              <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
              <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
            </svg>
          </div>
          <div class="storage-meta">
            <span class="storage-title">{t('Home Hub Storage')}</span>
            <span class="storage-sub">{files.length} {t('Files').toLowerCase()} · {bytes(totalLibraryBytes)}</span>
          </div>
        </div>
      </div>
    </div>
  </aside>

  <!-- Main Workspace Area -->
  <main class="files-main">
    <!-- Top Navigation & Action Toolbar -->
    <header class="files-topbar">
      <!-- Breadcrumb & Summary -->
      <div class="breadcrumb-cluster">
        <nav class="breadcrumb-trail" aria-label="Breadcrumb">
          <span class="trail-root">Home Hub</span>
          <span class="trail-sep">/</span>
          <span class="trail-current">
            {filterNeedsBackup ? t('Needs Backup') : getCategoryLabel(category)}
          </span>
        </nav>
        <span class="summary-line">
          {displayedFiles.length} {displayedFiles.length === 1 ? 'item' : 'items'} · {bytes(totalDisplayedBytes)}
        </span>
      </div>

      <!-- Actions & Filters -->
      <div class="topbar-actions">
        <!-- Search Input -->
        <form class="search-form" on:submit|preventDefault={handleSearchSubmit}>
          <span class="search-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
          </span>
          <input
            type="search"
            bind:value={query}
            placeholder={t('Search files…')}
            aria-label={t('Search')}
            class="search-input"
          />
          {#if query}
            <button type="button" class="search-clear-btn" on:click={clearSearch} aria-label={t('Clear search')}>
              &times;
            </button>
          {/if}
        </form>

        <!-- Sort Selector -->
        <div class="sort-selector-wrap">
          <select value={sort} on:change={onSortChange} class="sort-select" aria-label={t('Sort')}>
            <option value="newest">{t('Newest')}</option>
            <option value="oldest">{t('Oldest')}</option>
            <option value="name">{t('Name')}</option>
            <option value="size">{t('Size')}</option>
          </select>
        </div>

        <!-- View Mode Switcher -->
        <div class="view-switch" role="group" aria-label="View mode">
          <button
            type="button"
            class="view-btn"
            class:active={viewMode === 'grid'}
            on:click={() => viewMode = 'grid'}
            title="Grid view"
            aria-label="Grid view"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="7" height="7"/>
              <rect x="14" y="3" width="7" height="7"/>
              <rect x="14" y="14" width="7" height="7"/>
              <rect x="3" y="14" width="7" height="7"/>
            </svg>
          </button>
          <button
            type="button"
            class="view-btn"
            class:active={viewMode === 'list'}
            on:click={() => viewMode = 'list'}
            title="List view"
            aria-label="List view"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="8" y1="6" x2="21" y2="6"/>
              <line x1="8" y1="12" x2="21" y2="12"/>
              <line x1="8" y1="18" x2="21" y2="18"/>
              <line x1="3" y1="6" x2="3.01" y2="6"/>
              <line x1="3" y1="12" x2="3.01" y2="12"/>
              <line x1="3" y1="18" x2="3.01" y2="18"/>
            </svg>
          </button>
        </div>

        <!-- Refresh Action Button -->
        <button
          type="button"
          class="action-icon-btn"
          on:click={() => run(load)}
          title={t('Refresh')}
          aria-label={t('Refresh')}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M23 4v6h-6"/>
            <path d="M1 20v-6h6"/>
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
          </svg>
        </button>
      </div>
    </header>

    <!-- Content Workspace -->
    <div class="workspace-layout">
      <div class="workspace-scroll">
        {#if displayedFiles.length === 0}
          <!-- EMPTY STATE -->
          <div class="empty-state">
            <div class="empty-icon-wrap">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                <line x1="9" y1="14" x2="15" y2="14"/>
              </svg>
            </div>
            <h3>
              {#if filterNeedsBackup}
                {t('All files verified')}
              {:else if category}
                {t('No files in this category')}
              {:else if query}
                {t('No matching files')}
              {:else}
                {t('Nothing here yet.')}
              {/if}
            </h3>
            <p>
              {#if filterNeedsBackup}
                {t('All files in your Home Hub library currently have a verified second copy.')}
              {:else if query}
                {t('Try searching with a different term or clear your search filter.')}
              {:else}
                {t('Import files into your Home Hub library via Storage or drag files into your Hub folder.')}
              {/if}
            </p>
            {#if query}
              <button type="button" class="empty-action-btn" on:click={clearSearch}>
                {t('Clear search')}
              </button>
            {/if}
          </div>
        {:else if viewMode === 'grid'}
          <!-- 1. GRID VIEW MODE -->
          <div class="file-grid" role="list">
            {#each displayedFiles as file (file.id)}
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div
                class="file-card"
                class:selected={selectedFile?.id === file.id}
                role="listitem"
                tabindex="0"
                on:click={() => selectedFile = file}
                on:dblclick={() => viewFile(file)}
                on:keydown={e => { if (e.key === 'Enter') viewFile(file); }}
              >
                <!-- Thumbnail / Icon Container (110px) -->
                <div class="card-thumb-container">
                  {#if (file.category === 'photo' || file.category === 'video' || file.mime?.startsWith('image/') || file.mime?.startsWith('video/')) && !failedThumbs[file.id]}
                    <img
                      src={getThumbSrc(file.id)}
                      alt={file.name}
                      loading="lazy"
                      class="card-thumb-img"
                      on:error={() => failedThumbs[file.id] = true}
                    />
                    {#if file.category === 'video' || file.mime?.startsWith('video/')}
                      <span class="video-play-badge">
                        <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="8 5 19 12 8 19 8 5"/></svg>
                      </span>
                    {/if}
                  {:else}
                    <div class="card-generic-icon cat-{getFileIconType(file)}">
                      {#if getFileIconType(file) === 'photo'}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
                      {:else if getFileIconType(file) === 'video'}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="2" y="4" width="20" height="16" rx="2"/><polygon points="10 8 16 12 10 16 10 8"/></svg>
                      {:else if getFileIconType(file) === 'music'}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                      {:else if getFileIconType(file) === 'backup'}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
                      {:else if getFileIconType(file) === 'download'}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                      {:else}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
                      {/if}
                    </div>
                  {/if}

                  <!-- Second Copy Status Dot -->
                  <div class="card-status-beacon" title={getSecondCopyLabel(file.second_copy_status)}>
                    {#if file.second_copy_status === 'verified'}
                      <span class="status-dot verified"></span>
                    {:else if file.second_copy_status === 'disconnected'}
                      <span class="status-dot disconnected"></span>
                    {:else}
                      <span class="status-dot needs-copy"></span>
                    {/if}
                  </div>
                </div>

                <!-- Info Area -->
                <div class="card-info">
                  <div class="card-name" title={file.name}>{file.name}</div>
                  <div class="card-meta">
                    <span class="card-size">{bytes(file.size)}</span>
                    <span class="card-category-tag">{t(file.category || 'file')}</span>
                  </div>
                </div>

                <!-- Quick Action Bar -->
                <div class="card-hover-actions">
                  <button
                    type="button"
                    class="card-act-btn"
                    title={t('Preview')}
                    aria-label={t('Preview')}
                    on:click|stopPropagation={() => viewFile(file)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
                  </button>
                  <button
                    type="button"
                    class="card-act-btn"
                    title={t('Download')}
                    aria-label={t('Download')}
                    on:click|stopPropagation={() => run(() => saveFile(file))}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                  </button>
                  <button
                    type="button"
                    class="card-act-btn"
                    title={t('Rename')}
                    aria-label={t('Rename')}
                    on:click|stopPropagation={() => fileForm(file, 'Rename')}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
                  </button>
                  <button
                    type="button"
                    class="card-act-btn danger"
                    title={t('Trash')}
                    aria-label={t('Trash')}
                    on:click|stopPropagation={() => confirm(t('Trash'), () => mutate('/api/files/' + file.id, 'DELETE'), file.name)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <!-- 2. LIST / TABLE VIEW MODE -->
          <div class="table-container">
            <table class="files-table">
              <thead>
                <tr>
                  <th class="th-name">{t('Name')}</th>
                  <th class="th-cat">{t('Category')}</th>
                  <th class="th-size">{t('Size')}</th>
                  <th class="th-status">{t('Second copy')}</th>
                  <th class="th-actions"><span class="sr-only">{t('Settings')}</span></th>
                </tr>
              </thead>
              <tbody>
                {#each displayedFiles as file (file.id)}
                  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <tr
                    class="table-row"
                    class:selected={selectedFile?.id === file.id}
                    on:click={() => selectedFile = file}
                  >
                    <td class="td-name">
                      <div class="cell-name-cluster">
                        <!-- Mini Icon / Thumbnail -->
                        <div class="mini-thumb">
                          {#if (file.category === 'photo' || file.category === 'video' || file.mime?.startsWith('image/') || file.mime?.startsWith('video/')) && !failedThumbs[file.id]}
                            <img
                              src={getThumbSrc(file.id)}
                              alt={file.name}
                              loading="lazy"
                              on:error={() => failedThumbs[file.id] = true}
                            />
                          {:else}
                            <div class="mini-icon cat-{getFileIconType(file)}">
                              {#if getFileIconType(file) === 'photo'}
                                📷
                              {:else if getFileIconType(file) === 'video'}
                                🎬
                              {:else if getFileIconType(file) === 'music'}
                                🎵
                              {:else if getFileIconType(file) === 'backup'}
                                💾
                              {:else if getFileIconType(file) === 'download'}
                                📥
                              {:else}
                                📄
                              {/if}
                            </div>
                          {/if}
                        </div>
                        <button
                          type="button"
                          class="file-name-btn"
                          on:click|stopPropagation={() => viewFile(file)}
                          title={file.name}
                        >
                          {file.name}
                        </button>
                      </div>
                    </td>
                    <td class="td-cat">
                      <span class="cat-pill cat-{getFileIconType(file)}">
                        {t(file.category || 'file')}
                      </span>
                    </td>
                    <td class="td-size">{bytes(file.size)}</td>
                    <td class="td-status">
                      {#if file.second_copy_status === 'verified'}
                        <span class="status-pill verified">
                          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"/></svg>
                          {t('Verified')}
                        </span>
                      {:else if file.second_copy_status === 'disconnected'}
                        <span class="status-pill disconnected">
                          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><circle cx="8" cy="8" r="4"/></svg>
                          {t('Drive disconnected')}
                        </span>
                      {:else}
                        <span class="status-pill needs-copy">
                          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><path d="M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"/></svg>
                          {t('Needs copy')}
                        </span>
                      {/if}
                    </td>
                    <td class="td-actions">
                      <div class="row-actions">
                        <button type="button" class="tbl-btn" title={t('Preview')} on:click|stopPropagation={() => viewFile(file)}>{t('Preview')}</button>
                        <button type="button" class="tbl-btn" title={t('Download')} on:click|stopPropagation={() => run(() => saveFile(file))}>{t('Download')}</button>
                        <button type="button" class="tbl-btn" title={t('Rename')} on:click|stopPropagation={() => fileForm(file, 'Rename')}>{t('Rename')}</button>
                        <button type="button" class="tbl-btn" title={t('Copy')} on:click|stopPropagation={() => fileForm(file, 'Copy')}>{t('Copy')}</button>
                        <button type="button" class="tbl-btn" title={t('Move')} on:click|stopPropagation={() => fileForm(file, 'Move')}>{t('Move')}</button>
                        <button type="button" class="tbl-btn danger" title={t('Trash')} on:click|stopPropagation={() => confirm(t('Trash'), () => mutate('/api/files/' + file.id, 'DELETE'), file.name)}>{t('Trash')}</button>
                      </div>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        <!-- Pagination Load More -->
        {#if cursor}
          <div class="pagination-wrap">
            <button type="button" class="load-more-btn" on:click={() => run(more)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                <path d="M12 5v14M5 12l7 7 7-7"/>
              </svg>
              {t('Load more')}
            </button>
          </div>
        {/if}
      </div>

      <!-- D. DETAIL INSPECTOR PANE (Slide-Out / Side Sheet) -->
      {#if selectedFile}
        <aside class="inspector-pane" aria-label="File inspector">
          <div class="inspector-header">
            <span class="inspector-title">{t('Inspector')}</span>
            <button
              type="button"
              class="inspector-close-btn"
              on:click={() => selectedFile = null}
              title={t('Close')}
              aria-label={t('Close')}
            >
              &times;
            </button>
          </div>

          <div class="inspector-content">
            <!-- Large Preview -->
            <div class="inspector-preview-box">
              {#if (selectedFile.category === 'photo' || selectedFile.category === 'video' || selectedFile.mime?.startsWith('image/') || selectedFile.mime?.startsWith('video/')) && !failedThumbs[selectedFile.id]}
                <img
                  src={getThumbSrc(selectedFile.id)}
                  alt={selectedFile.name}
                  class="inspector-thumb-img"
                  on:error={() => failedThumbs[selectedFile.id] = true}
                />
              {:else}
                <div class="inspector-large-icon cat-{getFileIconType(selectedFile)}">
                  {#if getFileIconType(selectedFile) === 'photo'}
                    📷
                  {:else if getFileIconType(selectedFile) === 'video'}
                    🎬
                  {:else if getFileIconType(selectedFile) === 'music'}
                    🎵
                  {:else if getFileIconType(selectedFile) === 'backup'}
                    💾
                  {:else if getFileIconType(selectedFile) === 'download'}
                    📥
                  {:else}
                    📄
                  {/if}
                </div>
              {/if}
            </div>

            <!-- Primary Metadata -->
            <div class="inspector-filename" title={selectedFile.name}>{selectedFile.name}</div>
            <div class="inspector-subpath">{selectedFile.rel_path || selectedFile.name}</div>

            <!-- Metadata List -->
            <div class="inspector-dl">
              <div class="meta-row">
                <span class="meta-label">{t('Size')}</span>
                <span class="meta-value">{bytes(selectedFile.size)}</span>
              </div>
              <div class="meta-row">
                <span class="meta-label">{t('Category')}</span>
                <span class="meta-value cat-tag cat-{getFileIconType(selectedFile)}">
                  {t(selectedFile.category || 'file')}
                </span>
              </div>
              <div class="meta-row">
                <span class="meta-label">{t('MIME')}</span>
                <span class="meta-value mime-tag">{selectedFile.mime || '—'}</span>
              </div>
              <div class="meta-row">
                <span class="meta-label">{t('Created')}</span>
                <span class="meta-value">{formatDate(selectedFile.created_at)}</span>
              </div>
              <div class="meta-row">
                <span class="meta-label">{t('Second copy')}</span>
                <span class="meta-value">
                  {#if selectedFile.second_copy_status === 'verified'}
                    <span class="status-pill verified">✓ {t('Verified')}</span>
                  {:else if selectedFile.second_copy_status === 'disconnected'}
                    <span class="status-pill disconnected">⚡ {t('Drive disconnected')}</span>
                  {:else}
                    <span class="status-pill needs-copy">⚠ {t('Needs copy')}</span>
                  {/if}
                </span>
              </div>
            </div>

            <!-- Inspector Action Buttons -->
            <div class="inspector-actions">
              <button
                type="button"
                class="inspector-btn primary"
                on:click={() => viewFile(selectedFile)}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>
                </svg>
                {t('Preview')}
              </button>

              <button
                type="button"
                class="inspector-btn"
                on:click={() => run(() => saveFile(selectedFile))}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                {t('Download')}
              </button>

              <div class="inspector-btn-row">
                <button
                  type="button"
                  class="inspector-btn-sm"
                  on:click={() => fileForm(selectedFile, 'Rename')}
                >
                  {t('Rename')}
                </button>
                <button
                  type="button"
                  class="inspector-btn-sm"
                  on:click={() => fileForm(selectedFile, 'Copy')}
                >
                  {t('Copy')}
                </button>
                <button
                  type="button"
                  class="inspector-btn-sm"
                  on:click={() => fileForm(selectedFile, 'Move')}
                >
                  {t('Move')}
                </button>
              </div>

              <button
                type="button"
                class="inspector-btn danger"
                on:click={() => confirm(t('Trash'), () => mutate('/api/files/' + selectedFile.id, 'DELETE'), selectedFile.name)}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                  <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                </svg>
                {t('Trash')}
              </button>
            </div>
          </div>
        </aside>
      {/if}
    </div>
  </main>
</div>

<style>
  /* Root Container */
  .files-app {
    display: flex;
    flex-direction: row;
    height: 100%;
    width: 100%;
    overflow: hidden;
    background: #0c1015;
    color: #f1f5f9;
    font-family: inherit;
    user-select: none;
  }

  /* Left Sidebar (230px) */
  .files-sidebar {
    width: 230px;
    min-width: 230px;
    background: rgba(16, 20, 26, 0.85);
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    backdrop-filter: blur(16px);
  }

  .sidebar-header {
    padding: 16px 14px 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .sidebar-brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .folder-badge {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: linear-gradient(135deg, #3b82f6, #1d4ed8);
    display: grid;
    place-items: center;
    color: #fff;
    box-shadow: 0 2px 8px rgba(59, 130, 246, 0.4);
  }

  .brand-svg {
    width: 18px;
    height: 18px;
  }

  .brand-text {
    display: flex;
    flex-direction: column;
  }

  .brand-title {
    font-size: 14px;
    font-weight: 650;
    line-height: 1.2;
    color: #f8fafc;
    letter-spacing: -0.01em;
  }

  .brand-subtitle {
    font-size: 10.5px;
    color: #94a3b8;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .total-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.08);
    color: #94a3b8;
  }

  .sidebar-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  .nav-section {
    display: flex;
    flex-direction: column;
  }

  .nav-section-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #64748b;
    padding: 4px 8px 6px;
  }

  .nav-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: #cbd5e1;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: all 140ms ease;
  }

  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #f8fafc;
  }

  .nav-item.active {
    background: rgba(59, 130, 246, 0.16);
    color: #60a5fa;
    border-color: rgba(59, 130, 246, 0.3);
    font-weight: 600;
  }

  .nav-icon {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
  }

  .nav-icon svg {
    width: 16px;
    height: 16px;
  }

  .nav-icon.blue { color: #60a5fa; }
  .nav-icon.amber { color: #f59e0b; }
  .nav-icon.rose { color: #f43f5e; }
  .nav-icon.cat-photo { color: #ec4899; }
  .nav-icon.cat-video { color: #a78bfa; }
  .nav-icon.cat-document { color: #60a5fa; }
  .nav-icon.cat-music { color: #34d399; }
  .nav-icon.cat-backup { color: #fbbf24; }
  .nav-icon.cat-download { color: #38bdf8; }

  .nav-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count-pill {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: #64748b;
    padding: 1px 6px;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.04);
  }

  .count-pill.alert {
    background: rgba(244, 63, 94, 0.2);
    color: #fb7185;
    font-weight: 600;
  }

  .storage-division {
    margin-top: auto;
    padding-top: 10px;
  }

  .storage-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .storage-icon-wrap {
    width: 28px;
    height: 28px;
    border-radius: 7px;
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .storage-icon-wrap svg {
    width: 16px;
    height: 16px;
  }

  .storage-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .storage-title {
    font-size: 11.5px;
    font-weight: 600;
    color: #e2e8f0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .storage-sub {
    font-size: 10px;
    color: #64748b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Main Workspace */
  .files-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: radial-gradient(ellipse at 50% 0%, rgba(59, 130, 246, 0.08) 0%, transparent 60%), #0c1015;
  }

  /* Top Bar */
  .files-topbar {
    height: 54px;
    min-height: 54px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(15, 20, 28, 0.7);
    backdrop-filter: blur(12px);
  }

  .breadcrumb-cluster {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .breadcrumb-trail {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13.5px;
    font-weight: 600;
  }

  .trail-root {
    color: #94a3b8;
  }

  .trail-sep {
    color: #475569;
    font-size: 12px;
  }

  .trail-current {
    color: #f8fafc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .summary-line {
    font-size: 11px;
    color: #64748b;
  }

  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .search-form {
    position: relative;
    display: flex;
    align-items: center;
    width: 200px;
  }

  .search-icon {
    position: absolute;
    left: 9px;
    display: grid;
    place-items: center;
    color: #64748b;
    pointer-events: none;
  }

  .search-icon svg {
    width: 14px;
    height: 14px;
  }

  .search-input {
    width: 100%;
    height: 32px;
    padding: 0 28px 0 30px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #f8fafc;
    font-size: 12.5px;
    outline: none;
    transition: all 150ms ease;
  }

  .search-input:focus {
    background: rgba(255, 255, 255, 0.1);
    border-color: #3b82f6;
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.25);
  }

  .search-clear-btn {
    position: absolute;
    right: 6px;
    background: none;
    border: none;
    color: #94a3b8;
    cursor: pointer;
    font-size: 14px;
    padding: 2px 4px;
    min-height: 0;
  }

  .search-clear-btn:hover {
    color: #fff;
  }

  .sort-selector-wrap {
    position: relative;
  }

  .sort-select {
    height: 32px;
    padding: 0 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #cbd5e1;
    font-size: 12px;
    outline: none;
    cursor: pointer;
  }

  .sort-select:focus {
    border-color: #3b82f6;
  }

  .view-switch {
    display: flex;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 2px;
    gap: 2px;
  }

  .view-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    min-height: 0;
    padding: 0;
    border: none;
    background: transparent;
    color: #94a3b8;
    border-radius: 6px;
    cursor: pointer;
    transition: all 140ms ease;
  }

  .view-btn svg {
    width: 14px;
    height: 14px;
  }

  .view-btn:hover {
    color: #f8fafc;
  }

  .view-btn.active {
    background: rgba(59, 130, 246, 0.25);
    color: #60a5fa;
  }

  .action-icon-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    min-height: 0;
    padding: 0;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #94a3b8;
    cursor: pointer;
    transition: all 140ms ease;
  }

  .action-icon-btn svg {
    width: 15px;
    height: 15px;
  }

  .action-icon-btn:hover {
    color: #f8fafc;
    border-color: rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.1);
  }

  /* Workspace Layout & Scrolling */
  .workspace-layout {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
    overflow: hidden;
    position: relative;
  }

  .workspace-scroll {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow-y: auto;
    padding: 16px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }

  /* Grid View */
  .file-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 14px;
  }

  .file-card {
    position: relative;
    background: rgba(22, 28, 36, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 12px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    cursor: pointer;
    transition: all 160ms cubic-bezier(0.4, 0, 0.2, 1);
    user-select: none;
    outline: none;
  }

  .file-card:hover {
    background: rgba(30, 38, 50, 0.9);
    border-color: rgba(59, 130, 246, 0.4);
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.35);
  }

  .file-card.selected {
    background: rgba(30, 58, 95, 0.5);
    border-color: #3b82f6;
    box-shadow: 0 0 0 1px #3b82f6, 0 8px 24px rgba(59, 130, 246, 0.25);
  }

  .file-card:focus-visible {
    box-shadow: 0 0 0 2px #3b82f6;
  }

  .card-thumb-container {
    height: 110px;
    width: 100%;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(10, 14, 20, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
  }

  .card-thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 200ms ease;
  }

  .file-card:hover .card-thumb-img {
    transform: scale(1.04);
  }

  .video-play-badge {
    position: absolute;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    color: #fff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  }

  .video-play-badge svg {
    width: 16px;
    height: 16px;
    margin-left: 2px;
  }

  .card-generic-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border-radius: 12px;
  }

  .card-generic-icon svg {
    width: 28px;
    height: 28px;
  }

  .card-generic-icon.cat-photo { background: rgba(236, 72, 153, 0.15); color: #ec4899; }
  .card-generic-icon.cat-video { background: rgba(139, 92, 246, 0.15); color: #a78bfa; }
  .card-generic-icon.cat-document { background: rgba(59, 130, 246, 0.15); color: #60a5fa; }
  .card-generic-icon.cat-music { background: rgba(16, 185, 129, 0.15); color: #34d399; }
  .card-generic-icon.cat-backup { background: rgba(245, 158, 11, 0.15); color: #fbbf24; }
  .card-generic-icon.cat-download { background: rgba(6, 182, 212, 0.15); color: #38bdf8; }

  .card-status-beacon {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
  }

  .status-dot.verified {
    background: #10b981;
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.6);
  }

  .status-dot.disconnected {
    background: #f97316;
  }

  .status-dot.needs-copy {
    background: #f59e0b;
  }

  .card-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .card-name {
    font-size: 12px;
    font-weight: 550;
    line-height: 1.3;
    color: #f1f5f9;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-all;
  }

  .card-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    color: #64748b;
  }

  .card-category-tag {
    text-transform: capitalize;
    color: #94a3b8;
  }

  .card-hover-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    opacity: 0.6;
    transition: opacity 140ms ease;
  }

  .file-card:hover .card-hover-actions {
    opacity: 1;
  }

  .card-act-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    min-height: 0;
    padding: 0;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #cbd5e1;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .card-act-btn svg {
    width: 13px;
    height: 13px;
  }

  .card-act-btn:hover {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    color: #fff;
  }

  .card-act-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
    color: #f87171;
  }

  /* List / Table View */
  .table-container {
    width: 100%;
    overflow-x: auto;
    background: rgba(22, 28, 36, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
  }

  .files-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
    font-size: 13px;
  }

  .files-table th {
    padding: 10px 14px;
    font-size: 11.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #64748b;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(16, 22, 30, 0.6);
  }

  .table-row {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    cursor: pointer;
    transition: background 140ms ease;
  }

  .table-row:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .table-row.selected {
    background: rgba(30, 58, 95, 0.35);
  }

  .table-row td {
    padding: 9px 14px;
    vertical-align: middle;
  }

  .cell-name-cluster {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 180px;
  }

  .mini-thumb {
    width: 30px;
    height: 30px;
    border-radius: 6px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.4);
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .mini-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .mini-icon {
    font-size: 16px;
  }

  .file-name-btn {
    border: none;
    background: none;
    padding: 0;
    min-height: 0;
    color: #f1f5f9;
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 260px;
  }

  .file-name-btn:hover {
    color: #60a5fa;
    text-decoration: underline;
  }

  .cat-pill {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 500;
    text-transform: capitalize;
    background: rgba(255, 255, 255, 0.05);
    color: #94a3b8;
  }

  .td-size {
    color: #94a3b8;
    font-variant-numeric: tabular-nums;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    padding: 2px 8px;
    border-radius: 12px;
    font-weight: 500;
  }

  .status-pill.verified {
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
  }

  .status-pill.needs-copy {
    background: rgba(245, 158, 11, 0.15);
    color: #fbbf24;
  }

  .status-pill.disconnected {
    background: rgba(249, 115, 22, 0.15);
    color: #fb923c;
  }

  .row-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    flex-wrap: nowrap;
  }

  .tbl-btn {
    padding: 3px 8px;
    min-height: 26px;
    font-size: 11px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #cbd5e1;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .tbl-btn:hover {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    color: #fff;
  }

  .tbl-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
    color: #f87171;
  }

  /* Empty State */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 60px 20px;
    color: #64748b;
  }

  .empty-icon-wrap {
    width: 64px;
    height: 64px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    display: grid;
    place-items: center;
    color: #475569;
    margin-bottom: 16px;
  }

  .empty-icon-wrap svg {
    width: 32px;
    height: 32px;
  }

  .empty-state h3 {
    font-size: 16px;
    font-weight: 600;
    color: #e2e8f0;
    margin: 0 0 6px;
  }

  .empty-state p {
    font-size: 13px;
    max-width: 360px;
    line-height: 1.5;
    margin: 0 0 16px;
  }

  .empty-action-btn {
    padding: 6px 14px;
    font-size: 12px;
    border-radius: 8px;
    background: rgba(59, 130, 246, 0.15);
    border: 1px solid rgba(59, 130, 246, 0.3);
    color: #60a5fa;
    cursor: pointer;
  }

  /* Pagination */
  .pagination-wrap {
    display: flex;
    justify-content: center;
    padding: 20px 0;
  }

  .load-more-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 20px;
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #f1f5f9;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 150ms ease;
  }

  .load-more-btn:hover {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    color: #fff;
  }

  /* Inspector Pane (270px) */
  .inspector-pane {
    width: 270px;
    min-width: 270px;
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(16, 22, 30, 0.9);
    backdrop-filter: blur(16px);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    height: 100%;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }

  .inspector-header {
    padding: 14px 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .inspector-title {
    font-size: 13px;
    font-weight: 650;
    color: #f8fafc;
  }

  .inspector-close-btn {
    background: none;
    border: none;
    color: #94a3b8;
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
    min-height: 0;
  }

  .inspector-close-btn:hover {
    color: #fff;
  }

  .inspector-content {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .inspector-preview-box {
    width: 100%;
    height: 150px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.5);
    overflow: hidden;
    display: grid;
    place-items: center;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .inspector-thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .inspector-large-icon {
    font-size: 48px;
  }

  .inspector-filename {
    font-size: 13.5px;
    font-weight: 600;
    color: #f8fafc;
    word-break: break-all;
    line-height: 1.35;
  }

  .inspector-subpath {
    font-size: 11px;
    color: #64748b;
    word-break: break-all;
    margin-top: -10px;
  }

  .inspector-dl {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    gap: 8px;
  }

  .meta-label {
    color: #64748b;
    font-weight: 500;
  }

  .meta-value {
    color: #e2e8f0;
    font-weight: 500;
    text-align: right;
    word-break: break-all;
  }

  .cat-tag {
    text-transform: capitalize;
    color: #60a5fa;
  }

  .mime-tag {
    font-size: 10.5px;
    color: #94a3b8;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .inspector-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 4px;
  }

  .inspector-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    min-height: 34px;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 12.5px;
    font-weight: 500;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #e2e8f0;
    cursor: pointer;
    transition: all 140ms ease;
  }

  .inspector-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .inspector-btn.primary {
    background: #3b82f6;
    border-color: #3b82f6;
    color: #fff;
    font-weight: 600;
  }

  .inspector-btn.primary:hover {
    background: #2563eb;
  }

  .inspector-btn.danger {
    color: #f87171;
    border-color: rgba(239, 68, 68, 0.25);
  }

  .inspector-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
  }

  .inspector-btn-row {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 6px;
  }

  .inspector-btn-sm {
    min-height: 30px;
    padding: 4px;
    font-size: 11.5px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #cbd5e1;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .inspector-btn-sm:hover {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    color: #fff;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }
</style>
