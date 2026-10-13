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
  export let confirm = (msg, fn) => fn?.();
  export let website = () => {};
  export let restoreTabs = () => {};
  export let browserAction = () => {};
  export let bookmark = () => {};
  export let permissions = () => {};

  let searchQuery = '';
  let customUrlInput = '';
  let activeVideoId = 'jfKfPfyJRdk'; // Lofi Hip Hop Radio
  let activeVideoTitle = 'Lofi Hip Hop Radio — Beats to Relax/Study to';
  let embedNotice = '';

  const presets = [
    { id: 'jfKfPfyJRdk', title: 'Lofi Beats', emoji: '🎧', desc: 'Chill beats to relax/study to' },
    { id: 'r4S_v5v4wF8', title: 'Nature 4K', emoji: '🌿', desc: 'Relaxing scenic landscapes' },
    { id: '0b_mH_6851I', title: 'Space Ambient', emoji: '🪐', desc: 'Deep cosmic journey visuals' },
    { id: 'Dx5qFachd3A', title: 'Jazz Cafe', emoji: '☕', desc: 'Cozy acoustic jazz atmosphere' }
  ];

  const categories = [
    {
      id: 'trending',
      title: 'Trending',
      subtitle: 'Explore what the world is watching right now',
      url: 'https://www.youtube.com/feed/trending',
      badge: '🔥 Hot',
      color: '#FF453A'
    },
    {
      id: 'music',
      title: 'YouTube Music',
      subtitle: 'Official albums, top tracks, and personalized mixes',
      url: 'https://www.youtube.com/music',
      badge: '🎵 Audio',
      color: '#EE1D23'
    },
    {
      id: 'gaming',
      title: 'Gaming Hub',
      subtitle: 'Live game broadcasts, speedruns, and tournaments',
      url: 'https://www.youtube.com/gaming',
      badge: '🎮 Live',
      color: '#818CF8'
    },
    {
      id: 'news',
      title: 'World News',
      subtitle: 'Verified breaking stories and global journalism',
      url: 'https://www.youtube.com/news',
      badge: '📰 Reports',
      color: '#38BDF8'
    },
    {
      id: 'learning',
      title: 'Learning & Tech',
      subtitle: 'Deep dives, coding masterclasses, and science',
      url: 'https://www.youtube.com/learning',
      badge: '📚 Intel',
      color: '#34D399'
    },
    {
      id: 'movies',
      title: 'Movies & TV',
      subtitle: 'Cinema features, primetime releases, and rentals',
      url: 'https://www.youtube.com/feed/storefront',
      badge: '🎬 Studio',
      color: '#FBBF24'
    }
  ];

  $: safeTabs = Array.isArray(tabs) ? tabs : [];
  $: safePreviousTabs = Array.isArray(previousTabs) ? previousTabs : [];
  $: safeDownloads = Array.isArray(downloads) ? downloads : [];
  $: youtubeTab = safeTabs.find(tab => tab.isolated || (tab.url && tab.url.includes('youtube.com')));
  $: if (address && !customUrlInput) {
    customUrlInput = address;
  }
  $: youtubeBookmarks = (bookmarks || []).filter(b => {
    try {
      const u = json(b.body)?.url;
      return typeof u === 'string' && u.includes('youtube.com');
    } catch {
      return false;
    }
  });

  function extractVideoId(urlOrId) {
    if (!urlOrId) return '';
    const trimmed = urlOrId.trim();
    if (/^[a-zA-Z0-9_-]{11}$/.test(trimmed)) return trimmed;
    const match = trimmed.match(/(?:youtu\.be\/|youtube\.com\/(?:embed\/|v\/|watch\?v=|watch\?.+&v=))([\w-]{11})/);
    return match ? match[1] : '';
  }

  function launchUrl(url) {
    if (native) {
      run(() => website(url, true));
    } else {
      run(() => command('open_external', { url }));
    }
  }

  function handleSearch() {
    if (!searchQuery.trim()) return;
    const url = `https://www.youtube.com/results?search_query=${encodeURIComponent(searchQuery.trim())}`;
    launchUrl(url);
  }

  function searchTag(tag) {
    searchQuery = tag;
    handleSearch();
  }

  function selectPreset(preset) {
    activeVideoId = preset.id;
    activeVideoTitle = preset.title;
    embedNotice = '';
  }

  function loadCustomVideo() {
    const id = extractVideoId(customUrlInput);
    if (id) {
      activeVideoId = id;
      activeVideoTitle = `Custom Video (${id})`;
      embedNotice = '';
      customUrlInput = '';
    } else if (customUrlInput.trim()) {
      embedNotice = t('Invalid YouTube link or ID. Launching in session...');
      const target = customUrlInput.trim().startsWith('http')
        ? customUrlInput.trim()
        : `https://www.youtube.com/results?search_query=${encodeURIComponent(customUrlInput.trim())}`;
      launchUrl(target);
      customUrlInput = '';
    }
  }

  function popoutCurrent() {
    launchUrl(`https://www.youtube.com/watch?v=${activeVideoId}`);
  }
