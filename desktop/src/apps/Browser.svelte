<script>
  import { native, command, json } from '../api.js';

  export let address = '';
  export let tabs = [];
  export let previousTabs = [];
  export let bookmarks = [];
  export let downloads = [];
  export let t = (k) => k;
  export let run = (fn) => fn?.();
  export let load = async () => {};
  export let confirm = (msg, fn, label) => fn?.();
  export let website = () => {};
  export let restoreTabs = () => {};
  export let browserAction = () => {};
  export let bookmark = () => {};
  export let permissions = () => {};

  let searchQuery = '';
  let activeTabFilter = 'shortcuts'; // 'shortcuts' | 'bookmarks' | 'downloads'

  $: safeTabs = Array.isArray(tabs) ? tabs : [];
  $: safePreviousTabs = Array.isArray(previousTabs) ? previousTabs : [];
  $: safeBookmarks = Array.isArray(bookmarks) ? bookmarks : [];
  $: safeDownloads = Array.isArray(downloads) ? downloads : [];

  $: if (address && !searchQuery) {
    searchQuery = address;
  }

  const speedDial = [
    {
      id: 'google',
      title: 'Google Search',
      subtitle: 'The world’s information',
      url: 'https://www.google.com',
      badge: 'Search',
      color: '#4285F4',
      icon: 'search'
    },
    {
      id: 'wikipedia',
      title: 'Wikipedia',
      subtitle: 'Free encyclopedia & research',
      url: 'https://www.wikipedia.org',
      badge: 'Reference',
      color: '#EA4335',
      icon: 'book'
    },
    {
      id: 'github',
      title: 'GitHub',
      subtitle: 'Code, repositories & developer tools',
      url: 'https://github.com',
      badge: 'Dev',
      color: '#A855F7',
      icon: 'code'
    },
    {
      id: 'news',
      title: 'Google News',
      subtitle: 'Verified global headlines & reports',
      url: 'https://news.google.com',
      badge: 'News',
      color: '#34A853',
      icon: 'news'
    },
    {
      id: 'maps',
      title: 'Google Maps',
      subtitle: 'Satellite navigation & transit',
      url: 'https://maps.google.com',
      badge: 'Maps',
      color: '#FBBC05',
      icon: 'map'
    },
    {
      id: 'reddit',
      title: 'Reddit',
      subtitle: 'Trending discussions & communities',
      url: 'https://www.reddit.com',
      badge: 'Social',
      color: '#FF4500',
      icon: 'chat'
    }
  ];

  const searchPills = [
    { label: 'Google AI', query: 'Google Gemini AI' },
    { label: 'World News', query: 'World breaking news' },
    { label: 'GitHub Trending', query: 'GitHub trending repositories' },
    { label: 'Weather Forecast', query: 'Local weather radar' },
    { label: 'Wikipedia Today', query: 'Wikipedia featured articles' }
  ];

  function normalizeUrl(input) {
    if (!input) return 'https://www.google.com';
    const trimmed = input.trim();
    if (/^https?:\/\//i.test(trimmed)) {
      return trimmed;
    }
    if (/^[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+/i.test(trimmed) && !trimmed.includes(' ')) {
      return `https://${trimmed}`;
    }
    return `https://www.google.com/search?q=${encodeURIComponent(trimmed)}`;
  }

  function handleSearch(inSandbox = false) {
    const raw = searchQuery.trim();
    if (!raw) {
      if (inSandbox) {
        run(() => website('https://www.google.com', false));
      } else {
        run(() => command('open_external', { url: 'https://www.google.com' }));
      }
      return;
    }
    const target = normalizeUrl(raw);
    address = target;
    if (inSandbox) {
      run(() => website(target, false));
    } else {
      run(() => command('open_external', { url: target }));
    }
  }

  function launchShortcut(url, inSandbox = false) {
    address = url;
    if (inSandbox) {
      run(() => website(url, false));
    } else {
      run(() => command('open_external', { url }));
    }
  }

  function extractBookmarkUrl(row) {
    try {
      const data = json(row?.body);
      return data?.url || '';
    } catch {
      return '';
    }
  }
</script>

<div class="browser-app" role="region" aria-label={t('Browser')}>
  <div class="browser-container">
    <!-- Hero Header Banner -->
    <header class="browser-hero">
      <div class="hero-brand">
        <div class="brand-badge" aria-hidden="true">
          <svg viewBox="0 0 48 48" width="34" height="34" class="chrome-ring-icon">
            <defs>
              <linearGradient id="chrome-red" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#EA4335"/>
                <stop offset="100%" stop-color="#D93025"/>
              </linearGradient>
              <linearGradient id="chrome-green" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#34A853"/>
                <stop offset="100%" stop-color="#1E8E3E"/>
              </linearGradient>
              <linearGradient id="chrome-yellow" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#FBBC05"/>
                <stop offset="100%" stop-color="#F29900"/>
              </linearGradient>
              <linearGradient id="chrome-blue" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#4285F4"/>
                <stop offset="100%" stop-color="#1A73E8"/>
              </linearGradient>
            </defs>
            <circle cx="24" cy="24" r="21" fill="none" stroke="rgba(255,255,255,0.08)" stroke-width="2"/>
            <!-- Top Red Segment -->
            <path d="M24 13 A11 11 0 0 1 33.5 18.5 L43.5 18.5 A22 22 0 0 0 13 4.5 L18.5 14 A11 11 0 0 1 24 13 Z" fill="url(#chrome-red)"/>
            <!-- Right Green Segment -->
            <path d="M33.5 18.5 A11 11 0 0 1 29.5 32.5 L34.5 41 A22 22 0 0 0 45 22.5 L33.5 22.5 A11 11 0 0 1 33.5 18.5 Z" fill="url(#chrome-green)"/>
            <!-- Left Yellow Segment -->
            <path d="M29.5 32.5 A11 11 0 0 1 14.5 28 L9.5 37 A22 22 0 0 0 31.5 44 L27 34.5 A11 11 0 0 1 29.5 32.5 Z" fill="url(#chrome-yellow)"/>
            <!-- Center Blue Hub -->
            <circle cx="24" cy="24" r="9" fill="url(#chrome-blue)"/>
            <circle cx="24" cy="24" r="5" fill="#FFFFFF" opacity="0.95"/>
          </svg>
        </div>
        <div class="brand-text">
          <div class="brand-title-row">
            <h1 class="brand-title">{t('Web Browser')}</h1>
            <span class="chrome-pill">{t('CHROME / SANDBOX')}</span>
          </div>
          <p class="brand-subtitle">{t('Fast, secure browsing via Google Chrome or private isolated sandbox')}</p>
        </div>
      </div>

      <div class="hero-actions">
        <button
          type="button"
          class="btn-chrome-launch"
          on:click={() => run(() => command('open_external', { url: address || 'https://www.google.com' }))}
          title={t('Open in Google Chrome')}
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/>
          </svg>
          <span>{t('Open Google Chrome')}</span>
        </button>

        <button
          type="button"
          class="btn-sandbox-launch"
          on:click={() => run(() => website(address || 'https://www.google.com', false))}
          disabled={!native}
          title={t('Open in isolated sandbox window')}
        >
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
          <span>{t('Open in Isolated Sandbox')}</span>
        </button>
      </div>
    </header>

    {#if !native}
      <div class="notice-bar">
        <span>{t('Private apps and sandbox windows are available in the installed desktop application.')}</span>
      </div>
    {/if}

    <!-- Search & Navigation Hub -->
    <section class="search-section" aria-label={t('Search or enter address')}>
      <form class="search-bar" on:submit|preventDefault={() => handleSearch(false)}>
        <div class="search-icon-wrapper" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="#4285F4" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
        </div>
        <input
          type="text"
          class="search-input"
          bind:value={searchQuery}
          placeholder={t('Search Google or enter any URL… (e.g. github.com or world news)')}
          autocomplete="off"
          spellcheck="false"
        />
        <div class="search-bar-actions">
          <button
            type="submit"
            class="search-submit-btn"
            title={t('Search via Google Chrome')}
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3"/>
            </svg>
            <span>{t('Search in Chrome')}</span>
          </button>
          <button
            type="button"
            class="search-sandbox-btn"
            on:click={() => handleSearch(true)}
            disabled={!native}
            title={t('Open query in private sandbox')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            </svg>
            <span>{t('Sandbox')}</span>
          </button>
        </div>
      </form>

      <!-- Search Suggestions / Topic Pills -->
      <div class="search-pills-row" aria-label={t('Trending search topics')}>
        <span class="pills-label">{t('Quick Searches:')}</span>
        {#each searchPills as pill}
          <button
            type="button"
            class="search-pill"
            on:click={() => { searchQuery = pill.query; handleSearch(false); }}
          >
            {pill.label}
          </button>
        {/each}
      </div>
    </section>

    <!-- Restore Previous Session Banner (if tabs closed) -->
    {#if safePreviousTabs.length > 0 && safeTabs.length === 0}
      <div class="restore-bar">
        <div class="restore-text">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
            <path d="M3 3v5h5"/>
          </svg>
          <span>{t('Previous browsing session available from last run.')}</span>
        </div>
        <button type="button" class="restore-btn" on:click={() => run(restoreTabs)}>
          {t('Restore previous tabs')} ({safePreviousTabs.length})
        </button>
      </div>
    {/if}

    <!-- Active Sandbox Sessions (Live Tabs Controller) -->
    {#if safeTabs.length > 0}
      <section class="active-sessions-card" aria-label={t('Active Sandbox Tabs')}>
        <div class="section-card-header">
          <div class="beacon-title-row">
            <div class="beacon-pulse" aria-hidden="true">
              <span class="beacon-dot"></span>
              <span class="beacon-ring"></span>
            </div>
            <h2>{t('Active Sandbox Windows')}</h2>
            <span class="count-badge">{safeTabs.length}</span>
          </div>
          <span class="header-sublabel">{t('Isolated Webview Profiles')}</span>
        </div>

        <div class="tabs-list">
          {#each safeTabs as tab}
            <article class="tab-item-card">
              <div class="tab-item-info">
                <div class="tab-title-row">
                  <strong class="tab-title">{tab.title || t('Untitled Web Page')}</strong>
                  {#if tab.isolated}
                    <span class="pill-badge-isolated">{t('Isolated')}</span>
                  {/if}
                </div>
                <small class="tab-url-text">{tab.url}</small>
              </div>

              <div class="tab-controls-row">
                <button
                  type="button"
                  class="tab-btn highlight"
                  on:click={() => run(() => browserAction(tab, 'focus'))}
                  title={t('Focus window')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="10"></circle>
                    <circle cx="12" cy="12" r="3"></circle>
                  </svg>
                  <span>{t('Focus')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => run(() => browserAction(tab, 'reload'))}
                  title={t('Reload web page')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M23 4v6h-6M1 20v-6h6"/>
                    <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                  </svg>
                  <span>{t('Reload')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => run(() => browserAction(tab, 'back'))}
                  title={t('Back')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M19 12H5M12 19l-7-7 7-7"/>
                  </svg>
                  <span>{t('Back')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => run(() => browserAction(tab, 'forward'))}
                  title={t('Forward')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 12h14M12 5l7 7-7 7"/>
                  </svg>
                  <span>{t('Next')}</span>
                </button>
                {#if tab.isolated}
                  <button
                    type="button"
                    class="tab-btn"
                    on:click={() => run(() => browserAction(tab, 'fullscreen'))}
                    title={t('Fullscreen')}
                  >
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/>
                    </svg>
                    <span>{t('Fullscreen')}</span>
                  </button>
                {/if}
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => run(() => command('open_external', { url: tab.url }))}
                  title={t('Open this tab in Google Chrome')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3"/>
                  </svg>
                  <span>{t('Chrome')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => run(() => bookmark(tab))}
                  title={t('Save bookmark')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
                  </svg>
                  <span>{t('Bookmark')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn"
                  on:click={() => permissions(tab)}
                  title={t('Manage permissions')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="3"/>
                    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
                  </svg>
                  <span>{t('Permissions')}</span>
                </button>
                <button
                  type="button"
                  class="tab-btn close"
                  on:click={() => run(() => browserAction(tab, 'close'))}
                  title={t('Close window')}
                >
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="18" y1="6" x2="6" y2="18"></line>
                    <line x1="6" y1="6" x2="18" y2="18"></line>
                  </svg>
                  <span>{t('Close')}</span>
                </button>
              </div>
            </article>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Speed-Dial Shortcuts Grid -->
    <section class="shortcuts-section" aria-label={t('Speed Dial Shortcuts')}>
      <div class="section-title-row">
        <div class="section-title-left">
          <h2>{t('Speed Dial & Destinations')}</h2>
          <span class="subtext">{t('One-click launch directly in Google Chrome')}</span>
        </div>
        <div class="tabs-segmented-toggle">
          <button
            type="button"
            class="toggle-btn"
            class:active={activeTabFilter === 'shortcuts'}
            on:click={() => activeTabFilter = 'shortcuts'}
          >
            {t('Shortcuts')}
          </button>
          <button
            type="button"
            class="toggle-btn"
            class:active={activeTabFilter === 'bookmarks'}
            on:click={() => activeTabFilter = 'bookmarks'}
          >
            {t('Bookmarks')} ({safeBookmarks.length})
          </button>
          <button
            type="button"
            class="toggle-btn"
            class:active={activeTabFilter === 'downloads'}
            on:click={() => activeTabFilter = 'downloads'}
          >
            {t('Downloads')} ({safeDownloads.length})
          </button>
        </div>
      </div>

      {#if activeTabFilter === 'shortcuts'}
        <div class="shortcuts-grid">
          {#each speedDial as item}
            <div class="shortcut-card" on:click={() => launchShortcut(item.url, false)} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && launchShortcut(item.url, false)}>
              <div class="shortcut-header">
                <div class="shortcut-icon-squircle" style="background: {item.color}1A; border-color: {item.color}33;">
                  {#if item.icon === 'search'}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <circle cx="11" cy="11" r="8"></circle>
                      <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                    </svg>
                  {:else if item.icon === 'book'}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"></path>
                      <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"></path>
                    </svg>
                  {:else if item.icon === 'code'}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <polyline points="16 18 22 12 16 6"></polyline>
                      <polyline points="8 6 2 12 8 18"></polyline>
                    </svg>
                  {:else if item.icon === 'news'}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2Zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"></path>
                      <path d="M18 14h-8m8-4h-8m4-4h-4"></path>
                    </svg>
                  {:else if item.icon === 'map'}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <polygon points="1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6"></polygon>
                      <line x1="8" y1="2" x2="8" y2="18"></line>
                      <line x1="16" y1="6" x2="16" y2="22"></line>
                    </svg>
                  {:else}
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke={item.color} stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"></path>
                    </svg>
                  {/if}
                </div>
                <div class="shortcut-actions-inline">
                  <span class="category-chip" style="color: {item.color}; background: {item.color}15;">{item.badge}</span>
                  <button
                    type="button"
                    class="mini-sandbox-btn"
                    title={t('Open in isolated sandbox')}
                    on:click|stopPropagation={() => launchShortcut(item.url, true)}
                    disabled={!native}
                  >
                    <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
                    </svg>
                  </button>
                </div>
              </div>
              <div class="shortcut-body">
                <h3 class="shortcut-title">{item.title}</h3>
                <p class="shortcut-desc">{item.subtitle}</p>
                <span class="shortcut-url-subtext">{item.url.replace(/^https?:\/\//, '')}</span>
              </div>
            </div>
          {/each}
        </div>
      {:else if activeTabFilter === 'bookmarks'}
        <div class="bookmarks-container">
          {#if safeBookmarks.length > 0}
            <div class="bookmarks-grid">
              {#each safeBookmarks as row}
                {@const bUrl = extractBookmarkUrl(row)}
                <div class="bookmark-item-card">
                  <div class="bookmark-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="#4285F4" stroke-width="2">
                      <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
                    </svg>
                  </div>
                  <div class="bookmark-info">
                    <strong class="bookmark-title">{row.title || t('Saved Bookmark')}</strong>
                    <small class="bookmark-url">{bUrl || '—'}</small>
                  </div>
                  <div class="bookmark-actions">
                    <button
                      type="button"
                      class="btn-sm primary"
                      on:click={() => run(() => command('open_external', { url: bUrl }))}
                      title={t('Open in Chrome')}
                    >
                      {t('Open')}
                    </button>
                    <button
                      type="button"
                      class="btn-sm ghost"
                      on:click={() => run(() => website(bUrl, false))}
                      disabled={!native}
                      title={t('Open in Sandbox')}
                    >
                      {t('Sandbox')}
                    </button>
                    <button
                      type="button"
                      class="btn-sm danger"
                      on:click={() => run(async () => { await command('local_trash', { id: row.id }); await load(); })}
                      title={t('Remove Bookmark')}
                    >
                      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                        <polyline points="3 6 5 6 21 6"></polyline>
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                      </svg>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty-state-box">
              <div class="empty-icon-ring">
                <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="#6E7681" stroke-width="1.8">
                  <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
                </svg>
              </div>
              <p class="empty-title">{t('No bookmarks saved yet')}</p>
              <p class="empty-desc">{t('Use the Bookmark button on active sandbox tabs to save web pages here.')}</p>
            </div>
          {/if}
        </div>
      {:else}
        <div class="downloads-container">
          {#if safeDownloads.length > 0}
            <div class="downloads-list">
              {#each safeDownloads as row}
                {@const body = json(row.body) || {}}
                <div class="download-item-card">
                  <div class="download-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="#34A853" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                      <polyline points="7 10 12 15 17 10"></polyline>
                      <line x1="12" y1="15" x2="12" y2="3"></line>
                    </svg>
                  </div>
                  <div class="download-info">
                    <strong class="download-title">{row.title}</strong>
                    <div class="download-meta">
                      <span class="status-pill" class:complete={body.status === 'complete'}>
                        {t(body.status || 'unknown')}
                      </span>
                    </div>
                  </div>
                  <div class="download-actions">
                    <button
                      type="button"
                      class="btn-sm primary"
                      disabled={body.status !== 'complete'}
                      on:click={() => run(() => command('save_download', { id: row.id }))}
                    >
                      {t('Save')}
                    </button>
                    <button
                      type="button"
                      class="btn-sm danger"
                      disabled={body.status === 'downloading'}
                      on:click={() => confirm(t('Remove'), async () => { await command('discard_download', { id: row.id }); await load(); }, row.title)}
                    >
                      {t('Remove')}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty-state-box">
              <div class="empty-icon-ring">
                <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="#6E7681" stroke-width="1.8">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                  <polyline points="7 10 12 15 17 10"></polyline>
                  <line x1="12" y1="15" x2="12" y2="3"></line>
                </svg>
              </div>
              <p class="empty-title">{t('No active or recent downloads')}</p>
              <p class="empty-desc">{t('Downloads initiated from sandbox sessions will be displayed here for verification and saving.')}</p>
            </div>
          {/if}
        </div>
      {/if}
    </section>

    <!-- Bottom Footer: Privacy Assurance & Data Controls -->
    <footer class="browser-footer">
      <div class="privacy-badge-group">
        <div class="shield-icon-badge" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="#34A853" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            <polyline points="9 12 11 14 15 10"/>
          </svg>
        </div>
        <p class="privacy-text">
          {t('Browsing runs in your native Google Chrome or isolated webview profile. External websites cannot access local Home Hub private storage.')}
        </p>
      </div>

      <div class="footer-actions">
        <button
          type="button"
          class="btn-clear-cache"
          on:click={() => confirm(t('Clear website data'), async () => { await command('browser_clear'); await load(); })}
          disabled={!native}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
          </svg>
          <span>{t('Clear website data')}</span>
        </button>
      </div>
    </footer>
  </div>
</div>

<style>
  .browser-app {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background-color: #0c0e14;
    background-image:
      radial-gradient(ellipse 65% 35% at 50% 0%, rgba(66, 133, 244, 0.08) 0%, transparent 75%),
      radial-gradient(ellipse 40% 25% at 85% 15%, rgba(52, 168, 83, 0.05) 0%, transparent 60%);
    color: #e6edf3;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.12) transparent;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  .browser-container {
    max-width: 1100px;
    width: 100%;
    margin: 0 auto;
    padding: 24px 28px 48px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    box-sizing: border-box;
  }

  /* ---------------- HERO HEADER ---------------- */
  .browser-hero {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    padding: 20px 24px;
    background: rgba(22, 27, 34, 0.7);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }

  .hero-brand {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .brand-badge {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    background: #161b22;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.4);
    flex-shrink: 0;
  }

  .chrome-ring-icon {
    display: block;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.4));
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .brand-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .brand-title {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: #ffffff;
  }

  .chrome-pill {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 2px 8px;
    border-radius: 20px;
    background: rgba(66, 133, 244, 0.15);
    color: #4285F4;
    border: 1px solid rgba(66, 133, 244, 0.3);
  }

  .brand-subtitle {
    margin: 0;
    font-size: 13px;
    color: #8b949e;
    line-height: 1.4;
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .btn-chrome-launch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 600;
    color: #ffffff;
    background: linear-gradient(135deg, #4285F4 0%, #1A73E8 100%);
    border: 1px solid rgba(255, 255, 255, 0.2);
    box-shadow: 0 4px 14px rgba(66, 133, 244, 0.35);
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-chrome-launch:hover {
    background: linear-gradient(135deg, #5393f7 0%, #1765c9 100%);
    box-shadow: 0 6px 18px rgba(66, 133, 244, 0.5);
    transform: translateY(-1px);
  }

  .btn-chrome-launch:active {
    transform: translateY(0);
  }

  .btn-sandbox-launch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 500;
    color: #c9d1d9;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.18s ease;
  }

  .btn-sandbox-launch:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-1px);
  }

  .btn-sandbox-launch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .notice-bar {
    padding: 10px 16px;
    border-radius: 10px;
    background: rgba(245, 158, 11, 0.12);
    border: 1px solid rgba(245, 158, 11, 0.25);
    color: #fbbf24;
    font-size: 13px;
  }

  /* ---------------- SEARCH SECTION ---------------- */
  .search-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px 8px 18px;
    background: rgba(22, 27, 34, 0.85);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 16px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .search-bar:focus-within {
    border-color: rgba(66, 133, 244, 0.5);
    box-shadow: 0 8px 28px rgba(66, 133, 244, 0.2);
  }

  .search-icon-wrapper {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #ffffff;
    font-size: 15px;
    font-weight: 400;
  }

  .search-input::placeholder {
    color: #6e7681;
  }

  .search-bar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .search-submit-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 9px;
    font-size: 12px;
    font-weight: 600;
    color: #ffffff;
    background: #4285F4;
    border: none;
    cursor: pointer;
    transition: background 0.15s, transform 0.15s;
  }

  .search-submit-btn:hover {
    background: #3367d6;
    transform: translateY(-1px);
  }

  .search-sandbox-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 9px;
    font-size: 12px;
    font-weight: 500;
    color: #c9d1d9;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.15s;
  }

  .search-sandbox-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .search-sandbox-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .search-pills-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 0 4px;
  }

  .pills-label {
    font-size: 12px;
    color: #6e7681;
    font-weight: 500;
  }

  .search-pill {
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 500;
    color: #8b949e;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    cursor: pointer;
    transition: all 0.15s;
  }

  .search-pill:hover {
    color: #ffffff;
    background: rgba(66, 133, 244, 0.15);
    border-color: rgba(66, 133, 244, 0.3);
  }

  /* ---------------- RESTORE BAR ---------------- */
  .restore-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 18px;
    background: rgba(66, 133, 244, 0.08);
    border: 1px solid rgba(66, 133, 244, 0.2);
    border-radius: 12px;
  }

  .restore-text {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #79b8ff;
    font-size: 13px;
  }

  .restore-btn {
    padding: 6px 14px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 600;
    background: #4285F4;
    color: #ffffff;
    border: none;
    cursor: pointer;
    transition: background 0.15s;
  }

  .restore-btn:hover {
    background: #3367d6;
  }

  /* ---------------- ACTIVE SESSIONS CARD ---------------- */
  .active-sessions-card {
    background: rgba(22, 27, 34, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .beacon-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .beacon-pulse {
    position: relative;
    width: 12px;
    height: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .beacon-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #34A853;
  }

  .beacon-ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid #34A853;
    animation: beacon-ping 1.8s cubic-bezier(0, 0, 0.2, 1) infinite;
  }

  @keyframes beacon-ping {
    0% { transform: scale(0.8); opacity: 0.9; }
    80%, 100% { transform: scale(2.2); opacity: 0; }
  }

  .beacon-title-row h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #ffffff;
  }

  .count-badge {
    padding: 2px 7px;
    border-radius: 10px;
    background: rgba(52, 168, 83, 0.15);
    color: #34A853;
    font-size: 11px;
    font-weight: 700;
  }

  .header-sublabel {
    font-size: 12px;
    color: #8b949e;
  }

  .tabs-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tab-item-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    background: rgba(13, 17, 23, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    transition: border-color 0.15s;
  }

  .tab-item-card:hover {
    border-color: rgba(255, 255, 255, 0.12);
  }

  .tab-item-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }

  .tab-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tab-title {
    font-size: 14px;
    color: #ffffff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pill-badge-isolated {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 10px;
    background: rgba(168, 85, 247, 0.15);
    color: #c084fc;
    border: 1px solid rgba(168, 85, 247, 0.3);
  }

  .tab-url-text {
    font-size: 12px;
    color: #8b949e;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tab-controls-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    flex-shrink: 0;
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 9px;
    border-radius: 7px;
    font-size: 11px;
    font-weight: 500;
    color: #c9d1d9;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    transition: all 0.15s;
  }

  .tab-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .tab-btn.highlight {
    background: rgba(66, 133, 244, 0.18);
    color: #79b8ff;
    border-color: rgba(66, 133, 244, 0.35);
  }

  .tab-btn.highlight:hover {
    background: rgba(66, 133, 244, 0.3);
  }

  .tab-btn.close {
    color: #f85149;
  }

  .tab-btn.close:hover {
    background: rgba(248, 81, 73, 0.15);
    border-color: rgba(248, 81, 73, 0.3);
  }

  /* ---------------- SPEED-DIAL SHORTCUTS ---------------- */
  .shortcuts-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section-title-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
  }

  .section-title-left h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    color: #ffffff;
    letter-spacing: -0.01em;
  }

  .subtext {
    font-size: 12px;
    color: #8b949e;
  }

  .tabs-segmented-toggle {
    display: flex;
    padding: 3px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    gap: 2px;
  }

  .toggle-btn {
    padding: 5px 12px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 500;
    color: #8b949e;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 0.15s;
  }

  .toggle-btn:hover {
    color: #ffffff;
  }

  .toggle-btn.active {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
    font-weight: 600;
  }

  .shortcuts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(310px, 1fr));
    gap: 14px;
  }

  .shortcut-card {
    padding: 16px 18px;
    background: rgba(22, 27, 34, 0.65);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .shortcut-card:hover {
    background: rgba(30, 36, 48, 0.85);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3);
  }

  .shortcut-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .shortcut-icon-squircle {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
  }

  .shortcut-actions-inline {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .category-chip {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border-radius: 6px;
  }

  .mini-sandbox-btn {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #8b949e;
    cursor: pointer;
    transition: all 0.15s;
  }

  .mini-sandbox-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
  }

  .shortcut-body {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .shortcut-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: #ffffff;
  }

  .shortcut-desc {
    margin: 0;
    font-size: 12px;
    color: #8b949e;
    line-height: 1.4;
  }

  .shortcut-url-subtext {
    font-size: 11px;
    color: #58a6ff;
    font-family: monospace;
    margin-top: 4px;
  }

  /* ---------------- BOOKMARKS & DOWNLOADS ---------------- */
  .bookmarks-grid, .downloads-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .bookmark-item-card, .download-item-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 16px;
    background: rgba(22, 27, 34, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
  }

  .bookmark-icon, .download-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .bookmark-info, .download-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bookmark-title, .download-title {
    font-size: 13px;
    color: #ffffff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bookmark-url {
    font-size: 11px;
    color: #8b949e;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-pill {
    font-size: 10px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(245, 158, 11, 0.15);
    color: #fbbf24;
    display: inline-block;
  }

  .status-pill.complete {
    background: rgba(52, 168, 83, 0.15);
    color: #34A853;
  }

  .bookmark-actions, .download-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .btn-sm {
    padding: 4px 10px;
    border-radius: 7px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.15s;
  }

  .btn-sm.primary {
    background: #4285F4;
    color: #ffffff;
  }

  .btn-sm.primary:hover:not(:disabled) {
    background: #3367d6;
  }

  .btn-sm.ghost {
    background: rgba(255, 255, 255, 0.06);
    color: #c9d1d9;
    border-color: rgba(255, 255, 255, 0.08);
  }

  .btn-sm.ghost:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .btn-sm.danger {
    background: rgba(248, 81, 73, 0.1);
    color: #f85149;
    border-color: rgba(248, 81, 73, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .btn-sm.danger:hover:not(:disabled) {
    background: rgba(248, 81, 73, 0.2);
  }

  .btn-sm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .empty-state-box {
    padding: 36px 20px;
    background: rgba(22, 27, 34, 0.4);
    border: 1px dashed rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 8px;
  }

  .empty-icon-ring {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.04);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 4px;
  }

  .empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #e6edf3;
  }

  .empty-desc {
    margin: 0;
    font-size: 12px;
    color: #8b949e;
    max-width: 360px;
  }

  /* ---------------- FOOTER ---------------- */
  .browser-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 16px 20px;
    background: rgba(22, 27, 34, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 14px;
    margin-top: 8px;
  }

  .privacy-badge-group {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
  }

  .shield-icon-badge {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: rgba(52, 168, 83, 0.12);
    border: 1px solid rgba(52, 168, 83, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .privacy-text {
    margin: 0;
    font-size: 12px;
    color: #8b949e;
    line-height: 1.4;
  }

  .footer-actions {
    flex-shrink: 0;
  }

  .btn-clear-cache {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    color: #f85149;
    background: rgba(248, 81, 73, 0.08);
    border: 1px solid rgba(248, 81, 73, 0.2);
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-clear-cache:hover:not(:disabled) {
    background: rgba(248, 81, 73, 0.18);
    border-color: rgba(248, 81, 73, 0.35);
  }

  .btn-clear-cache:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ---------------- RESPONSIVENESS ---------------- */
  @media (max-width: 768px) {
    .browser-container {
      padding: 16px;
      gap: 16px;
    }
    .browser-hero {
      flex-direction: column;
      align-items: flex-start;
      gap: 14px;
    }
    .hero-actions {
      width: 100%;
      flex-direction: column;
    }
    .btn-chrome-launch, .btn-sandbox-launch {
      width: 100%;
      justify-content: center;
    }
    .section-title-row {
      flex-direction: column;
      align-items: flex-start;
    }
    .shortcuts-grid {
      grid-template-columns: 1fr;
    }
    .browser-footer {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
