<script>
  import { native, command, media, json, bytes } from '../api.js';

  export let error;
  export let cursor;
  export let playlists = [];
  export let playlist = null;
  export let music = [];
  export let playing = null;
  export let queuedTracks = [];
  export let shuffle = false;
  export let repeat = false;
  export let form = null;
  export let formValue = '';
  export let t;
  export let musicTracks = [];
  export let run;
  export let load;
  export let more;
  export let confirm;
  export let addTrack;
  export let nextTrack;

  let player;
  let volume = 1;
  let isPlaying = false;
  let currentTime = 0;
  let duration = 0;
  let searchQuery = '';
  let showQueue = false;
  let showAddPicker = false;
  let isMuted = false;
  let prevVolume = 1;

  $: if (!playing) {
    isPlaying = false;
    currentTime = 0;
  }

  $: totalBytes = (musicTracks || []).reduce((acc, tr) => acc + (tr.size || 0), 0);

  $: filteredTracks = (musicTracks || []).filter(track => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase().trim();
    return (track.name || '').toLowerCase().includes(q);
  });

  $: currentPlaylistIds = playlist ? (json(playlist.body, { ids: [] }).ids || []) : [];

  $: availableForPlaylist = playlist && Array.isArray(music)
    ? music.filter(f => !currentPlaylistIds.includes(f.id))
    : [];

  $: queueCount = (queuedTracks || []).length;

  function formatTime(seconds) {
    if (isNaN(seconds) || seconds == null || seconds < 0) return '0:00';
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  function cleanTitle(name) {
    if (!name) return 'Unknown Track';
    return name.replace(/\.(mp3|flac|wav|m4a|aac|ogg|wma|opus)$/i, '');
  }

  function getExt(name) {
    if (!name) return 'AUDIO';
    const match = name.match(/\.([a-z0-9]+)$/i);
    return match ? match[1].toUpperCase() : 'AUDIO';
  }

  function playTrack(track) {
    playing = track;
    isPlaying = true;
  }

  function togglePlay() {
    if (!player) {
      if (filteredTracks.length > 0) {
        playTrack(filteredTracks[0]);
      } else if (musicTracks.length > 0) {
        playTrack(musicTracks[0]);
      }
      return;
    }
    if (player.paused) {
      player.play().catch(() => {
        error = t("This track could not be played.");
      });
    } else {
      player.pause();
    }
  }

  function playAll() {
    if (musicTracks.length > 0) {
      playTrack(musicTracks[0]);
    }
  }

  function playShuffle() {
    if (musicTracks.length > 0) {
      shuffle = true;
      const idx = Math.floor(Math.random() * musicTracks.length);
      playTrack(musicTracks[idx]);
    }
  }

  function enqueue(track) {
    queuedTracks = [...queuedTracks, track];
  }

  function dequeue(index) {
    queuedTracks = queuedTracks.filter((_, i) => i !== index);
  }

  function clearQueue() {
    queuedTracks = [];
  }

  function triggerNewPlaylist() {
    form = { action: 'playlist' };
    formValue = '';
  }

  function deletePlaylist(targetPlaylist) {
    confirm(
      t('Remove'),
      async () => {
        await command('local_trash', { id: targetPlaylist.id });
        if (playlist?.id === targetPlaylist.id) {
          playlist = null;
        }
        await load();
      },
      targetPlaylist.title
    );
  }

  function handleSeek(e) {
    const val = Number(e.currentTarget.value);
    currentTime = val;
    if (player) {
      player.currentTime = val;
    }
  }

  function handleVolumeInput(e) {
    volume = Number(e.currentTarget.value);
    if (player) {
      player.volume = volume;
    }
    isMuted = volume === 0;
  }

  function toggleMute() {
    if (isMuted) {
      volume = prevVolume > 0 ? prevVolume : 0.8;
      isMuted = false;
    } else {
      prevVolume = volume > 0 ? volume : 0.8;
      volume = 0;
      isMuted = true;
    }
    if (player) {
      player.volume = volume;
    }
  }
</script>

{#if playing}
  <audio
    bind:this={player}
    src={media(playing.id)}
    autoplay
    loop={repeat}
    on:play={() => isPlaying = true}
    on:pause={() => isPlaying = false}
    on:timeupdate={() => {
      if (player) {
        currentTime = player.currentTime;
        duration = player.duration || 0;
      }
    }}
    on:loadedmetadata={() => {
      if (player) {
        duration = player.duration || 0;
        player.volume = volume;
      }
    }}
    on:ended={() => nextTrack(1, true)}
    on:error={() => {
      error = t("This track could not be played.");
      playing = null;
      isPlaying = false;
    }}
  ></audio>
{/if}

<div class="music-app">
  <div class="music-body">
    <!-- Left Sidebar (230px) -->
    <aside class="music-sidebar" aria-label={t('Library')}>
      <div class="sidebar-top">
        <div class="sidebar-brand">
          <div class="brand-badge" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
            </svg>
          </div>
          <span class="brand-title">Home Hub</span>
        </div>
        <span class="library-count-pill" title={t('All music')}>
          {(music || []).length} {t('tracks')}
        </span>
      </div>

      <div class="sidebar-section-title">{t('Library')}</div>
      <nav class="sidebar-nav">
        <button
          class="nav-item"
          class:active={playlist === null && !showQueue}
          on:click={() => { playlist = null; showQueue = false; searchQuery = ''; showAddPicker = false; }}
        >
          <svg class="nav-icon" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M9 18V5l12-2v13"/>
            <circle cx="6" cy="18" r="3"/>
            <circle cx="18" cy="16" r="3"/>
          </svg>
          <span class="nav-label">{t('All music')}</span>
          <span class="nav-count">{(music || []).length}</span>
        </button>

        <button
          class="nav-item"
          class:active={showQueue}
          on:click={() => showQueue = !showQueue}
        >
          <svg class="nav-icon" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/>
            <circle cx="4" cy="6" r="1.5" fill="currentColor"/><circle cx="4" cy="12" r="1.5" fill="currentColor"/><circle cx="4" cy="18" r="1.5" fill="currentColor"/>
          </svg>
          <span class="nav-label">{t("Up next")}</span>
          {#if queueCount > 0}
            <span class="queue-nav-badge">{queueCount}</span>
          {/if}
        </button>
      </nav>

      {#if !native}
        <div class="sidebar-notice">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>
          </svg>
          <span>{t('Private playlists are available in the installed Windows app.')}</span>
        </div>
      {/if}

      <div class="sidebar-section-header">
        <span class="sidebar-section-title">{t('Playlists')}</span>
        {#if native}
          <button class="section-action-btn" on:click={triggerNewPlaylist} title={t('New playlist')}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.5">
              <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
            <span>{t('New')}</span>
          </button>
        {/if}
      </div>

      <div class="sidebar-playlists" role="list">
        {#each playlists as row (row.id)}
          {@const pCount = (json(row.body, { ids: [] }).ids || []).length}
          <div class="playlist-row" class:active={playlist?.id === row.id}>
            <button
              class="playlist-link"
              on:click={() => { playlist = row; showQueue = false; searchQuery = ''; showAddPicker = false; }}
              title={row.title}
            >
              <svg class="playlist-icon" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15V6M18.5 18a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM12 12H3M16 6H3M12 18H3"/>
              </svg>
              <span class="playlist-title">{row.title}</span>
              <span class="playlist-count">{pCount}</span>
            </button>
            <button
              class="playlist-del-btn"
              on:click|stopPropagation={() => deletePlaylist(row)}
              title={`${t('Remove')} ${row.title}`}
              aria-label={`${t('Remove')} ${row.title}`}
            >
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
              </svg>
            </button>
          </div>
        {:else}
          <div class="sidebar-empty">
            <p>{t('No playlists yet')}</p>
            {#if native}
              <button class="empty-new-btn" on:click={triggerNewPlaylist}>+ {t('Create playlist')}</button>
            {/if}
          </div>
        {/each}
      </div>
    </aside>

    <!-- Main Content View -->
    <main class="music-main" role="region" aria-label={t('All music')}>
      <!-- Header Hero Banner -->
      <header class="banner-hero">
        <div class="hero-art-container">
          <div class="hero-art" aria-hidden="true">
            <div class="vinyl-outer-groove"></div>
            <div class="vinyl-groove ring-1"></div>
            <div class="vinyl-groove ring-2"></div>
            <div class="vinyl-groove ring-3"></div>
            <div class="hero-art-center">
              <svg viewBox="0 0 24 24" width="28" height="28" fill="#FFFFFF">
                <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
              </svg>
            </div>
          </div>
        </div>

        <div class="hero-details">
          <div class="hero-eyebrow">
            {playlist ? t('Playlist') : t('All tracks')}
          </div>
          <h1 class="hero-title">
            {playlist ? playlist.title : t('All music')}
          </h1>
          <div class="hero-meta">
            <span>{musicTracks.length} {t('tracks')}</span>
            <span class="meta-dot">·</span>
            <span>{bytes(totalBytes)}</span>
            {#if playlist}
              <span class="meta-dot">·</span>
              <span class="meta-badge">{t('Playlist')}</span>
            {/if}
          </div>

          <div class="hero-actions">
            <button class="action-btn-primary" on:click={playAll} disabled={musicTracks.length === 0} title={t('Play')}>
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                <polygon points="5 3 19 12 5 21 5 3"/>
              </svg>
              <span>{t('Play')}</span>
            </button>

            <button class="action-btn-secondary" on:click={playShuffle} disabled={musicTracks.length === 0} title={t('Shuffle')}>
              <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/>
              </svg>
              <span>{t('Shuffle')}</span>
            </button>

            {#if playlist}
              <button
                class="action-btn-secondary"
                class:active={showAddPicker}
                on:click={() => showAddPicker = !showAddPicker}
                title={t('Add to playlist')}
              >
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2">
                  <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
                <span>{t('Add tracks')} ({availableForPlaylist.length})</span>
              </button>

              <button
                class="action-btn-danger"
                on:click={() => deletePlaylist(playlist)}
                title={`${t('Remove')} ${playlist.title}`}
              >
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2">
                  <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
                </svg>
                <span>{t('Remove')}</span>
              </button>
            {/if}

            <div class="search-box">
              <svg class="search-icon" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2">
                <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
              </svg>
              <input
                type="search"
                class="search-input"
                placeholder={t('Search tracks…')}
                bind:value={searchQuery}
                aria-label={t('Search tracks…')}
              />
              {#if searchQuery}
                <button class="search-clear" on:click={() => searchQuery = ''} aria-label={t('Close')}>×</button>
              {/if}
            </div>
          </div>
        </div>
      </header>

      <!-- Add to Playlist Picker Section -->
      {#if playlist && showAddPicker}
        <section class="picker-panel" aria-label={t('Add to playlist')}>
          <div class="picker-header">
            <div>
              <h3>{t('Add to playlist')}</h3>
              <p>{t('Select tracks from your Home Hub library to add to')} {playlist.title}.</p>
            </div>
            <button class="picker-close" on:click={() => showAddPicker = false} aria-label={t('Close')}>×</button>
          </div>
          {#if availableForPlaylist.length > 0}
            <div class="picker-list">
              {#each availableForPlaylist as track (track.id)}
                <div class="picker-row">
                  <div class="picker-title">
                    <strong>{cleanTitle(track.name)}</strong>
                    <span class="picker-ext">{getExt(track.name)}</span>
                    <small>{bytes(track.size)}</small>
                  </div>
                  <button class="picker-add-btn" on:click={() => run(() => addTrack(track))}>
                    + {t('Add')}
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <p class="picker-empty">{t('All music tracks are already in this playlist.')}</p>
          {/if}
        </section>
      {/if}

      <!-- Tracklist / Empty State -->
      <section class="tracklist-container">
        {#if filteredTracks.length > 0}
          <table class="track-table">
            <thead>
              <tr>
                <th class="th-num">#</th>
                <th class="th-title">{t('Title')}</th>
                <th class="th-size">{t('Size')}</th>
                <th class="th-actions">{t('Actions')}</th>
              </tr>
            </thead>
            <tbody>
              {#each filteredTracks as track, index (track.id)}
                {@const isCurrent = playing?.id === track.id}
                <tr class="track-row" class:is-playing={isCurrent} on:dblclick={() => playTrack(track)}>
                  <td class="td-num">
                    <button class="play-indicator-btn" on:click|stopPropagation={() => isCurrent ? togglePlay() : playTrack(track)} aria-label={isCurrent && isPlaying ? t('Pause') : t('Play')}>
                      {#if isCurrent && isPlaying}
                        <div class="eq-bars" aria-hidden="true">
                          <span class="bar bar-1"></span>
                          <span class="bar bar-2"></span>
                          <span class="bar bar-3"></span>
                        </div>
                      {:else}
                        <span class="track-index">{index + 1}</span>
                        <svg class="hover-play-icon" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                          <polygon points="5 3 19 12 5 21 5 3"/>
                        </svg>
                      {/if}
                    </button>
                  </td>

                  <td class="td-title">
                    <div class="title-cell">
                      <button class="track-title-btn" on:click={() => playTrack(track)}>
                        <span class="track-name" class:active-title={isCurrent}>{cleanTitle(track.name)}</span>
                      </button>
                      <span class="ext-pill">{getExt(track.name)}</span>
                    </div>
                  </td>

                  <td class="td-size">
                    <span class="size-pill">{bytes(track.size)}</span>
                  </td>

                  <td class="td-actions">
                    <div class="row-actions">
                      <button
                        class="row-action-btn"
                        on:click|stopPropagation={() => enqueue(track)}
                        title={t("Add to queue")}
                        aria-label={t("Add to queue")}
                      >
                        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                          <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
                        </svg>
                      </button>

                      {#if playlist}
                        <button
                          class="row-action-btn danger"
                          on:click|stopPropagation={() => run(() => addTrack(track, true))}
                          title={t('Remove')}
                          aria-label={t('Remove')}
                        >
                          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                          </svg>
                        </button>
                      {/if}
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <div class="empty-state">
            <div class="empty-vinyl-disc" aria-hidden="true">
              <div class="empty-vinyl-groove g-1"></div>
              <div class="empty-vinyl-groove g-2"></div>
              <div class="empty-vinyl-groove g-3"></div>
              <div class="empty-vinyl-center">
                <svg viewBox="0 0 24 24" width="22" height="22" fill="#8B5CF6">
                  <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                </svg>
              </div>
            </div>
            {#if searchQuery}
              <h3>{t('Nothing here yet.')}</h3>
              <p>{t('No tracks match your search')} "{searchQuery}".</p>
              <button class="action-btn-secondary" on:click={() => searchQuery = ''}>{t('Clear search')}</button>
            {:else if playlist}
              <h3>{t('This playlist is empty')}</h3>
              <p>{t('Add tracks from your library to start listening.')}</p>
              {#if availableForPlaylist.length > 0}
                <button class="action-btn-primary" on:click={() => showAddPicker = true}>+ {t('Add tracks')}</button>
              {/if}
            {:else}
              <h3>{t('No music found')}</h3>
              <p>{t('Add audio files (.mp3, .flac, .wav, .m4a) to your Home Hub library.')}</p>
            {/if}
          </div>
        {/if}

        {#if cursor}
          <div class="pagination-footer">
            <button class="load-more-btn" on:click={() => run(more)}>
              {t('Load more')}
            </button>
          </div>
        {/if}
      </section>
    </main>

    <!-- Up Next Queue Slide-Over Drawer -->
    {#if showQueue}
      <aside class="queue-drawer" aria-label={t("Up next")}>
        <div class="queue-header">
          <div class="queue-header-left">
            <h3>{t("Up next")}</h3>
            <span class="queue-count-pill">{queueCount}</span>
          </div>
          <div class="queue-header-actions">
            {#if queueCount > 0}
              <button class="queue-clear-btn" on:click={clearQueue}>{t('Clear')}</button>
            {/if}
            <button class="queue-close-btn" on:click={() => showQueue = false} aria-label={t('Close')}>×</button>
          </div>
        </div>

        {#if playing}
          <div class="queue-section-label">{t('Playing')}</div>
          <div class="queue-now-playing">
            <div class="q-art" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
              </svg>
            </div>
            <div class="q-info">
              <strong>{cleanTitle(playing.name)}</strong>
              <small>{isPlaying ? t('Playing') : t('Paused')}</small>
            </div>
            <div class="eq-bars" aria-hidden="true">
              <span class="bar bar-1"></span>
              <span class="bar bar-2"></span>
              <span class="bar bar-3"></span>
            </div>
          </div>
        {/if}

        <div class="queue-section-label">{t('Queue')} ({queueCount})</div>
        <div class="queue-list">
          {#each queuedTracks as qTrack, qIndex}
            <div class="queue-item">
              <span class="q-index">{qIndex + 1}</span>
              <button class="q-play-btn" on:click={() => { playing = qTrack; dequeue(qIndex); }} title={t('Play')}>
                <span class="q-name">{cleanTitle(qTrack.name)}</span>
                <span class="q-size">{bytes(qTrack.size)}</span>
              </button>
              <button class="q-remove-btn" on:click={() => dequeue(qIndex)} title={t("Remove")} aria-label={t("Remove")}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>
            </div>
          {:else}
            <div class="queue-empty">
              <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
              </svg>
              <p>{t("Queue is empty")}</p>
              <small>{t('Click + next to any track to add it to your playback queue.')}</small>
            </div>
          {/each}
        </div>
      </aside>
    {/if}
  </div>

  <!-- Persistent Bottom Player Bar (Height 76px) -->
  <footer class="player-bar" aria-label={t('Player')}>
    <!-- Left: Now Playing Info (minmax 200px, 1fr) -->
    <div class="player-left">
      <div class="track-art-wrap" class:spinning={isPlaying && !!playing}>
        <div class="art-disc">
          <div class="disc-spindle"></div>
        </div>
      </div>
      <div class="track-meta">
        {#if playing}
          <strong class="now-playing-title" title={playing.name}>{cleanTitle(playing.name)}</strong>
          <div class="now-playing-status">
            <span class="status-indicator-dot" class:active={isPlaying}></span>
            <span>{isPlaying ? t('Playing') : t('Paused')}</span>
            <span class="status-sep">·</span>
            <span>{bytes(playing.size)}</span>
          </div>
        {:else}
          <strong class="now-playing-title muted">Home Hub Music</strong>
          <span class="now-playing-status muted">{t('Select a track to play')}</span>
        {/if}
      </div>
    </div>

    <!-- Center: Transport Controls & Scrubber (Fixed Dead-Center) -->
    <div class="player-center">
      <div class="transport-controls">
        <button
          class="ctrl-btn"
          class:active={shuffle}
          on:click={() => shuffle = !shuffle}
          title={t('Shuffle')}
          aria-label={t('Shuffle')}
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/>
          </svg>
          {#if shuffle}<span class="active-dot"></span>{/if}
        </button>

        <button
          class="ctrl-btn"
          on:click={() => nextTrack(-1)}
          disabled={!playing && musicTracks.length === 0}
          title={t('Previous')}
          aria-label={t('Previous')}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
            <polygon points="19 20 9 12 19 4 19 20"/><rect x="5" y="4" width="2.5" height="16" rx="1"/>
          </svg>
        </button>

        <button
          class="play-toggle-btn"
          on:click={togglePlay}
          title={isPlaying ? t('Pause') : t('Play')}
          aria-label={isPlaying ? t('Pause') : t('Play')}
        >
          {#if isPlaying}
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
              <rect x="6" y="4" width="4" height="16" rx="1.5"/><rect x="14" y="4" width="4" height="16" rx="1.5"/>
            </svg>
          {:else}
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" style="margin-left: 2px;">
              <polygon points="6 4 20 12 6 20 6 4"/>
            </svg>
          {/if}
        </button>

        <button
          class="ctrl-btn"
          on:click={() => nextTrack(1)}
          disabled={!playing && musicTracks.length === 0}
          title={t('Next')}
          aria-label={t('Next')}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
            <polygon points="5 4 15 12 5 20 5 4"/><rect x="16.5" y="4" width="2.5" height="16" rx="1"/>
          </svg>
        </button>

        <button
          class="ctrl-btn"
          class:active={repeat}
          on:click={() => repeat = !repeat}
          title={t('Repeat')}
          aria-label={t('Repeat')}
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 014-4h14M7 23l-4-4 4-4"/><path d="M21 13v2a4 4 0 01-4 4H3"/>
          </svg>
          {#if repeat}<span class="active-dot"></span>{/if}
        </button>
      </div>

      <div class="scrubber-row">
        <span class="time-label">{formatTime(currentTime)}</span>
        <div class="slider-wrapper scrubber-wrapper">
          <input
            type="range"
            class="scrubber-slider"
            min="0"
            max={duration || 100}
            step="0.1"
            value={currentTime}
            on:input={handleSeek}
            aria-label={t('Playback progress')}
          />
        </div>
        <span class="time-label">{formatTime(duration)}</span>
      </div>
    </div>

    <!-- Right: Queue Toggle & Volume Controls (minmax 200px, 1fr) -->
    <div class="player-right">
      <button
        class="queue-bar-btn"
        class:active={showQueue}
        on:click={() => showQueue = !showQueue}
        title={t("Up next")}
        aria-label={t("Up next")}
      >
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/>
          <circle cx="4" cy="6" r="1.5" fill="currentColor"/><circle cx="4" cy="12" r="1.5" fill="currentColor"/><circle cx="4" cy="18" r="1.5" fill="currentColor"/>
        </svg>
        {#if queueCount > 0}
          <span class="bar-badge">{queueCount}</span>
        {/if}
      </button>

      <div class="volume-container">
        <button
          class="vol-icon-btn"
          on:click={toggleMute}
          title={isMuted ? t('Unmute') : t('Mute')}
          aria-label={isMuted ? t('Unmute') : t('Mute')}
        >
          {#if isMuted || volume === 0}
            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><line x1="23" y1="9" x2="17" y2="15"/><line x1="17" y1="9" x2="23" y2="15"/>
            </svg>
          {:else if volume < 0.5}
            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 010 7.07"/>
            </svg>
          {:else}
            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/>
            </svg>
          {/if}
        </button>

        <div class="slider-wrapper volume-wrapper">
          <input
            type="range"
            class="volume-slider"
            min="0"
            max="1"
            step="0.01"
            value={volume}
            on:input={handleVolumeInput}
            aria-label={t('Volume')}
          />
        </div>
      </div>
    </div>
  </footer>
</div>

<style>
  .music-app {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #0B0E14;
    color: #e6edf3;
    position: relative;
    overflow: hidden;
    font-family: inherit;
    user-select: none;
  }

  .music-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    position: relative;
  }

  /* ------------------------------------------------------------- */
  /* Left Sidebar (230px)                                          */
  /* ------------------------------------------------------------- */
  .music-sidebar {
    width: 230px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: rgba(14, 18, 25, 0.88);
    border-right: 1px solid rgba(255, 255, 255, 0.07);
    backdrop-filter: blur(24px);
    padding: 20px 14px 16px;
    gap: 12px;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .sidebar-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 6px;
  }

  .sidebar-brand {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .brand-badge {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: linear-gradient(135deg, #8B5CF6 0%, #6D28D9 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #FFFFFF;
    box-shadow: 0 2px 8px rgba(139, 92, 246, 0.4);
  }

  .brand-title {
    font-size: 13px;
    font-weight: 700;
    color: #FFFFFF;
    letter-spacing: -0.01em;
  }

  .library-count-pill {
    font-size: 11px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.55);
  }

  .sidebar-section-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.38);
    padding: 8px 8px 2px;
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: rgba(255, 255, 255, 0.75);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: all 0.15s ease;
    width: 100%;
  }

  .nav-item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #FFFFFF;
  }

  .nav-item.active {
    background: rgba(139, 92, 246, 0.15);
    border-color: rgba(139, 92, 246, 0.3);
    color: #C4B5FD;
    font-weight: 600;
  }

  .nav-icon {
    flex-shrink: 0;
    color: currentColor;
  }

  .nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nav-count {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    font-weight: 500;
  }

  .queue-nav-badge {
    font-size: 10px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 8px;
    background: #8B5CF6;
    color: #FFFFFF;
  }

  .sidebar-notice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.5);
    font-size: 11px;
    line-height: 1.4;
    margin: 4px 0;
  }

  .sidebar-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .sidebar-section-header .sidebar-section-title {
    padding: 0;
  }

  .section-action-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(139, 92, 246, 0.12);
    border: 1px solid rgba(139, 92, 246, 0.25);
    color: #C4B5FD;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    padding: 2px 7px;
    border-radius: 12px;
    transition: all 0.15s ease;
  }

  .section-action-btn:hover {
    background: #8B5CF6;
    color: #FFFFFF;
  }

  .sidebar-playlists {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-height: 60px;
    overflow-y: auto;
  }

  .playlist-row {
    display: flex;
    align-items: center;
    border-radius: 8px;
    padding: 1px 4px;
    position: relative;
    transition: background 0.15s ease;
  }

  .playlist-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .playlist-row.active {
    background: rgba(139, 92, 246, 0.15);
    box-shadow: inset 0 0 0 1px rgba(139, 92, 246, 0.3);
  }

  .playlist-link {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.8);
    padding: 7px 4px;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }

  .playlist-row.active .playlist-link {
    color: #C4B5FD;
    font-weight: 600;
  }

  .playlist-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .playlist-count {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    font-weight: 500;
    margin-right: 4px;
  }

  .playlist-del-btn {
    opacity: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: rgba(255, 255, 255, 0.4);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .playlist-row:hover .playlist-del-btn,
  .playlist-row:focus-within .playlist-del-btn {
    opacity: 1;
  }

  .playlist-del-btn:hover {
    background: rgba(237, 129, 123, 0.2);
    color: #ED817B;
  }

  .sidebar-empty {
    padding: 16px 8px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .sidebar-empty p {
    margin: 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.4);
  }

  .empty-new-btn {
    font-size: 11px;
    font-weight: 600;
    color: #C4B5FD;
    background: rgba(139, 92, 246, 0.1);
    border: 1px solid rgba(139, 92, 246, 0.2);
    border-radius: 12px;
    padding: 3px 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .empty-new-btn:hover {
    background: #8B5CF6;
    color: #FFFFFF;
  }

  /* ------------------------------------------------------------- */
  /* Main Content Area                                             */
  /* ------------------------------------------------------------- */
  .music-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    background: radial-gradient(ellipse 650px 320px at 80% 0%, rgba(139, 92, 246, 0.08) 0%, transparent 70%), #0B0E14;
    padding: 28px 32px;
    gap: 28px;
  }

  /* Header Hero */
  .banner-hero {
    display: flex;
    align-items: center;
    gap: 24px;
    flex-wrap: wrap;
  }

  .hero-art-container {
    flex-shrink: 0;
  }

  .hero-art {
    width: 112px;
    height: 112px;
    border-radius: 22px;
    background: linear-gradient(135deg, #5B21B6 0%, #7C3AED 50%, #9333EA 100%);
    box-shadow: 0 16px 36px -8px rgba(109, 40, 217, 0.45), inset 0 0 0 1px rgba(255, 255, 255, 0.2);
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .vinyl-outer-groove {
    position: absolute;
    width: 96px;
    height: 96px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.12);
    pointer-events: none;
  }

  .vinyl-groove {
    position: absolute;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.08);
    pointer-events: none;
  }

  .ring-1 { width: 76px; height: 76px; }
  .ring-2 { width: 56px; height: 56px; }
  .ring-3 { width: 38px; height: 38px; }

  .hero-art-center {
    position: relative;
    z-index: 2;
    filter: drop-shadow(0 2px 8px rgba(0, 0, 0, 0.35));
  }

  .hero-details {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 260px;
  }

  .hero-eyebrow {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #A78BFA;
  }

  .hero-title {
    font-size: 32px;
    font-weight: 800;
    margin: 0;
    line-height: 1.15;
    color: #FFFFFF;
    letter-spacing: -0.025em;
  }

  .hero-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.55);
    margin-top: 2px;
  }

  .meta-dot {
    opacity: 0.4;
  }

  .meta-badge {
    padding: 1px 7px;
    border-radius: 6px;
    background: rgba(139, 92, 246, 0.2);
    color: #C4B5FD;
    font-size: 11px;
    font-weight: 600;
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 14px;
    flex-wrap: wrap;
  }

  .action-btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 20px;
    border-radius: 19px;
    background: linear-gradient(135deg, #8B5CF6 0%, #7C3AED 100%);
    color: #FFFFFF;
    border: none;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 14px rgba(139, 92, 246, 0.4);
    transition: all 0.15s ease;
  }

  .action-btn-primary:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 6px 20px rgba(139, 92, 246, 0.55);
  }

  .action-btn-primary:disabled {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.35);
    box-shadow: none;
    cursor: not-allowed;
  }

  .action-btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 16px;
    border-radius: 19px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .action-btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }

  .action-btn-secondary.active {
    background: rgba(139, 92, 246, 0.22);
    border-color: rgba(139, 92, 246, 0.4);
    color: #C4B5FD;
  }

  .action-btn-secondary:disabled {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.3);
    cursor: not-allowed;
  }

  .action-btn-danger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 38px;
    padding: 0 14px;
    border-radius: 19px;
    background: rgba(237, 129, 123, 0.12);
    border: 1px solid rgba(237, 129, 123, 0.22);
    color: #ED817B;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .action-btn-danger:hover {
    background: rgba(237, 129, 123, 0.22);
  }

  .search-box {
    display: flex;
    align-items: center;
    position: relative;
    margin-left: auto;
    width: 220px;
  }

  .search-icon {
    position: absolute;
    left: 12px;
    color: rgba(255, 255, 255, 0.45);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    height: 36px;
    padding: 0 28px 0 34px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #FFFFFF;
    font-size: 13px;
    outline: none;
    transition: all 0.15s ease;
  }

  .search-input:focus {
    background: rgba(255, 255, 255, 0.09);
    border-color: rgba(167, 139, 250, 0.5);
    box-shadow: 0 0 0 3px rgba(167, 139, 250, 0.15);
  }

  .search-clear {
    position: absolute;
    right: 10px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    font-size: 14px;
    cursor: pointer;
    padding: 2px 4px;
  }

  /* Picker Panel */
  .picker-panel {
    background: rgba(18, 22, 30, 0.9);
    border: 1px solid rgba(139, 92, 246, 0.3);
    border-radius: 14px;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  }

  .picker-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
  }

  .picker-header h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .picker-header p {
    margin: 3px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
  }

  .picker-close {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.6);
    font-size: 18px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .picker-close:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #FFFFFF;
  }

  .picker-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 8px;
    max-height: 200px;
    overflow-y: auto;
  }

  .picker-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 12px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .picker-title {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    font-size: 12px;
  }

  .picker-title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 140px;
  }

  .picker-ext {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.6);
  }

  .picker-title small {
    color: rgba(255, 255, 255, 0.4);
  }

  .picker-add-btn {
    background: rgba(139, 92, 246, 0.2);
    border: 1px solid rgba(139, 92, 246, 0.4);
    color: #C4B5FD;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .picker-add-btn:hover {
    background: #8B5CF6;
    color: #FFFFFF;
  }

  .picker-empty {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    margin: 0;
  }

  /* Tracklist Table */
  .tracklist-container {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .track-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    text-align: left;
  }

  .track-table thead th {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.4);
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .th-num { width: 48px; text-align: center; }
  .th-size { width: 90px; text-align: right; }
  .th-actions { width: 90px; text-align: right; }

  .track-row {
    border-radius: 8px;
    transition: background 0.15s ease;
  }

  .track-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .track-row.is-playing {
    background: rgba(139, 92, 246, 0.14);
  }

  .track-row td {
    padding: 9px 14px;
    vertical-align: middle;
  }

  .td-num {
    text-align: center;
  }

  .play-indicator-btn {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.45);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    transition: all 0.15s ease;
  }

  .track-row:hover .play-indicator-btn {
    color: #FFFFFF;
  }

  .hover-play-icon {
    display: none;
  }

  .track-row:hover .track-index {
    display: none;
  }

  .track-row:hover .hover-play-icon {
    display: block;
    color: #C4B5FD;
  }

  /* Equalizer Animation */
  .eq-bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 14px;
    width: 14px;
  }

  .eq-bars .bar {
    flex: 1;
    background: #8B5CF6;
    border-radius: 1px;
    animation: eq-bounce 0.8s ease-in-out infinite alternate;
  }

  .eq-bars .bar-1 { animation-delay: 0s; height: 35%; }
  .eq-bars .bar-2 { animation-delay: 0.3s; height: 95%; }
  .eq-bars .bar-3 { animation-delay: 0.6s; height: 55%; }

  @keyframes eq-bounce {
    0% { height: 20%; }
    100% { height: 100%; }
  }

  .td-title {
    max-width: 0;
  }

  .title-cell {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .track-title-btn {
    background: transparent;
    border: none;
    padding: 0;
    font-size: 13px;
    font-weight: 500;
    color: #FFFFFF;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: calc(100% - 60px);
  }

  .track-name.active-title {
    color: #C4B5FD;
    font-weight: 600;
  }

  .ext-pill {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.5);
    flex-shrink: 0;
  }

  .td-size {
    text-align: right;
  }

  .size-pill {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.45);
    font-variant-numeric: tabular-nums;
  }

  .td-actions {
    text-align: right;
  }

  .row-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }

  .row-action-btn {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.4);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .track-row:hover .row-action-btn {
    opacity: 1;
  }

  .row-action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #FFFFFF;
  }

  .row-action-btn.danger:hover {
    background: rgba(237, 129, 123, 0.2);
    color: #ED817B;
  }

  /* Empty state */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 24px;
    text-align: center;
    gap: 6px;
    color: rgba(255, 255, 255, 0.6);
  }

  .empty-vinyl-disc {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: radial-gradient(circle, #221538 0%, #130a21 50%, #0c0617 100%);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6), inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 12px;
  }

  .empty-vinyl-groove {
    position: absolute;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .g-1 { width: 56px; height: 56px; }
  .g-2 { width: 42px; height: 42px; }
  .g-3 { width: 28px; height: 28px; }

  .empty-vinyl-center {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2;
  }

  .empty-state h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    color: #FFFFFF;
  }

  .empty-state p {
    margin: 2px 0 12px;
    font-size: 13px;
    max-width: 340px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.5);
  }

  .pagination-footer {
    display: flex;
    justify-content: center;
    padding: 20px 0;
  }

  .load-more-btn {
    height: 36px;
    padding: 0 22px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .load-more-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  /* ------------------------------------------------------------- */
  /* Up Next Queue Drawer                                          */
  /* ------------------------------------------------------------- */
  .queue-drawer {
    width: 320px;
    flex-shrink: 0;
    background: rgba(13, 16, 23, 0.96);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(28px);
    display: flex;
    flex-direction: column;
    padding: 18px;
    gap: 14px;
    box-shadow: -10px 0 32px rgba(0, 0, 0, 0.5);
    z-index: 10;
  }

  .queue-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .queue-header-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .queue-header-left h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.8);
  }

  .queue-count-pill {
    font-size: 11px;
    font-weight: 700;
    padding: 1px 7px;
    border-radius: 10px;
    background: rgba(139, 92, 246, 0.25);
    color: #C4B5FD;
  }

  .queue-header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .queue-clear-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    font-size: 12px;
    cursor: pointer;
    padding: 3px 7px;
    border-radius: 4px;
  }

  .queue-clear-btn:hover {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .queue-close-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    font-size: 17px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .queue-close-btn:hover {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .queue-section-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.4);
    margin-top: 4px;
  }

  .queue-now-playing {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 10px;
    background: rgba(139, 92, 246, 0.15);
    border: 1px solid rgba(139, 92, 246, 0.3);
  }

  .q-art {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: #8B5CF6;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #FFFFFF;
    flex-shrink: 0;
  }

  .q-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .q-info strong {
    font-size: 12px;
    color: #FFFFFF;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .q-info small {
    font-size: 11px;
    color: #C4B5FD;
  }

  .queue-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .queue-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    transition: background 0.15s ease;
  }

  .queue-item:hover {
    background: rgba(255, 255, 255, 0.07);
  }

  .q-index {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    width: 16px;
    text-align: center;
  }

  .q-play-btn {
    background: transparent;
    border: none;
    flex: 1;
    min-width: 0;
    text-align: left;
    cursor: pointer;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .q-name {
    font-size: 12px;
    color: #FFFFFF;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .q-size {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.4);
  }

  .q-remove-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.4);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
  }

  .q-remove-btn:hover {
    color: #ED817B;
    background: rgba(237, 129, 123, 0.15);
  }

  .queue-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 36px 16px;
    text-align: center;
    color: rgba(255, 255, 255, 0.4);
    gap: 6px;
  }

  .queue-empty p {
    margin: 4px 0 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
  }

  .queue-empty small {
    font-size: 11px;
    line-height: 1.4;
  }

  /* ------------------------------------------------------------- */
  /* Persistent Bottom Player Bar (Height 76px)                     */
  /* ------------------------------------------------------------- */
  .player-bar {
    height: 76px;
    flex-shrink: 0;
    background: rgba(14, 17, 24, 0.94);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(28px);
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(360px, 560px) minmax(180px, 1fr);
    align-items: center;
    padding: 0 24px;
    z-index: 20;
    gap: 16px;
  }

  /* Left Player Area */
  .player-left {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    justify-self: start;
  }

  .track-art-wrap {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: #11151c;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5), inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    position: relative;
  }

  .art-disc {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: radial-gradient(circle, #25123d 0%, #150926 40%, #0d0617 100%);
    box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.08), inset 0 0 0 6px rgba(0, 0, 0, 0.5), inset 0 0 0 10px rgba(139, 92, 246, 0.15);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .disc-spindle {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #8B5CF6;
    border: 2px solid #0c1015;
    box-shadow: 0 0 6px rgba(139, 92, 246, 0.6);
  }

  .track-art-wrap.spinning .art-disc {
    animation: spin-vinyl 10s linear infinite;
  }

  @keyframes spin-vinyl {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .track-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 2px;
  }

  .now-playing-title {
    font-size: 13px;
    font-weight: 600;
    color: #FFFFFF;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }

  .now-playing-title.muted {
    color: rgba(255, 255, 255, 0.5);
  }

  .now-playing-status {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.6);
  }

  .now-playing-status.muted {
    color: rgba(255, 255, 255, 0.35);
  }

  .status-indicator-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.3);
  }

  .status-indicator-dot.active {
    background: #8B5CF6;
    box-shadow: 0 0 6px #8B5CF6;
  }

  .status-sep {
    opacity: 0.4;
  }

  /* Center Player Area (Dead Center) */
  .player-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
  }

  .transport-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
  }

  .ctrl-btn {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    position: relative;
    transition: all 0.15s ease;
    padding: 0;
  }

  .ctrl-btn:hover:not(:disabled) {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .ctrl-btn.active {
    color: #C4B5FD;
  }

  .ctrl-btn:disabled {
    color: rgba(255, 255, 255, 0.22);
    cursor: not-allowed;
  }

  .active-dot {
    position: absolute;
    bottom: 3px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: #C4B5FD;
  }

  .play-toggle-btn {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: linear-gradient(135deg, #8B5CF6 0%, #7C3AED 100%);
    border: none;
    color: #FFFFFF;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 4px 16px rgba(139, 92, 246, 0.45);
    transition: all 0.15s ease;
    padding: 0;
  }

  .play-toggle-btn:hover {
    transform: scale(1.06);
    box-shadow: 0 6px 22px rgba(139, 92, 246, 0.6);
  }

  .play-toggle-btn:active {
    transform: scale(0.97);
  }

  .scrubber-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    max-width: 480px;
  }

  .time-label {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.45);
    font-variant-numeric: tabular-nums;
    width: 34px;
    text-align: center;
  }

  .slider-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .scrubber-wrapper {
    flex: 1;
  }

  /* Range Sliders Reset */
  .scrubber-slider,
  .volume-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 16px;
    background: transparent;
    border: none;
    outline: none;
    cursor: pointer;
    margin: 0;
    padding: 0;
  }

  /* Scrubber WebKit Track */
  .scrubber-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.16);
    border: none;
    transition: height 0.12s ease, background 0.12s ease;
  }

  .scrubber-slider:hover::-webkit-slider-runnable-track {
    height: 6px;
    background: rgba(255, 255, 255, 0.25);
  }

  /* Scrubber WebKit Thumb */
  .scrubber-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #FFFFFF;
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.45);
    margin-top: -4px;
    border: none;
    transition: transform 0.12s ease, background 0.12s ease;
  }

  .scrubber-slider:hover::-webkit-slider-thumb {
    background: #C4B5FD;
    transform: scale(1.2);
  }

  /* Scrubber Moz Track & Thumb */
  .scrubber-slider::-moz-range-track {
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.16);
    border: none;
    transition: height 0.12s ease, background 0.12s ease;
  }

  .scrubber-slider:hover::-moz-range-track {
    height: 6px;
    background: rgba(255, 255, 255, 0.25);
  }

  .scrubber-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #FFFFFF;
    border: none;
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.45);
  }

  /* Right Player Area */
  .player-right {
    display: flex;
    align-items: center;
    justify-self: end;
    gap: 14px;
  }

  .queue-bar-btn {
    position: relative;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.15s ease;
    padding: 0;
  }

  .queue-bar-btn:hover {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .queue-bar-btn.active {
    color: #C4B5FD;
    background: rgba(139, 92, 246, 0.2);
  }

  .bar-badge {
    position: absolute;
    top: 2px;
    right: 2px;
    font-size: 9px;
    font-weight: 700;
    background: #8B5CF6;
    color: #FFFFFF;
    border-radius: 6px;
    padding: 1px 4px;
    line-height: 1;
  }

  .volume-container {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .vol-icon-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    padding: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    transition: all 0.15s ease;
  }

  .vol-icon-btn:hover {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .volume-wrapper {
    width: 86px;
  }

  /* Volume WebKit Track */
  .volume-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.16);
    border: none;
    transition: background 0.12s ease;
  }

  .volume-slider:hover::-webkit-slider-runnable-track {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Volume WebKit Thumb */
  .volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #FFFFFF;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
    margin-top: -3px;
    border: none;
    transition: transform 0.12s ease;
  }

  .volume-slider:hover::-webkit-slider-thumb {
    transform: scale(1.2);
  }

  /* Volume Moz Track & Thumb */
  .volume-slider::-moz-range-track {
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.16);
    border: none;
  }

  .volume-slider::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #FFFFFF;
    border: none;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  }
</style>
