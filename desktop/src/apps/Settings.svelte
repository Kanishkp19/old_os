<script>
  import { native, api, command, backupSummary } from '../api.js';
  import { onMount } from 'svelte';
  import Details from '../Details.svelte';
  import { exportPrivate, importPrivate } from '../state/private.js';

  export let language;
  export let large;
  export let contrast;
  export let reduced;
  export let notifications;
  export let busy = false;
  export let settings = {};
  export let setup = false;
  export let setupStep = 0;
  export let hubName = '';
  export let copyFolder = '';
  export let privateTrash = [];
  export let updateCheck = null;
  export let t = s => s;
  export let run = async (fn, _msg) => { await fn(); };
  export let load = async () => {};
  export let confirm = (_title, _fn, _detail) => {};
  export let saveSettings = async () => {};
  export let flushNote = async () => {};
  export let savePreferences = async () => {};
  export let diagnostics = async () => {};
  export let checkUpdates = async () => {};
  export let loadPrivateTrash = async () => {};

  let backup = null;
  let network = null;
  let summaryError = '';
  let activeSection = 'general';
  let searchQuery = '';

  onMount(() => {
    if (native) {
      backupSummary()
        .then(value => { backup = value; })
        .catch(e => { summaryError = String(e?.message || e); });
    }
    api('/api/network')
      .then(value => { network = value; })
      .catch(e => { summaryError = String(e?.message || e); });
  });

  const sections = [
    {
      id: 'general',
      name: 'General',
      subtitle: 'System identity, discovery controls, and setup wizard',
      keywords: 'name hub appliance mode pause sharing privacy setup wizard identity broadcast'
    },
    {
      id: 'storage',
      name: 'Storage & Backup',
      subtitle: 'Protected library path, redundant second copies, and health',
      keywords: 'storage backup library folder second copy drive health scrub drive bitlocker'
    },
    {
      id: 'schedules',
      name: 'Schedules & Power',
      subtitle: 'Automated backup frequency, integrity scrubs, and scheduled wake',
      keywords: 'schedules schedule auto backup interval scrub wake sleep screen power sharing'
    },
    {
      id: 'language',
      name: 'Language',
      subtitle: 'System locale and display language',
      keywords: 'language english hindi bhasha translation locale'
    },
    {
      id: 'display',
      name: 'Display & Accessibility',
      subtitle: 'Visual sizing, high contrast, reduced motion, and banners',
      keywords: 'display accessibility large text contrast motion notification theme font visual banners'
    },
    {
      id: 'updates',
      name: 'System Updates',
      subtitle: 'Signed updates, release verification, and slot telemetry',
      keywords: 'system updates version signed release channel slot patch manifest key check'
    },
    {
      id: 'diagnostics',
      name: 'Diagnostics & Data',
      subtitle: 'Redacted diagnostic dumps and private app export/import',
      keywords: 'diagnostics data export import private apps telemetry log status bitlocker'
    },
    {
      id: 'trash',
      name: 'Private Trash',
      subtitle: 'Recover deleted notes, playlists, and bookmarks',
      keywords: 'private trash bin deleted notes playlists bookmarks restore purge delete'
    },
    {
      id: 'network',
      name: 'Network',
      subtitle: 'Local IP addresses, ports, and interface topology',
      keywords: 'network local ip interfaces ports addresses status wifi connection adapter'
    }
  ];

  $: filteredSections = sections.filter(sec => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase().trim();
    return (
      sec.name.toLowerCase().includes(q) ||
      t(sec.name).toLowerCase().includes(q) ||
      sec.subtitle.toLowerCase().includes(q) ||
      sec.keywords.toLowerCase().includes(q)
    );
  });

  $: if (searchQuery.trim() && !filteredSections.some(s => s.id === activeSection) && filteredSections.length > 0) {
    activeSection = filteredSections[0].id;
  }

  $: currentSectionMeta = sections.find(s => s.id === activeSection) || sections[0];

  async function handleSaveSettings() {
    await run(saveSettings, t('Saved'));
  }

  async function handleTogglePauseSharing() {
    await run(async () => {
      await api('/api/pause-sharing', 'POST', { paused: !settings?.pause_sharing });
      await load();
    });
  }

  function handleRerunSetup() {
    setup = true;
    setupStep = 0;
  }

  function setLanguage(lang) {
    if (language !== lang) {
      language = lang;
      savePreferences();
    }
  }

  function toggleLarge() {
    large = !large;
    savePreferences();
  }

  function toggleContrast() {
    contrast = !contrast;
    savePreferences();
  }

  function toggleReduced() {
    reduced = !reduced;
    savePreferences();
  }

  function toggleNotifications() {
    notifications = !notifications;
    savePreferences();
  }

  async function handleExportPrivate() {
    await run(async () => {
      await flushNote();
      await exportPrivate();
    });
  }

  async function handleImportPrivate() {
    await run(async () => {
      await importPrivate();
      await loadPrivateTrash();
    });
  }

  async function handleRestoreTrash(row) {
    await run(async () => {
      await command('local_restore', { id: row.id });
      await loadPrivateTrash();
    });
  }

  function handlePurgeTrash(row) {
    confirm(
      t('Delete permanently'),
      async () => {
        await command('local_purge', { id: row.id });
        await loadPrivateTrash();
      },
      row.title
    );
  }
</script>