</script>

<div class="youtube-app" role="region" aria-label={t('YouTube')}>
  <div class="yt-container">
    <!-- Hero Header Banner -->
    <header class="yt-hero">
      <div class="hero-brand">
        <div class="brand-squircle" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="28" height="28" fill="#FFFFFF">
            <path d="M19.615 3.184c-3.604-.246-11.631-.245-15.23 0-3.897.266-4.356 2.62-4.385 8.816.029 6.185.484 8.549 4.385 8.816 3.6.245 11.626.246 15.23 0 3.897-.266 4.356-2.62 4.385-8.816-.029-6.185-.484-8.549-4.385-8.816zm-10.615 12.816v-8l8 3.993-8 4.007z"/>
          </svg>
        </div>
        <div class="brand-text">
          <div class="brand-title-row">
            <h1 class="brand-title">YouTube</h1>
            <span class="sandbox-badge">{t('SANDBOXED')}</span>
          </div>
          <p class="brand-subtitle">{t('Dedicated isolated web media session')}</p>
        </div>
      </div>

      <div class="hero-actions">
        {#if youtubeTab}
          <button
            type="button"
            class="yt-btn-primary"
            on:click={() => run(() => browserAction(youtubeTab, 'focus'))}
            title={t('Bring YouTube window to front')}
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>
            </svg>
            <span>{t('Bring Window to Front')}</span>
          </button>
        {:else}
          <button
            type="button"
            class="yt-btn-primary"
            on:click={() => launchUrl('https://www.youtube.com')}
            disabled={!native}
            title={t('Open dedicated YouTube session')}
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            <span>{t('Launch YouTube Session')}</span>
          </button>
        {/if}

        <button
          type="button"
          class="yt-btn-ghost"
          on:click={() => run(() => command('open_external', { url: 'https://www.youtube.com' }))}
          title={t('Open in default system browser')}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3"/>
          </svg>
          <span>{t('Open in Default Browser')}</span>
        </button>
      </div>
    </header>

    {#if !native}
      <div class="notice-bar">
        <span>{t('Private apps are available in the installed Windows and Linux app.')}</span>
      </div>
    {/if}

    <!-- Restore Previous Session Banner (if closed) -->
    {#if safePreviousTabs.length > 0 && !youtubeTab}
      <div class="restore-bar">
        <div class="restore-text">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
            <path d="M3 3v5h5"/>
          </svg>
          <span>{t('Previous session available from your last run.')}</span>
        </div>
        <button type="button" class="restore-btn" on:click={() => run(restoreTabs)}>
          {t('Restore previous tabs')} ({safePreviousTabs.length})
        </button>
      </div>
    {/if}

    <!-- Active Session Controller (When YouTube Window is Live) -->
    {#if youtubeTab}
      <section class="active-session-card" aria-label={t('Active YouTube Session')}>
        <div class="session-header">
          <div class="session-beacon-row">
            <div class="beacon-pulse" aria-hidden="true">
              <span class="beacon-dot"></span>
              <span class="beacon-ring"></span>
            </div>
            <div class="session-meta">
              <div class="session-label-row">
                <span class="session-status-text">{t('Live Session Active')}</span>
                {#if youtubeTab.isolated}
                  <span class="session-pill">{t('Isolated Window')}</span>
                {/if}
              </div>
              <div class="session-title" title={youtubeTab.title}>{youtubeTab.title || 'YouTube Media Window'}</div>
              <div class="session-url" title={youtubeTab.url}>{youtubeTab.url}</div>
            </div>
          </div>
        </div>

        <div class="session-toolbar">
          <button
            type="button"
            class="session-btn session-btn-primary"
            on:click={() => run(() => browserAction(youtubeTab, 'focus'))}
            title={t('Bring YouTube window to front')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>
            </svg>
            <span>{t('Focus Window')}</span>
          </button>

          <button
            type="button"
            class="session-btn"
            on:click={() => run(() => browserAction(youtubeTab, 'fullscreen'))}
            title={t('Toggle fullscreen view')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/>
            </svg>
            <span>{t('Fullscreen')}</span>
          </button>

          <button
            type="button"
            class="session-btn"
            on:click={() => run(() => browserAction(youtubeTab, 'reload'))}
            title={t('Reload session')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
            </svg>
            <span>{t('Reload')}</span>
          </button>

          <button
            type="button"
            class="session-btn"
            on:click={() => run(() => bookmark(youtubeTab))}
            title={t('Save current video to bookmarks')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>
            </svg>
            <span>{t('Bookmark')}</span>
          </button>

          <button
            type="button"
            class="session-btn"
            on:click={() => permissions(youtubeTab)}
            title={t('Manage permissions')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
              <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
            </svg>
            <span>{t('Permissions')}</span>
          </button>

          <button
            type="button"
            class="session-btn session-btn-danger"
            on:click={() => run(() => browserAction(youtubeTab, 'close'))}
            title={t('Close YouTube session')}
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
            <span>{t('Close Session')}</span>
          </button>
        </div>
      </section>
    {/if}

    <!-- Search Section -->
    <section class="search-section" aria-label={t('Search YouTube')}>
      <form class="search-form" on:submit|preventDefault={handleSearch}>
        <div class="search-input-shell">
          <svg class="search-icon" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"/>
            <path d="m21 21-4.35-4.35"/>
          </svg>
          <input
            type="search"
            class="search-input"
            placeholder={t('Search YouTube (e.g. lo-fi music, news, gaming, podcasts)...')}
            bind:value={searchQuery}
            aria-label={t('Search YouTube')}
          />
          {#if searchQuery}
            <button
              type="button"
              class="search-clear-btn"
              on:click={() => searchQuery = ''}
              aria-label={t('Clear search query')}
            >×</button>
          {/if}
        </div>
        <button
          type="submit"
          class="search-submit-btn"
          disabled={!searchQuery.trim()}
          title={t('Search in YouTube session')}
        >
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"/>
            <path d="m21 21-4.35-4.35"/>
          </svg>
          <span>{t('Search')}</span>
        </button>
      </form>

      <div class="quick-tags-row">
        <span class="tags-lead">{t('Trending Topics')}:</span>
        <div class="tags-list">
          {#each ['Lo-Fi Beats', 'Tech Reviews', 'Live Music', 'Science & Space', 'Documentaries', 'Cooking'] as tag}
            <button
              type="button"
              class="tag-chip"
              on:click={() => searchTag(tag)}
            >
              {tag}
            </button>
          {/each}
        </div>
      </div>
    </section>

    <!-- Quick Category Launchers (Grid of 6) -->
    <section class="categories-section" aria-label={t('Quick Categories')}>
      <div class="section-header">
        <h2 class="section-title">{t('Explore Categories')}</h2>
        <span class="section-hint">{t('One-click launch to curated feeds')}</span>
      </div>

      <div class="categories-grid">
        {#each categories as cat}
          <button
            type="button"
            class="category-card"
            style="--cat-color: {cat.color};"
            on:click={() => launchUrl(cat.url)}
          >
            <div class="cat-top">
              <div class="cat-icon-squircle">
                {#if cat.id === 'trending'}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 3z"/>
                  </svg>
                {:else if cat.id === 'music'}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M9 18V5l12-2v13"/>
                    <circle cx="6" cy="18" r="3"/>
                    <circle cx="18" cy="16" r="3"/>
                  </svg>
                {:else if cat.id === 'gaming'}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="6" y1="12" x2="10" y2="12"/>
                    <line x1="8" y1="10" x2="8" y2="14"/>
                    <line x1="15" y1="13" x2="15.01" y2="13"/>
                    <line x1="18" y1="11" x2="18.01" y2="11"/>
                    <rect x="2" y="6" width="20" height="12" rx="6"/>
                  </svg>
                {:else if cat.id === 'news'}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2Zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"/>
                    <path d="M18 14h-8"/>
                    <path d="M15 18h-5"/>
                    <path d="M10 6h8v4h-8z"/>
                  </svg>
                {:else if cat.id === 'learning'}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 10v6M2 10l10-5 10 5-10 5z"/>
                    <path d="M6 12v5c3 3 9 3 12 0v-5"/>
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"/>
                    <line x1="7" y1="2" x2="7" y2="22"/>
                    <line x1="17" y1="2" x2="17" y2="22"/>
                    <line x1="2" y1="12" x2="22" y2="12"/>
                    <line x1="2" y1="7" x2="7" y2="7"/>
                    <line x1="2" y1="17" x2="7" y2="17"/>
                    <line x1="17" y1="17" x2="22" y2="17"/>
                    <line x1="17" y1="7" x2="22" y2="7"/>
                  </svg>
                {/if}
              </div>
              <span class="cat-badge">{cat.badge}</span>
            </div>

            <div class="cat-content">
              <h3 class="cat-title">{cat.title}</h3>
              <p class="cat-desc">{cat.subtitle}</p>
            </div>

            <div class="cat-footer">
              <span class="cat-launch-label">{t('Launch Feed')}</span>
              <svg class="cat-arrow" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12h14M12 5l7 7-7 7"/>
              </svg>
            </div>
          </button>
        {/each}
      </div>
    </section>

    <!-- In-Window Embedded Video Player -->
    <section class="player-section" aria-label={t('In-Window Player')}>
      <div class="player-topbar">
        <div class="player-meta">
          <div class="player-title-row">
            <span class="player-tag">{t('IN-APP CANVAS')}</span>
            <h2 class="player-title">{t('In-Window Player')}</h2>
          </div>
          <p class="player-desc">{activeVideoTitle || t('Stream video directly inside Home Hub')}</p>
        </div>

        <div class="player-presets">
          {#each presets as p}
            <button
              type="button"
              class="preset-pill"
              class:active={activeVideoId === p.id}
              on:click={() => selectPreset(p)}
              title={p.desc}
            >
              <span class="preset-emoji">{p.emoji}</span>
              <span class="preset-name">{p.title}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- 16:9 Responsive Video Canvas -->
      <div class="player-frame-wrapper">
        <div class="player-aspect-box">
          <iframe
            class="embedded-player-frame"
            src={`https://www.youtube-nocookie.com/embed/${activeVideoId}?autoplay=0&rel=0&modestbranding=1`}
            title={activeVideoTitle || "YouTube Video Player"}
            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
            allowfullscreen
          ></iframe>
        </div>
      </div>

      <!-- Paste Custom Video Bar -->
      <form class="custom-video-bar" on:submit|preventDefault={loadCustomVideo}>
        <div class="custom-input-wrapper">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
            <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
          </svg>
          <input
            type="text"
            class="custom-url-input"
            placeholder={t('Paste YouTube URL or video ID to stream here (e.g. youtu.be/...)')}
            bind:value={customUrlInput}
            aria-label={t('YouTube video URL or ID')}
          />
        </div>
        <button
          type="submit"
          class="play-in-app-btn"
          disabled={!customUrlInput.trim()}
        >
          {t('Play In-Window')}
        </button>
        <button
          type="button"
          class="popout-btn"
          on:click={popoutCurrent}
          title={t('Open active video in isolated YouTube session')}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3"/>
          </svg>
          <span>{t('Pop Out')}</span>
        </button>
      </form>

      {#if embedNotice}
        <p class="embed-notice">{embedNotice}</p>
      {/if}
    </section>

    <!-- Bookmarks section (shown only when user has saved YouTube bookmarks) -->
    {#if youtubeBookmarks.length > 0}
      <section class="bookmarks-section" aria-label={t('Saved Bookmarks')}>
        <div class="section-header">
          <h2 class="section-title">{t('Saved YouTube Bookmarks')}</h2>
          <span class="section-hint">{youtubeBookmarks.length} {t('saved')}</span>
        </div>
        <div class="bookmarks-grid">
          {#each youtubeBookmarks as row}
            {@const bUrl = json(row.body)?.url}
            <div class="bookmark-card">
              <button
                type="button"
                class="bookmark-link-btn"
                on:click={() => launchUrl(bUrl)}
                title={bUrl}
              >
                <div class="bookmark-icon">
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="#EE1D23">
                    <path d="M19.615 3.184c-3.604-.246-11.631-.245-15.23 0-3.897.266-4.356 2.62-4.385 8.816.029 6.185.484 8.549 4.385 8.816 3.6.245 11.626.246 15.23 0 3.897-.266 4.356-2.62 4.385-8.816-.029-6.185-.484-8.549-4.385-8.816zm-10.615 12.816v-8l8 3.993-8 4.007z"/>
                  </svg>
                </div>
                <span class="bookmark-title">{row.title || bUrl}</span>
              </button>
              <button
                type="button"
                class="bookmark-remove-btn"
                on:click={() => run(async () => { await command('local_trash', { id: row.id }); await load(); })}
                aria-label={t('Remove bookmark')}
                title={t('Remove')}
              >×</button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Privacy & Sandbox Notice Footer -->
    <footer class="privacy-footer">
      <div class="privacy-notice">
        <div class="privacy-shield" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="#22C55E" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
        </div>
        <div class="privacy-copy">
          <div class="privacy-header-row">
            <strong class="privacy-title">{t('Isolated Web Runner · Sandboxed Profile · Zero Local Hub Access')}</strong>
          </div>
          <p class="privacy-desc">
            {t('YouTube runs in a dedicated webview container with private on-disk cookies and storage. Google login credentials and tracking cookies cannot access your local drives, photos, database, or cryptographic keys.')}
          </p>
        </div>
      </div>

      <div class="privacy-actions">
        <button
          type="button"
          class="clear-storage-btn"
          on:click={() => confirm(t('Clear website data'), async () => { await command('browser_clear'); await load(); })}
          title={t('Clear cache, cookies, and local session storage')}
        >
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M10 11v6M14 11v6"/>
          </svg>
          <span>{t('Clear Session Storage')}</span>
        </button>
      </div>
    </footer>
  </div>
</div>

<style>
  .youtube-app {
    width: 100%;
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    overflow-x: hidden;
    background: radial-gradient(ellipse 90% 40% at 80% 0%, rgba(238, 29, 35, 0.12), transparent 70%), #0A0E13;
    color: #E2E8F0;
    box-sizing: border-box;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  .yt-container {
    max-width: 1100px;
    width: 100%;
    margin: 0 auto;
    padding: 24px 28px 48px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    box-sizing: border-box;
  }

  /* Hero Header */
  .yt-hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 20px 24px;
    background: linear-gradient(135deg, rgba(238, 29, 35, 0.14) 0%, rgba(22, 28, 38, 0.8) 100%);
    border: 1px solid rgba(238, 29, 35, 0.28);
    border-radius: 16px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(12px);
  }

  .hero-brand {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .brand-squircle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    background: linear-gradient(135deg, #FF0000 0%, #C41218 100%);
    box-shadow: 0 6px 20px rgba(238, 29, 35, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.3);
    flex-shrink: 0;
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
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: #FFFFFF;
    line-height: 1.15;
  }

  .sandbox-badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(238, 29, 35, 0.18);
    color: #FF6B6B;
    border: 1px solid rgba(238, 29, 35, 0.35);
  }

  .brand-subtitle {
    margin: 0;
    font-size: 13px;
    color: #94A3B8;
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .yt-btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #EE1D23 0%, #C41218 100%);
    color: #FFFFFF;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 10px;
    padding: 10px 18px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 16px rgba(238, 29, 35, 0.35);
    transition: all 0.18s ease;
  }

  .yt-btn-primary:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 6px 20px rgba(238, 29, 35, 0.5);
    background: linear-gradient(135deg, #FF2E34 0%, #D4141A 100%);
  }

  .yt-btn-primary:active:not(:disabled) {
    transform: translateY(0);
  }

  .yt-btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .yt-btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #CBD5E1;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 10px;
    padding: 10px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    backdrop-filter: blur(8px);
    transition: all 0.18s ease;
  }

  .yt-btn-ghost:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
    border-color: rgba(255, 255, 255, 0.2);
  }

  /* Notice Bar & Restore Bar */
  .notice-bar, .restore-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 18px;
    border-radius: 10px;
    font-size: 12px;
  }

  .notice-bar {
    background: rgba(238, 29, 35, 0.08);
    border: 1px solid rgba(238, 29, 35, 0.2);
    color: #FCA5A5;
  }

  .restore-bar {
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.25);
    color: #93C5FD;
  }

  .restore-text {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .restore-btn {
    background: #2563EB;
    color: #FFFFFF;
    border: none;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .restore-btn:hover {
    background: #1D4ED8;
  }

  /* Active Session Controller */
  .active-session-card {
    background: linear-gradient(135deg, rgba(16, 185, 129, 0.08) 0%, rgba(22, 28, 38, 0.85) 100%);
    border: 1px solid rgba(16, 185, 129, 0.25);
    border-radius: 14px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }

  .session-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .session-beacon-row {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    min-width: 0;
  }

  .beacon-pulse {
    position: relative;
    width: 14px;
    height: 14px;
    margin-top: 3px;
    flex-shrink: 0;
  }

  .beacon-dot {
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    background: #10B981;
    box-shadow: 0 0 10px #10B981;
  }

  .beacon-ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid #10B981;
    animation: beacon-expand 2s infinite ease-out;
  }

  @keyframes beacon-expand {
    0% { transform: scale(0.8); opacity: 0.9; }
    80% { transform: scale(2.2); opacity: 0; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  .session-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .session-label-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .session-status-text {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #34D399;
    text-transform: uppercase;
  }

  .session-pill {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: #94A3B8;
  }

  .session-title {
    font-size: 14px;
    font-weight: 600;
    color: #F8FAFC;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 650px;
  }

  .session-url {
    font-size: 12px;
    color: #64748B;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 650px;
  }

  .session-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .session-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: #E2E8F0;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .session-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
  }

  .session-btn-primary {
    background: rgba(16, 185, 129, 0.18);
    color: #6EE7B7;
    border-color: rgba(16, 185, 129, 0.35);
  }

  .session-btn-primary:hover {
    background: rgba(16, 185, 129, 0.28);
    color: #A7F3D0;
  }

  .session-btn-danger {
    background: rgba(239, 68, 68, 0.12);
    color: #FCA5A5;
    border-color: rgba(239, 68, 68, 0.25);
  }

  .session-btn-danger:hover {
    background: rgba(239, 68, 68, 0.22);
    color: #FECACA;
  }

  /* Search Section */
  .search-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .search-form {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .search-input-shell {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 14px;
    color: #64748B;
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    background: rgba(22, 28, 38, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 12px 38px 12px 42px;
    font-size: 14px;
    color: #F8FAFC;
    outline: none;
    transition: all 0.18s ease;
    box-sizing: border-box;
  }

  .search-input:focus {
    border-color: rgba(238, 29, 35, 0.5);
    box-shadow: 0 0 0 3px rgba(238, 29, 35, 0.15);
    background: rgba(26, 33, 46, 0.95);
  }

  .search-input::placeholder {
    color: #64748B;
  }

  .search-clear-btn {
    position: absolute;
    right: 12px;
    background: transparent;
    border: none;
    color: #94A3B8;
    font-size: 18px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
  }

  .search-clear-btn:hover {
    color: #FFFFFF;
  }

  .search-submit-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #EE1D23 0%, #C41218 100%);
    color: #FFFFFF;
    border: none;
    border-radius: 12px;
    padding: 12px 20px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 14px rgba(238, 29, 35, 0.3);
    transition: all 0.18s ease;
    flex-shrink: 0;
  }

  .search-submit-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 6px 18px rgba(238, 29, 35, 0.45);
  }

  .search-submit-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .quick-tags-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 12px;
  }

  .tags-lead {
    color: #64748B;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 600;
  }

  .tags-list {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .tag-chip {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #94A3B8;
    border-radius: 20px;
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .tag-chip:hover {
    background: rgba(238, 29, 35, 0.15);
    color: #FFFFFF;
    border-color: rgba(238, 29, 35, 0.3);
  }

  /* Categories Section */
  .categories-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .section-title {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: #FFFFFF;
    letter-spacing: -0.01em;
  }

  .section-hint {
    font-size: 12px;
    color: #64748B;
  }

  .categories-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 14px;
  }

  .category-card {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 140px;
    background: rgba(22, 28, 38, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 16px;
    text-align: left;
    cursor: pointer;
    transition: all 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.2);
    position: relative;
    overflow: hidden;
  }

  .category-card::before {
    content: '';
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 10% 10%, var(--cat-color), transparent 65%);
    opacity: 0.12;
    transition: opacity 0.22s ease;
    pointer-events: none;
  }

  .category-card:hover {
    transform: translateY(-3px);
    border-color: var(--cat-color);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
  }

  .category-card:hover::before {
    opacity: 0.24;
  }

  .cat-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .cat-icon-squircle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--cat-color);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .cat-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: #CBD5E1;
  }

  .cat-content {
    margin-bottom: 12px;
  }

  .cat-title {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 700;
    color: #F8FAFC;
  }

  .cat-desc {
    margin: 0;
    font-size: 12px;
    color: #94A3B8;
    line-height: 1.35;
  }

  .cat-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 10px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    color: #CBD5E1;
    font-size: 12px;
    font-weight: 600;
  }

  .cat-arrow {
    transform: translateX(0);
    transition: transform 0.18s ease;
    color: var(--cat-color);
  }

  .category-card:hover .cat-arrow {
    transform: translateX(4px);
  }

  /* In-Window Embedded Player */
  .player-section {
    background: rgba(18, 24, 34, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.3);
  }

  .player-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }

  .player-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .player-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .player-tag {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(238, 29, 35, 0.18);
    color: #FF6B6B;
  }

  .player-title {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: #FFFFFF;
  }

  .player-desc {
    margin: 0;
    font-size: 12px;
    color: #94A3B8;
  }

  .player-presets {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .preset-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 6px 10px;
    font-size: 12px;
    color: #CBD5E1;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .preset-pill:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #FFFFFF;
  }

  .preset-pill.active {
    background: rgba(238, 29, 35, 0.2);
    border-color: rgba(238, 29, 35, 0.4);
    color: #FFFFFF;
    box-shadow: 0 0 10px rgba(238, 29, 35, 0.2);
  }

  .player-frame-wrapper {
    width: 100%;
    border-radius: 12px;
    overflow: hidden;
    background: #000000;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.6);
  }

  .player-aspect-box {
    position: relative;
    width: 100%;
    padding-bottom: 56.25%; /* 16:9 ratio */
    height: 0;
  }

  .embedded-player-frame {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    border: 0;
  }

  .custom-video-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .custom-input-wrapper {
    position: relative;
    flex: 1;
    min-width: 260px;
    display: flex;
    align-items: center;
  }

  .custom-input-wrapper svg {
    position: absolute;
    left: 12px;
    color: #64748B;
  }

  .custom-url-input {
    width: 100%;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 9px 12px 9px 36px;
    font-size: 13px;
    color: #F8FAFC;
    outline: none;
    box-sizing: border-box;
    transition: all 0.15s ease;
  }

  .custom-url-input:focus {
    border-color: rgba(238, 29, 35, 0.4);
    background: rgba(255, 255, 255, 0.08);
  }

  .play-in-app-btn {
    background: #EE1D23;
    color: #FFFFFF;
    border: none;
    border-radius: 8px;
    padding: 9px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    flex-shrink: 0;
  }

  .play-in-app-btn:hover:not(:disabled) {
    background: #D9161B;
  }

  .play-in-app-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .popout-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.06);
    color: #CBD5E1;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 9px 14px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    flex-shrink: 0;
  }

  .popout-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
  }

  .embed-notice {
    margin: -4px 0 0;
    font-size: 12px;
    color: #FBBF24;
  }

  /* Bookmarks Section */
  .bookmarks-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .bookmarks-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }

  .bookmark-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    background: rgba(22, 28, 38, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 8px 10px;
    transition: background 0.15s;
  }

  .bookmark-card:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .bookmark-link-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    color: #CBD5E1;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    flex: 1;
    padding: 0;
  }

  .bookmark-link-btn:hover {
    color: #FFFFFF;
  }

  .bookmark-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .bookmark-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bookmark-remove-btn {
    background: transparent;
    border: none;
    color: #64748B;
    font-size: 16px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
  }

  .bookmark-remove-btn:hover {
    color: #EF4444;
  }

  /* Privacy Footer */
  .privacy-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 16px 20px;
    background: rgba(16, 22, 30, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 14px;
    flex-wrap: wrap;
  }

  .privacy-notice {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    max-width: 750px;
  }

  .privacy-shield {
    margin-top: 2px;
    flex-shrink: 0;
  }

  .privacy-copy {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .privacy-title {
    font-size: 12px;
    font-weight: 700;
    color: #E2E8F0;
    letter-spacing: 0.02em;
  }

  .privacy-desc {
    margin: 0;
    font-size: 11px;
    color: #94A3B8;
    line-height: 1.45;
  }

  .clear-storage-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(239, 68, 68, 0.1);
    color: #FCA5A5;
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 8px;
    padding: 8px 14px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .clear-storage-btn:hover {
    background: rgba(239, 68, 68, 0.2);
    color: #FECACA;
  }

  @media (max-width: 768px) {
    .yt-hero {
      flex-direction: column;
      align-items: flex-start;
    }
    .hero-actions {
      width: 100%;
    }
    .yt-btn-primary, .yt-btn-ghost {
      flex: 1;
      justify-content: center;
    }
    .categories-grid {
      grid-template-columns: 1fr;
    }
    .privacy-footer {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