<div class="settings-hub" class:contrast-theme={contrast}>
  <!-- Left Sidebar Navigation -->
  <aside class="settings-sidebar" aria-label={t('Settings')}>
    <div class="sidebar-top">
      <div class="sidebar-title-bar">
        <h2 class="sidebar-heading">{t('Settings')}</h2>
      </div>
      <div class="search-wrapper">
        <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <input
          type="search"
          class="sidebar-search-input"
          placeholder={t('Search settings…')}
          bind:value={searchQuery}
          aria-label={t('Search settings…')}
        />
        {#if searchQuery}
          <button type="button" class="search-clear-btn" on:click={() => { searchQuery = ''; }} aria-label={t('Clear Search')}>
            ×
          </button>
        {/if}
      </div>
    </div>

    <nav class="sidebar-nav" aria-label={t('Settings')}>
      {#each filteredSections as section (section.id)}
        {@const isActive = activeSection === section.id}
        <button
          type="button"
          class="nav-tab-btn"
          class:active={isActive}
          on:click={() => { activeSection = section.id; }}
          aria-current={isActive ? 'page' : undefined}
        >
          <span class="nav-icon-slot" aria-hidden="true">
            {#if section.id === 'general'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="3"/>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
              </svg>
            {:else if section.id === 'storage'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="4" width="20" height="16" rx="2"/><path d="M6 12h.01M10 12h.01"/><path d="M2 16h20"/>
              </svg>
            {:else if section.id === 'schedules'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
              </svg>
            {:else if section.id === 'language'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
              </svg>
            {:else if section.id === 'display'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>
              </svg>
            {:else if section.id === 'updates'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
              </svg>
            {:else if section.id === 'diagnostics'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
              </svg>
            {:else if section.id === 'trash'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
              </svg>
            {:else if section.id === 'network'}
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.55a11 11 0 0 1 14.08 0M1.42 9a16 16 0 0 1 21.16 0M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/>
              </svg>
            {/if}
          </span>
          <span class="nav-label">{t(section.name)}</span>
          {#if section.id === 'trash' && privateTrash?.length > 0}
            <span class="nav-count-badge" aria-label="{privateTrash.length} items">{privateTrash.length}</span>
          {/if}
          {#if section.id === 'updates' && updateCheck?.available}
            <span class="nav-dot-badge" title={t('Update available')} aria-label={t('Update available')}></span>
          {/if}
        </button>
      {:else}
        <div class="search-empty-state">
          <p>{t('No matching settings found')}</p>
        </div>
      {/each}
    </nav>
  </aside>

  <!-- Right Detail Content Panel -->
  <main class="settings-content" tabindex="-1">
    <header class="content-header">
      <div class="header-titles">
        <h1 class="active-section-title">{t(currentSectionMeta.name)}</h1>
        <p class="active-section-desc">{t(currentSectionMeta.subtitle)}</p>
      </div>
    </header>

    <div class="section-body">
      <!-- 1. GENERAL -->
      {#if activeSection === 'general'}
        <div class="hero-appliance-card">
          <div class="hero-symbol-circle">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
              <polyline points="9 22 9 12 15 12 15 22"/>
            </svg>
          </div>
          <div class="hero-content">
            <div class="hero-badge-row">
              <span class="hero-badge">{t('Local Appliance')}</span>
              <span class="status-pill active">
                <span class="dot"></span>
                {t('Connected locally')}
              </span>
            </div>
            <h3 class="hero-title">{hubName || settings.hub_name || 'Home Hub'}</h3>
            <p class="hero-text">{t('Private by design. Offline-capable local cloud for media streaming, photos, and backups.')}</p>
          </div>
        </div>

        <section class="pref-card" aria-labelledby="general-name-heading">
          <div class="pref-card-header">
            <h4 id="general-name-heading">{t('System Identity')}</h4>
            <p class="pref-card-subtitle">{t('Configure network name broadcast to local clients')}</p>
          </div>
          <form on:submit|preventDefault={handleSaveSettings}>
            <div class="pref-row">
              <div class="pref-meta">
                <label for="hub-name-input" class="pref-title">{t('Hub name')}</label>
                <span class="pref-desc">{t('The friendly broadcast name shown to paired phones & tablets')}</span>
              </div>
              <div class="pref-control-group">
                <input
                  id="hub-name-input"
                  class="modern-input"
                  bind:value={hubName}
                  maxlength="120"
                  required
                  placeholder="e.g. Living Room Hub"
                />
                <button type="submit" class="primary-btn" disabled={busy || !hubName.trim()}>
                  {t('Save Changes')}
                </button>
              </div>
            </div>
          </form>
        </section>

        <section class="pref-card" aria-labelledby="general-privacy-heading">
          <div class="pref-card-header">
            <h4 id="general-privacy-heading">{t('Local Network Sharing')}</h4>
            <p class="pref-card-subtitle">{t('Control local network discovery and peer syncing')}</p>
          </div>
          <div class="pref-row">
            <div class="pref-meta">
              <div class="pref-title-with-badge">
                <span class="pref-title">{t('Network Discovery & Sync')}</span>
                <span class="status-pill" class:active={!settings?.pause_sharing} class:paused={settings?.pause_sharing}>
                  <span class="dot"></span>
                  {settings?.pause_sharing ? t('Sharing Paused') : t('Active & Discoverable')}
                </span>
              </div>
              <span class="pref-desc">{t('Pause discovery and sharing across all local devices without disconnecting.')}</span>
            </div>
            <div class="pref-control">
              <button
                type="button"
                class={settings?.pause_sharing ? 'primary-btn' : 'secondary-btn'}
                disabled={busy}
                on:click={handleTogglePauseSharing}
              >
                {t(settings?.pause_sharing ? 'Resume sharing' : 'Pause sharing')}
              </button>
            </div>
          </div>
        </section>

        <section class="pref-card" aria-labelledby="general-wizard-heading">
          <div class="pref-card-header">
            <h4 id="general-wizard-heading">{t('Appliance Setup')}</h4>
            <p class="pref-card-subtitle">{t('First-run walkthrough and storage configuration')}</p>
          </div>
          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('First-Run Setup Wizard')}</span>
              <span class="pref-desc">{t('Re-execute initial configuration, storage assignment, and pairing walkthrough')}</span>
            </div>
            <div class="pref-control">
              <button type="button" class="secondary-btn" on:click={handleRerunSetup}>
                {t('Rerun Setup Wizard')}
              </button>
            </div>
          </div>
        </section>

      <!-- 2. STORAGE & BACKUP -->
      {:else if activeSection === 'storage'}
        <section class="pref-card" aria-labelledby="storage-loc-heading">
          <div class="pref-card-header">
            <h4 id="storage-loc-heading">{t('Storage Folders')}</h4>
            <p class="pref-card-subtitle">{t('Primary library and redundant secondary mirror destinations')}</p>
          </div>
          <div class="pref-row">
            <div class="pref-meta">
              <div class="pref-title-with-badge">
                <span class="pref-title">{t('Primary Library Storage')}</span>
                <span class="status-pill verified">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                  {t('Protected System Storage')}
                </span>
              </div>
              <span class="pref-desc">{t('Main storage repository where photos, videos, and files are indexed.')}</span>
            </div>
            <div class="pref-control">
              <code class="path-badge">{settings?.library_root || t('Protected System Storage')}</code>
            </div>
          </div>

          <div class="pref-row">
            <div class="pref-meta">
              <div class="pref-title-with-badge">
                <label for="copy-folder-input" class="pref-title">{t('Second-Copy Drive')}</label>
                <span class="status-pill warning">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                  {t('A single drive is not a backup.')}
                </span>
              </div>
              <span class="pref-desc">{t('Destination folder on external disk or secondary drive for mirrored redundancy.')}</span>
            </div>
            <div class="pref-control-group">
              <input
                id="copy-folder-input"
                class="modern-input"
                bind:value={copyFolder}
                placeholder="e.g. D:\Backups\HomeHub"
              />
              <button type="button" class="primary-btn" disabled={busy} on:click={handleSaveSettings}>
                {t('Save')}
              </button>
            </div>
          </div>
        </section>

        <section class="pref-card" aria-labelledby="storage-health-heading">
          <div class="pref-card-header">
            <h4 id="storage-health-heading">{t('Health & Backups')}</h4>
            <p class="pref-card-subtitle">{t('Turn on BitLocker in Windows to protect data at rest.')}</p>
          </div>
          <div class="pref-card-body">
            {#if summaryError}
              <div class="alert-banner error" role="alert">{summaryError}</div>
            {/if}
            {#if backup}
              <Details value={backup} {t}/>
            {:else}
              <p class="empty-hint">{t('Diagnostics include system status and omit file names and contents.')}</p>
            {/if}
          </div>
        </section>

      <!-- 3. SCHEDULES & POWER -->
      {:else if activeSection === 'schedules'}
        <form on:submit|preventDefault={handleSaveSettings}>
          <section class="pref-card" aria-labelledby="sched-intervals-heading">
            <div class="pref-card-header">
              <h4 id="sched-intervals-heading">{t('Backup and second-copy schedules')}</h4>
              <p class="pref-card-subtitle">{t('Intervals for background automated integrity verification and syncing')}</p>
            </div>
            <div class="pref-row">
              <div class="pref-meta">
                <label for="sched-backup-input" class="pref-title">{t('Auto-Backup Interval')}</label>
                <span class="pref-desc">{t('Backup interval (minutes)')}</span>
              </div>
              <div class="pref-control">
                <div class="input-with-unit">
                  <input
                    id="sched-backup-input"
                    type="number"
                    min="1"
                    max="10080"
                    class="stepper-input"
                    bind:value={settings.backup_interval_minutes}
                  />
                  <span class="unit-label">min</span>
                </div>
              </div>
            </div>

            <div class="pref-row">
              <div class="pref-meta">
                <label for="sched-scrub-input" class="pref-title">{t('Storage Scrub Interval')}</label>
                <span class="pref-desc">{t('Scrub interval (hours)')}</span>
              </div>
              <div class="pref-control">
                <div class="input-with-unit">
                  <input
                    id="sched-scrub-input"
                    type="number"
                    min="1"
                    max="720"
                    class="stepper-input"
                    bind:value={settings.scrub_interval_hours}
                  />
                  <span class="unit-label">hrs</span>
                </div>
              </div>
            </div>

            <div class="pref-row">
              <div class="pref-meta">
                <label for="sched-second-input" class="pref-title">{t('Second-Copy Mirror Interval')}</label>
                <span class="pref-desc">{t('Second-copy interval (minutes)')}</span>
              </div>
              <div class="pref-control">
                <div class="input-with-unit">
                  <input
                    id="sched-second-input"
                    type="number"
                    min="1"
                    max="43200"
                    class="stepper-input"
                    bind:value={settings.second_copy_interval_minutes}
                  />
                  <span class="unit-label">min</span>
                </div>
              </div>
            </div>
          </section>

          <section class="pref-card" aria-labelledby="sched-power-heading">
            <div class="pref-card-header">
              <h4 id="sched-power-heading">{t('Power & Screen Sharing')}</h4>
              <p class="pref-card-subtitle">{t('Wake timer configurations and media casting permissions')}</p>
            </div>
            <div class="pref-row">
              <div class="pref-meta">
                <span class="pref-title">{t('Allow Screen Sharing')}</span>
                <span class="pref-desc">{t('Allow screen sharing')}</span>
              </div>
              <div class="pref-control">
                <label class="switch-toggle" aria-label={t('Allow screen sharing')}>
                  <input type="checkbox" bind:checked={settings.screen_enabled} />
                  <span class="switch-slider"></span>
                </label>
              </div>
            </div>

            <div class="pref-row">
              <div class="pref-meta">
                <span class="pref-title">{t('Scheduled Wake Request')}</span>
                <span class="pref-desc">{t('Request scheduled wake')}</span>
              </div>
              <div class="pref-control">
                <label class="switch-toggle" aria-label={t('Request scheduled wake')}>
                  <input type="checkbox" bind:checked={settings.wake_enabled} />
                  <span class="switch-slider"></span>
                </label>
              </div>
            </div>

            {#if settings.wake_enabled}
              <div class="pref-row">
                <div class="pref-meta">
                  <label for="sched-wake-time" class="pref-title">{t('Daily Wake Time')}</label>
                  <span class="pref-desc">{t('Wake time')}</span>
                </div>
                <div class="pref-control">
                  <input
                    id="sched-wake-time"
                    type="time"
                    class="modern-time-input"
                    bind:value={settings.wake_time}
                  />
                </div>
              </div>
            {/if}
          </section>

          <div class="form-actions-row">
            <button type="submit" class="primary-btn" disabled={busy}>
              {t('Save Changes')}
            </button>
          </div>
        </form>

      <!-- 4. LANGUAGE -->
      {:else if activeSection === 'language'}
        <section class="pref-card" aria-labelledby="lang-select-heading">
          <div class="pref-card-header">
            <h4 id="lang-select-heading">{t('Language')}</h4>
            <p class="pref-card-subtitle">{t('Choose your interface language across all windows')}</p>
          </div>
          <div class="pref-card-body">
            <div class="lang-cards-grid">
              <button
                type="button"
                class="lang-choice-card"
                class:selected={language === 'en'}
                on:click={() => setLanguage('en')}
                aria-pressed={language === 'en'}
              >
                <div class="lang-card-top">
                  <span class="lang-symbol">🇺🇸</span>
                  <span class="check-circle" class:checked={language === 'en'}>
                    {#if language === 'en'}
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5"><polyline points="20 6 9 17 4 12"/></svg>
                    {/if}
                  </span>
                </div>
                <div class="lang-card-text">
                  <span class="lang-native-title">English</span>
                  <span class="lang-english-sub">Default language</span>
                </div>
              </button>

              <button
                type="button"
                class="lang-choice-card"
                class:selected={language === 'hi'}
                on:click={() => setLanguage('hi')}
                aria-pressed={language === 'hi'}
              >
                <div class="lang-card-top">
                  <span class="lang-symbol">🇮🇳</span>
                  <span class="check-circle" class:checked={language === 'hi'}>
                    {#if language === 'hi'}
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5"><polyline points="20 6 9 17 4 12"/></svg>
                    {/if}
                  </span>
                </div>
                <div class="lang-card-text">
                  <span class="lang-native-title">हिन्दी (Hindi)</span>
                  <span class="lang-english-sub">प्राथमिक भाषा</span>
                </div>
              </button>
            </div>
          </div>
        </section>

      <!-- 5. DISPLAY & ACCESSIBILITY -->
      {:else if activeSection === 'display'}
        <section class="pref-card" aria-labelledby="display-heading">
          <div class="pref-card-header">
            <h4 id="display-heading">{t('Display & Accessibility')}</h4>
            <p class="pref-card-subtitle">{t('Notification preferences')}</p>
          </div>
          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('Larger text')}</span>
              <span class="pref-desc">{t('Scale typography size across all apps and toolbars for improved legibility')}</span>
            </div>
            <div class="pref-control">
              <label class="switch-toggle" aria-label={t('Larger text')}>
                <input type="checkbox" checked={large} on:change={toggleLarge} />
                <span class="switch-slider"></span>
              </label>
            </div>
          </div>

          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('High contrast')}</span>
              <span class="pref-desc">{t('Increase contrast ratios and sharp border outlines for visual clarity')}</span>
            </div>
            <div class="pref-control">
              <label class="switch-toggle" aria-label={t('High contrast')}>
                <input type="checkbox" checked={contrast} on:change={toggleContrast} />
                <span class="switch-slider"></span>
              </label>
            </div>
          </div>

          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('Reduce motion')}</span>
              <span class="pref-desc">{t('Minimize window transitions, dock scaling, and background animations')}</span>
            </div>
            <div class="pref-control">
              <label class="switch-toggle" aria-label={t('Reduce motion')}>
                <input type="checkbox" checked={reduced} on:change={toggleReduced} />
                <span class="switch-slider"></span>
              </label>
            </div>
          </div>

          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('Notifications in this app')}</span>
              <span class="pref-desc">{t('Show transfer progress banners, backup completion toasts, and alerts')}</span>
            </div>
            <div class="pref-control">
              <label class="switch-toggle" aria-label={t('Notifications in this app')}>
                <input type="checkbox" checked={notifications} on:change={toggleNotifications} />
                <span class="switch-slider"></span>
              </label>
            </div>
          </div>
        </section>

      <!-- 6. SYSTEM UPDATES -->
      {:else if activeSection === 'updates'}
        <div class="hero-appliance-card">
          <div class="hero-symbol-circle update-circle">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
            </svg>
          </div>
          <div class="hero-content">
            <div class="hero-badge-row">
              <span class="hero-badge">{t('System Updates')}</span>
              <span class="status-pill verified">
                <span class="dot"></span>
                slot_a
              </span>
            </div>
            <h3 class="hero-title">{t('Current OS Version')}: 0.1.0</h3>
            <p class="hero-text">{t('Atomic A/B partition updates with Ed25519 cryptographic rollback protection.')}</p>
          </div>
          <div class="hero-action-slot">
            <button type="button" class="primary-btn" disabled={busy} on:click={() => run(checkUpdates)}>
              {t('Check for Updates')}
            </button>
          </div>
        </div>

        {#if updateCheck}
          <div class="update-status-card" class:update-avail={updateCheck.available} role="status">
            <div class="update-status-icon" aria-hidden="true">
              {#if updateCheck.available}
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
              {:else}
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="2.5"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
              {/if}
            </div>
            <div class="update-status-body">
              <strong>{updateCheck.available ? t('Update available') : t('System Status')}</strong>
              <p>
                {updateCheck.available
                  ? `${t('Update available')}: ${updateCheck.latest_version}`
                  : updateCheck.reason === 'updates_disabled'
                  ? t('Turn on signed update checks and save settings first.')
                  : updateCheck.reason === 'channel_not_configured'
                  ? t('Set the update URL and public key, then save settings.')
                  : updateCheck.reason
                  ? t('Update check failed.')
                  : t('Up to date.')}
              </p>
            </div>
          </div>
        {/if}

        <form on:submit|preventDefault={handleSaveSettings}>
          <section class="pref-card" aria-labelledby="signed-updates-heading">
            <div class="pref-card-header">
              <h4 id="signed-updates-heading">{t('Signed Updates')}</h4>
              <p class="pref-card-subtitle">{t('Configure signed release manifest and public verification key')}</p>
            </div>
            <div class="pref-row">
              <div class="pref-meta">
                <span class="pref-title">{t('Check for signed updates')}</span>
                <span class="pref-desc">{t('Verify signed releases from your configured manifest source')}</span>
              </div>
              <div class="pref-control">
                <label class="switch-toggle" aria-label={t('Check for signed updates')}>
                  <input type="checkbox" bind:checked={settings.update_enabled} />
                  <span class="switch-slider"></span>
                </label>
              </div>
            </div>

            {#if settings.update_enabled}
              <div class="pref-row">
                <div class="pref-meta">
                  <label for="update-manifest-url" class="pref-title">{t('Update manifest URL')}</label>
                  <span class="pref-desc">{t('Remote HTTPS endpoint containing signed release digests')}</span>
                </div>
                <div class="pref-control">
                  <input
                    id="update-manifest-url"
                    type="url"
                    class="modern-input wide-input"
                    bind:value={settings.update_manifest_url}
                    placeholder="https://updates.homehub.local/manifest.json"
                  />
                </div>
              </div>

              <div class="pref-row">
                <div class="pref-meta">
                  <label for="update-pubkey-hex" class="pref-title">{t('Update public key')}</label>
                  <span class="pref-desc">{t('Ed25519 verification public key (64 hex characters)')}</span>
                </div>
                <div class="pref-control">
                  <input
                    id="update-pubkey-hex"
                    maxlength="64"
                    class="modern-input wide-input mono-font"
                    bind:value={settings.update_pubkey_hex}
                    placeholder="64-character Ed25519 public key hex"
                  />
                </div>
              </div>
            {/if}
          </section>

          <div class="form-actions-row">
            <button type="submit" class="primary-btn" disabled={busy}>
              {t('Save Changes')}
            </button>
          </div>
        </form>

      <!-- 7. DIAGNOSTICS & DATA -->
      {:else if activeSection === 'diagnostics'}
        <section class="pref-card" aria-labelledby="diag-export-heading">
          <div class="pref-card-header">
            <h4 id="diag-export-heading">{t('Diagnostics & Data')}</h4>
            <p class="pref-card-subtitle">{t('Diagnostics include system status and omit file names and contents.')}</p>
          </div>
          <div class="pref-row">
            <div class="pref-meta">
              <span class="pref-title">{t('Export diagnostics')}</span>
              <span class="pref-desc">{t('Download redacted system report with service health, partition status, and network topology')}</span>
            </div>
            <div class="pref-control">
              <button type="button" class="secondary-btn" disabled={busy} on:click={() => run(diagnostics)}>
                {t('Export diagnostics')}
              </button>
            </div>
          </div>

          {#if native}
            <div class="pref-row">
              <div class="pref-meta">
                <span class="pref-title">{t('Export private apps')}</span>
                <span class="pref-desc">{t('Export notes, playlists, and bookmarks into an encrypted portable backup file')}</span>
              </div>
              <div class="pref-control">
                <button type="button" class="secondary-btn" disabled={busy} on:click={handleExportPrivate}>
                  {t('Export private apps')}
                </button>
              </div>
            </div>

            <div class="pref-row">
              <div class="pref-meta">
                <span class="pref-title">{t('Import private apps')}</span>
                <span class="pref-desc">{t('Restore notes, playlists, and bookmarks from a previous private app backup')}</span>
              </div>
              <div class="pref-control">
                <button type="button" class="secondary-btn" disabled={busy} on:click={handleImportPrivate}>
                  {t('Import private apps')}
                </button>
              </div>
            </div>
          {/if}
        </section>

        <section class="pref-card" aria-labelledby="privacy-policy-heading">
          <div class="pref-card-header">
            <h4 id="privacy-policy-heading">{t('Privacy')}</h4>
            <p class="pref-card-subtitle">{t('No telemetry is sent by default.')}</p>
          </div>
          <div class="pref-card-body">
            <div class="security-guidance-list">
              <div class="guidance-item">
                <span class="guidance-dot green"></span>
                <span>{t('No telemetry is sent by default.')}</span>
              </div>
              <div class="guidance-item">
                <span class="guidance-dot blue"></span>
                <span>{t('Diagnostics include system status and omit file names and contents.')}</span>
              </div>
              <div class="guidance-item">
                <span class="guidance-dot purple"></span>
                <span>{t('Turn on BitLocker in Windows to protect data at rest.')}</span>
              </div>
            </div>
          </div>
        </section>

      <!-- 8. PRIVATE TRASH -->
      {:else if activeSection === 'trash'}
        {#if !native}
          <section class="pref-card">
            <div class="pref-card-body empty-box">
              <p>{t('Private apps are available in the installed Windows app.')}</p>
            </div>
          </section>
        {:else}
          <section class="pref-card" aria-labelledby="trash-heading">
            <div class="pref-card-header">
              <div class="header-with-badge">
                <h4 id="trash-heading">{t('Private app trash')}</h4>
                {#if privateTrash?.length > 0}
                  <span class="item-count-pill">{privateTrash.length}</span>
                {/if}
              </div>
              <p class="pref-card-subtitle">{t('Deleted Notes & Playlists')}</p>
            </div>
            <div class="trash-list-wrap">
              {#each privateTrash || [] as row (row.id)}
                <div class="trash-item-row">
                  <div class="trash-item-meta">
                    <span class="kind-tag-badge">{t(row.kind)}</span>
                    <span class="trash-item-title">{row.title}</span>
                  </div>
                  <div class="trash-item-actions">
                    <button type="button" class="secondary-btn" on:click={() => handleRestoreTrash(row)}>
                      {t('Restore')}
                    </button>
                    <button type="button" class="danger-btn" on:click={() => handlePurgeTrash(row)}>
                      {t('Delete permanently')}
                    </button>
                  </div>
                </div>
              {:else}
                <div class="empty-trash-box">
                  <div class="empty-icon-circle">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                    </svg>
                  </div>
                  <p class="empty-title">{t('Nothing here yet.')}</p>
                  <p class="empty-subtitle">{t('Deleted notes, playlists, and bookmarks will appear here for safe recovery.')}</p>
                </div>
              {/each}
            </div>
          </section>
        {/if}

      <!-- 9. NETWORK -->
      {:else if activeSection === 'network'}
        <section class="pref-card" aria-labelledby="network-heading">
          <div class="pref-card-header">
            <div class="header-with-badge">
              <h4 id="network-heading">{t('Network')}</h4>
              <span class="status-pill active">
                <span class="dot"></span>
                {t('Connected locally')}
              </span>
            </div>
            <p class="pref-card-subtitle">{t('Active interface sockets, mTLS endpoints, and addresses')}</p>
          </div>
          <div class="pref-card-body">
            {#if network}
              <Details value={network} {t}/>
            {:else}
              <p class="empty-hint">{t('Nothing here yet.')}</p>
            {/if}
          </div>
        </section>
      {/if}
    </div>
  </main>
</div>

<style>
  /* Root Layout Container */
  .settings-hub {
    display: flex;
    height: 100%;
    min-height: 0;
    width: 100%;
    background: #141715;
    color: #f5f5f7;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    overflow: hidden;
  }

  /* Left Sidebar Navigation (Width 240px) */
  .settings-sidebar {
    width: 240px;
    flex-shrink: 0;
    background: #161917;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sidebar-top {
    padding: 18px 14px 12px;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .sidebar-title-bar {
    margin-bottom: 12px;
    padding: 0 4px;
  }

  .sidebar-heading {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: -0.02em;
  }

  .search-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    color: #8e8e93;
    pointer-events: none;
  }

  .sidebar-search-input {
    width: 100%;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #f5f5f7;
    border-radius: 8px;
    padding: 7px 28px 7px 32px;
    font-size: 0.82rem;
    transition: all 0.16s ease;
    outline: none;
    box-sizing: border-box;
  }

  .sidebar-search-input:focus {
    background: rgba(255, 255, 255, 0.09);
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.25);
  }

  .sidebar-search-input::placeholder {
    color: #8e8e93;
  }

  .search-clear-btn {
    position: absolute;
    right: 8px;
    background: none;
    border: none;
    color: #8e8e93;
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 2px 4px;
  }

  .search-clear-btn:hover {
    color: #ffffff;
  }

  .sidebar-nav {
    flex: 1;
    overflow-y: auto;
    padding: 10px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.12) transparent;
  }

  .nav-tab-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: #a1a1a6;
    font-size: 0.84rem;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
    outline: none;
    position: relative;
    user-select: none;
  }

  .nav-tab-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #ffffff;
  }

  .nav-tab-btn:focus-visible {
    outline: 2px solid #0071e3;
    outline-offset: 1px;
  }

  /* Active Navigation State with Frosted Pill & Accent Glow */
  .nav-tab-btn.active {
    background: rgba(0, 113, 227, 0.18);
    border-color: rgba(0, 113, 227, 0.38);
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 10px rgba(0, 113, 227, 0.25);
  }

  .nav-tab-btn.active .nav-icon-slot {
    color: #2997ff;
  }

  .nav-icon-slot {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    color: #8e8e93;
    transition: color 0.15s ease;
  }

  .nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nav-count-badge {
    background: rgba(255, 255, 255, 0.12);
    color: #e5e5ea;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
  }

  .nav-dot-badge {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #f59e0b;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.6);
  }

  .search-empty-state {
    padding: 20px 10px;
    text-align: center;
    color: #8e8e93;
    font-size: 0.8rem;
  }

  /* Right Detail Content Panel (Flex 1, Full Bleed Scrollable) */
  .settings-content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 26px 36px 40px;
    background: #141715;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.16) transparent;
  }

  .content-header {
    margin-bottom: 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    padding-bottom: 18px;
  }

  .active-section-title {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: -0.025em;
  }

  .active-section-desc {
    margin: 6px 0 0;
    font-size: 0.86rem;
    color: #8e8e93;
    line-height: 1.4;
  }

  .section-body {
    max-width: 820px;
  }

  /* Hero Appliance Card */
  .hero-appliance-card {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 20px 24px;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.05) 0%, rgba(255, 255, 255, 0.02) 100%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    margin-bottom: 22px;
  }

  .hero-symbol-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    background: rgba(0, 113, 227, 0.2);
    border: 1px solid rgba(0, 113, 227, 0.35);
    color: #2997ff;
    flex-shrink: 0;
  }

  .hero-symbol-circle.update-circle {
    background: rgba(52, 199, 89, 0.18);
    border-color: rgba(52, 199, 89, 0.32);
    color: #34c759;
  }

  .hero-content {
    flex: 1;
    min-width: 0;
  }

  .hero-badge-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
    flex-wrap: wrap;
  }

  .hero-badge {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #2997ff;
  }

  .hero-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: #ffffff;
  }

  .hero-text {
    margin: 4px 0 0;
    font-size: 0.82rem;
    color: #8e8e93;
    line-height: 1.4;
  }

  .hero-action-slot {
    flex-shrink: 0;
  }

  /* Preference Cards (Apple / Windows 11 Fluent) */
  .pref-card {
    background: #181c19;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    margin-bottom: 20px;
    overflow: hidden;
  }

  .pref-card-header {
    padding: 16px 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.015);
  }

  .pref-card-header h4 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 650;
    color: #ffffff;
  }

  .pref-card-subtitle {
    margin: 4px 0 0;
    font-size: 0.78rem;
    color: #8e8e93;
  }

  .pref-card-body {
    padding: 18px 20px;
  }

  .header-with-badge {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .item-count-pill {
    background: rgba(255, 255, 255, 0.1);
    color: #f5f5f7;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
  }

  /* Preference Rows */
  .pref-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    gap: 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .pref-row:last-child {
    border-bottom: none;
  }

  .pref-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .pref-title {
    font-size: 0.88rem;
    font-weight: 600;
    color: #f5f5f7;
  }

  .pref-desc {
    font-size: 0.78rem;
    color: #8e8e93;
    line-height: 1.35;
  }

  .pref-title-with-badge {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .pref-control {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .pref-control-group {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  /* Interactive Controls */
  .modern-input {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: #ffffff;
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 0.84rem;
    min-width: 220px;
    outline: none;
    transition: border-color 0.16s ease, box-shadow 0.16s ease;
    box-sizing: border-box;
  }

  .modern-input.wide-input {
    min-width: 320px;
  }

  .modern-input.mono-font {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.78rem;
  }

  .modern-input:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.25);
  }

  .stepper-input {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: #ffffff;
    border-radius: 8px 0 0 8px;
    padding: 7px 10px;
    font-size: 0.84rem;
    width: 80px;
    outline: none;
    text-align: right;
    box-sizing: border-box;
  }

  .stepper-input:focus {
    border-color: #0071e3;
  }

  .input-with-unit {
    display: flex;
    align-items: center;
  }

  .unit-label {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-left: none;
    color: #8e8e93;
    padding: 7px 10px;
    border-radius: 0 8px 8px 0;
    font-size: 0.8rem;
    font-weight: 500;
  }

  .modern-time-input {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: #ffffff;
    border-radius: 8px;
    padding: 7px 12px;
    font-size: 0.84rem;
    outline: none;
    box-sizing: border-box;
  }

  .modern-time-input:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.25);
  }

  /* Buttons */
  .primary-btn {
    background: #0071e3;
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #ffffff;
    border-radius: 8px;
    padding: 7px 16px;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
    box-shadow: 0 1px 4px rgba(0, 113, 227, 0.35);
  }

  .primary-btn:hover:not(:disabled) {
    background: #0077ed;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.45);
  }

  .secondary-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
    border-radius: 8px;
    padding: 7px 15px;
    font-size: 0.82rem;
    font-weight: 550;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .secondary-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
    color: #ffffff;
  }

  .danger-btn {
    background: rgba(239, 68, 68, 0.14);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #f87171;
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .danger-btn:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.24);
    color: #fca5a5;
  }

  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }

  .form-actions-row {
    display: flex;
    justify-content: flex-end;
    margin-top: 10px;
    margin-bottom: 20px;
  }

  /* Apple-Style Animated Toggle Switch */
  .switch-toggle {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .switch-toggle input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .switch-slider {
    position: absolute;
    inset: 0;
    background-color: rgba(255, 255, 255, 0.16);
    border-radius: 24px;
    transition: all 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .switch-slider::before {
    position: absolute;
    content: "";
    height: 18px;
    width: 18px;
    left: 2px;
    bottom: 2px;
    background-color: #ffffff;
    border-radius: 50%;
    transition: transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
  }

  .switch-toggle input:checked + .switch-slider {
    background-color: #0071e3;
    border-color: #0071e3;
  }

  .switch-toggle input:checked + .switch-slider::before {
    transform: translateX(20px);
  }

  .switch-toggle input:focus-visible + .switch-slider {
    outline: 2px solid #0071e3;
    outline-offset: 2px;
  }

  /* Status Pills & Badges */
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 550;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #d1d5db;
  }

  .status-pill .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #8e8e93;
  }

  .status-pill.active {
    background: rgba(52, 199, 89, 0.12);
    border-color: rgba(52, 199, 89, 0.3);
    color: #4ade80;
  }

  .status-pill.active .dot {
    background: #34c759;
    box-shadow: 0 0 6px rgba(52, 199, 89, 0.6);
  }

  .status-pill.paused {
    background: rgba(245, 158, 11, 0.12);
    border-color: rgba(245, 158, 11, 0.3);
    color: #fbbf24;
  }

  .status-pill.paused .dot {
    background: #f59e0b;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.6);
  }

  .status-pill.warning {
    background: rgba(245, 158, 11, 0.1);
    border-color: rgba(245, 158, 11, 0.25);
    color: #f59e0b;
  }

  .status-pill.verified {
    background: rgba(0, 113, 227, 0.12);
    border-color: rgba(0, 113, 227, 0.3);
    color: #60a5fa;
  }

  .path-badge {
    display: inline-block;
    padding: 6px 12px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.78rem;
    color: #93c5fd;
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Language Switcher Grid */
  .lang-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
  }

  .lang-choice-card {
    display: flex;
    flex-direction: column;
    padding: 16px 18px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1.5px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    text-align: left;
    transition: all 0.18s ease;
    outline: none;
    color: inherit;
  }

  .lang-choice-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.16);
  }

  .lang-choice-card.selected {
    background: rgba(0, 113, 227, 0.12);
    border-color: #0071e3;
    box-shadow: 0 2px 12px rgba(0, 113, 227, 0.2);
  }

  .lang-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .lang-symbol {
    font-size: 1.5rem;
    line-height: 1;
  }

  .check-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.2);
    color: #ffffff;
    transition: all 0.15s ease;
  }

  .check-circle.checked {
    background: #0071e3;
    border-color: #0071e3;
  }

  .lang-card-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .lang-native-title {
    font-size: 1rem;
    font-weight: 650;
    color: #ffffff;
  }

  .lang-english-sub {
    font-size: 0.78rem;
    color: #8e8e93;
  }

  /* Updates Status Card */
  .update-status-card {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 16px 20px;
    background: rgba(16, 185, 129, 0.08);
    border: 1px solid rgba(16, 185, 129, 0.2);
    border-radius: 12px;
    margin-bottom: 20px;
  }

  .update-status-card.update-avail {
    background: rgba(245, 158, 11, 0.08);
    border-color: rgba(245, 158, 11, 0.24);
  }

  .update-status-icon {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .update-status-body strong {
    display: block;
    font-size: 0.9rem;
    color: #ffffff;
    margin-bottom: 4px;
  }

  .update-status-body p {
    margin: 0;
    font-size: 0.82rem;
    color: #d1d5db;
    line-height: 1.4;
  }

  /* Security Guidance */
  .security-guidance-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .guidance-item {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 0.82rem;
    color: #d1d5db;
  }

  .guidance-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .guidance-dot.green {
    background: #34c759;
  }

  .guidance-dot.blue {
    background: #0071e3;
  }

  .guidance-dot.purple {
    background: #af52de;
  }

  /* Trash List */
  .trash-list-wrap {
    display: flex;
    flex-direction: column;
  }

  .trash-item-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    gap: 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .trash-item-row:last-child {
    border-bottom: none;
  }

  .trash-item-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .kind-tag-badge {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #9ca3af;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 6px;
    text-transform: capitalize;
    flex-shrink: 0;
  }

  .trash-item-title {
    font-size: 0.86rem;
    font-weight: 550;
    color: #f5f5f7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .trash-item-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .empty-trash-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 20px;
    text-align: center;
  }

  .empty-icon-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.05);
    color: #8e8e93;
    margin-bottom: 12px;
  }

  .empty-title {
    font-size: 0.95rem;
    font-weight: 600;
    color: #ffffff;
    margin: 0 0 4px;
  }

  .empty-subtitle {
    font-size: 0.8rem;
    color: #8e8e93;
    margin: 0;
    max-width: 320px;
  }

  .alert-banner {
    padding: 10px 14px;
    border-radius: 8px;
    margin-bottom: 14px;
    font-size: 0.82rem;
  }

  .alert-banner.error {
    background: rgba(239, 68, 68, 0.14);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #f87171;
  }

  .empty-hint {
    color: #8e8e93;
    font-size: 0.82rem;
    margin: 0;
  }

  .empty-box {
    text-align: center;
    color: #8e8e93;
    font-size: 0.84rem;
  }

  /* High Contrast Theme Overrides */
  .contrast-theme .pref-card,
  .contrast-theme .hero-appliance-card,
  .contrast-theme .settings-sidebar {
    border-color: rgba(255, 255, 255, 0.35);
  }

  .contrast-theme .pref-row,
  .contrast-theme .trash-item-row,
  .contrast-theme .pref-card-header {
    border-bottom-color: rgba(255, 255, 255, 0.25);
  }

  .contrast-theme .modern-input,
  .contrast-theme .sidebar-search-input {
    border-color: rgba(255, 255, 255, 0.4);
  }

  .contrast-theme .nav-tab-btn.active {
    border-color: #ffffff;
    background: #0071e3;
  }

  /* Responsive Adjustments */
  @media (max-width: 768px) {
    .settings-hub {
      flex-direction: column;
    }
    .settings-sidebar {
      width: 100%;
      height: auto;
      border-right: none;
      border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }
    .sidebar-nav {
      flex-direction: row;
      overflow-x: auto;
      padding: 8px;
    }
    .nav-tab-btn {
      width: auto;
      white-space: nowrap;
    }
    .settings-content {
      padding: 20px 16px;
    }
    .pref-row {
      flex-direction: column;
      align-items: flex-start;
      gap: 12px;
    }
    .pref-control,
    .pref-control-group {
      width: 100%;
      justify-content: flex-start;
    }
    .modern-input,
    .modern-input.wide-input {
      width: 100%;
      min-width: 0;
    }
  }
</style>
