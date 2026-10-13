<script>
  export let data;
  export let form;
  export let formValue;
  export let t;
  export let date;
  export let mutate;
  export let confirm;
  export let changeScope;
  export let pair = () => {};
  export let load = () => {};
  export let busy = false;

  $: devices = data?.devices || [];
  $: activity = data?.activity || [];

  const permissionScopes = [
    {
      key: 'files',
      label: 'Files',
      desc: 'Read & download storage files',
      accent: 'blue'
    },
    {
      key: 'transfer',
      label: 'Transfers',
      desc: 'Upload photos and backups',
      accent: 'teal'
    },
    {
      key: 'photos',
      label: 'Photos',
      desc: 'Timeline & albums access',
      accent: 'purple'
    },
    {
      key: 'remote',
      label: 'Remote',
      desc: 'Cast screen & control input',
      accent: 'orange'
    },
    {
      key: 'admin',
      label: 'Admin',
      desc: 'Settings & device management',
      accent: 'indigo'
    }
  ];

  const actionFriendlyLabels = {
    pair: 'Device paired',
    pair_window_open: 'Pairing session started',
    revoke: 'Device revoked',
    device_permissions: 'Permissions updated',
    settings_updated: 'Settings updated',
    sharing_pause: 'Sharing paused',
    hotspot_toggle: 'Hotspot toggled',
    remote_wake: 'Remote wake request'
  };

  function hasScope(device, scopeKey) {
    const scopes = Array.isArray(device?.scopes)
      ? device.scopes
      : String(device?.scopes || '').split(',').filter(Boolean);
    return scopes.includes(scopeKey);
  }

  function handleToggleScope(device, scopeKey) {
    if (device.status === 'revoked') return;
    const current = hasScope(device, scopeKey);
    changeScope(device, scopeKey, !current);
  }

  function getDeviceType(platform = '') {
    const p = String(platform || '').toLowerCase();
    if (p.includes('tablet') || p.includes('ipad')) return 'tablet';
    if (p.includes('mac') || p.includes('windows') || p.includes('linux') || p.includes('desktop') || p.includes('pc')) return 'desktop';
    return 'phone';
  }

  function getDeviceCountLabel(list) {
    const count = list.length;
    if (count === 0) return `0 ${t('Devices')}`;
    if (count === 1) {
      const type = getDeviceType(list[0]?.platform);
      if (type === 'phone') return `1 ${t('Paired Phone')}`;
      return `1 ${t('Paired Device')}`;
    }
    return `${count} ${t('Paired Devices')}`;
  }

  function formatSmartDate(value) {
    if (!value) return '—';
    try {
      const d = new Date(value);
      if (isNaN(d.getTime())) return date(value);
      const now = new Date();
      const isToday = d.toDateString() === now.toDateString();
      const yesterday = new Date(now);
      yesterday.setDate(now.getDate() - 1);
      const isYesterday = d.toDateString() === yesterday.toDateString();
      const timeStr = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
      if (isToday) return `${t('Today')}, ${timeStr}`;
      if (isYesterday) return `${t('Yesterday')}, ${timeStr}`;
      const datePart = d.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
      return `${datePart} • ${timeStr}`;
    } catch {
      return date(value);
    }
  }

  function getActionLabel(action = '') {
    const translated = t(action);
    if (translated && translated !== action) {
      return translated;
    }
    const friendly = actionFriendlyLabels[action];
    if (friendly) {
      const translatedFriendly = t(friendly);
      return translatedFriendly || friendly;
    }
    return action.replace(/_/g, ' ');
  }

  function getActivityIconType(action = '') {
    const act = String(action || '').toLowerCase();
    if (act.includes('pair')) return 'pair';
    if (act.includes('permission') || act.includes('scope')) return 'permission';
    if (act.includes('revoke') || act.includes('delete')) return 'revoke';
    if (act.includes('setting')) return 'settings';
    return 'info';
  }
</script>

<div class="devices-app">
  <!-- Integrated App Header (Opaque solid backing to avoid scroll overlap bleed) -->
  <header class="devices-header">
    <div class="header-top-nav">
      <div class="nav-left">
        <div class="title-row">
          <h1 class="hub-title">{t('Connected Devices')}</h1>
          <span class="device-count-pill">
            <span class="status-dot-active"></span>
            {getDeviceCountLabel(devices)}
          </span>
        </div>
        <p class="hub-subtitle">{t('Manage paired phones, tablets, access permissions, and mTLS security.')}</p>
        <div class="security-notice-pill">
          <svg class="pill-lock-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
          <span>{t('Zero-Knowledge Local CA • End-to-End Encrypted mTLS')}</span>
        </div>
      </div>
      <div class="nav-right-actions">
        <button
          type="button"
          class="action-btn secondary-btn"
          disabled={busy}
          on:click={load}
          title={t('Refresh')}
          aria-label={t('Refresh')}
        >
          <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.19"/>
          </svg>
          <span>{t('Refresh')}</span>
        </button>
        <button
          type="button"
          class="action-btn primary-pair-btn"
          disabled={busy}
          on:click={pair}
          title={t('Pair a device')}
          aria-label={t('Pair a device')}
        >
          <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          <span>{t('Pair a device')}</span>
        </button>
      </div>
    </div>
  </header>

  <!-- Scrollable Hub Content Area -->
  <div class="devices-scroll-area">
    {#if devices.length === 0}
      <!-- Empty Devices State -->
      <section class="empty-devices-canvas">
        <div class="empty-graphic-wrap" aria-hidden="true">
          <div class="radar-circle radar-outer"></div>
          <div class="radar-circle radar-middle"></div>
          <div class="radar-circle radar-inner"></div>
          <div class="empty-phone-icon">
            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <rect x="5" y="2" width="14" height="20" rx="3" ry="3"/>
              <path d="M12 18h.01"/>
            </svg>
          </div>
        </div>
        <h2 class="empty-title">{t('No devices connected yet')}</h2>
        <p class="empty-subtitle">{t('Pair your smartphone or tablet using a QR code to automatically sync photos and stream media.')}</p>
        <button type="button" class="empty-pair-btn" disabled={busy} on:click={pair}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          <span>{t('Pair your first device')}</span>
        </button>
      </section>
    {:else}
      <!-- Hardware Device Cards -->
      <div class="device-cards-list">
        {#each devices as device (device.id)}
          {@const isRevoked = device.status === 'revoked'}
          <article class="device-card" class:is-revoked={isRevoked}>
            <div class="device-card-header">
              <div class="device-header-left">
                <div class="device-avatar" class:online={!isRevoked} class:offline={isRevoked} aria-hidden="true">
                  {#if getDeviceType(device.platform) === 'tablet'}
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <rect x="4" y="2" width="16" height="20" rx="3" ry="3"/>
                      <circle cx="12" cy="18" r="1"/>
                    </svg>
                  {:else if getDeviceType(device.platform) === 'desktop'}
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <rect x="2" y="3" width="20" height="14" rx="2"/>
                      <path d="M8 21h8M12 17v4"/>
                    </svg>
                  {:else}
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <rect x="5" y="2" width="14" height="20" rx="3" ry="3"/>
                      <path d="M12 18h.01"/>
                    </svg>
                  {/if}
                  <span
                    class="device-beacon"
                    class:beacon-online={!isRevoked}
                    class:beacon-revoked={isRevoked}
                    title={isRevoked ? t('revoked') : t('Connected')}
                  ></span>
                </div>
                <div class="device-identity">
                  <div class="device-name-row">
                    <h2 class="device-name">{device.name}</h2>
                    {#if isRevoked}
                      <span class="status-chip chip-revoked">{t('revoked')}</span>
                    {:else}
                      <span class="status-chip chip-active">
                        <span class="chip-dot"></span>
                        {t('Connected')}
                      </span>
                    {/if}
                  </div>
                  <div class="device-meta-chips">
                    <span class="meta-chip">{device.platform || 'Device'}</span>
                    <span class="meta-chip last-seen">{t('Last seen')}: {date(device.last_seen_at)}</span>
                  </div>
                </div>
              </div>

              <div class="device-header-actions">
                <button
                  type="button"
                  class="pill-action-btn rename-btn"
                  title={t('Rename')}
                  on:click={() => { form = { action: 'device', file: device }; formValue = device.name; }}
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z"/>
                  </svg>
                  <span>{t('Rename')}</span>
                </button>
                <button
                  type="button"
                  class="pill-action-btn revoke-btn"
                  disabled={isRevoked}
                  title={t('Revoke device')}
                  on:click={() => confirm(t('Revoke device'), () => mutate(`/api/devices/${device.id}`, 'DELETE'), device.name)}
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <circle cx="12" cy="12" r="10"/>
                    <line x1="15" y1="9" x2="9" y2="15"/>
                    <line x1="9" y1="9" x2="15" y2="15"/>
                  </svg>
                  <span>{t('Revoke device')}</span>
                </button>
              </div>
            </div>

            <!-- Interactive Permissions Grid -->
            <div class="permissions-section">
              <div class="permissions-header">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                </svg>
                <span>{t('ACCESS PERMISSIONS')}</span>
              </div>
              <div class="permissions-grid" class:disabled-grid={isRevoked}>
                {#each permissionScopes as perm}
                  {@const granted = hasScope(device, perm.key)}
                  <button
                    type="button"
                    class="perm-tile"
                    class:is-active={granted}
                    disabled={isRevoked}
                    role="switch"
                    aria-checked={granted}
                    aria-label={`${t(perm.label)}: ${t(perm.desc)}`}
                    on:click={() => handleToggleScope(device, perm.key)}
                  >
                    <div class="perm-icon-box" class:is-inactive={!granted} class:accent-blue={granted && perm.accent === 'blue'} class:accent-teal={granted && perm.accent === 'teal'} class:accent-purple={granted && perm.accent === 'purple'} class:accent-orange={granted && perm.accent === 'orange'} class:accent-indigo={granted && perm.accent === 'indigo'} aria-hidden="true">
                      {#if perm.key === 'files'}
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                        </svg>
                      {:else if perm.key === 'transfer'}
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <polyline points="17 1 21 5 17 9"/>
                          <path d="M3 11V9a4 4 0 0 1 4-4h14"/>
                          <polyline points="7 23 3 19 7 15"/>
                          <path d="M21 13v2a4 4 0 0 1-4 4H3"/>
                        </svg>
                      {:else if perm.key === 'photos'}
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
                          <circle cx="8.5" cy="8.5" r="1.5"/>
                          <polyline points="21 15 16 10 5 21"/>
                        </svg>
                      {:else if perm.key === 'remote'}
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <rect x="2" y="3" width="20" height="14" rx="2"/>
                          <path d="M8 21h8M12 17v4"/>
                          <path d="M10 9l2-2 2 2M12 7v6"/>
                        </svg>
                      {:else if perm.key === 'admin'}
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                          <circle cx="12" cy="11" r="3"/>
                        </svg>
                      {/if}
                    </div>
                    <div class="perm-info">
                      <div class="perm-title">{t(perm.label)}</div>
                      <div class="perm-desc">{t(perm.desc)}</div>
                    </div>
                    <div class="apple-switch" class:is-on={granted} aria-hidden="true">
                      <div class="apple-switch-thumb"></div>
                    </div>
                  </button>
                {/each}
              </div>
            </div>
          </article>
        {/each}
      </div>
    {/if}

    <!-- Security Activity Timeline -->
    <section class="activity-section">
      <div class="activity-header">
        <div class="activity-title-wrap">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
          <h2 class="activity-title">{t('Security & Audit Activity')}</h2>
        </div>
      </div>
      {#if activity.length === 0}
        <div class="activity-empty-card">
          <p>{t('No security activity recorded yet.')}</p>
        </div>
      {:else}
        <div class="activity-card-container">
          {#each activity as event}
            {@const iconType = getActivityIconType(event.action)}
            <div class="timeline-row">
              <div
                class="timeline-badge"
                class:badge-pair={iconType === 'pair'}
                class:badge-perm={iconType === 'permission'}
                class:badge-revoke={iconType === 'revoke'}
                class:badge-settings={iconType === 'settings'}
                aria-hidden="true"
              >
                {#if iconType === 'pair'}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 2l-2 2m-1.5 1.5L14 9l-1.5-1.5-2 2 1.5 1.5L10 13l-1.5-1.5-2 2 1.5 1.5L6.5 16.5A5 5 0 1 1 2 12a5 5 0 0 1 7.07-4.54"/>
                  </svg>
                {:else if iconType === 'permission'}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                  </svg>
                {:else if iconType === 'revoke'}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"/>
                    <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
                  </svg>
                {:else if iconType === 'settings'}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="3"/>
                    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
                  </svg>
                {:else}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"/>
                    <line x1="12" y1="16" x2="12" y2="12"/>
                    <line x1="12" y1="8" x2="12.01" y2="8"/>
                  </svg>
                {/if}
              </div>
              <div class="timeline-content">
                <div class="timeline-main">
                  <span class="event-action">{getActionLabel(event.action)}</span>
                  {#if event.device}
                    <span class="event-device-badge">{event.device}</span>
                  {/if}
                </div>
                <time class="event-timestamp">{formatSmartDate(event.ts)}</time>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .devices-app {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    color: #f5f5f7;
    background: #141715;
  }

  /* Integrated Solid App Header (Prevents scroll bleed-through) */
  .devices-header {
    flex-shrink: 0;
    padding: 22px 28px 18px;
    background: #161917;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    z-index: 10;
  }

  .header-top-nav {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
    flex-wrap: wrap;
  }

  .nav-left {
    flex: 1;
    min-width: 260px;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .hub-title {
    font-size: 1.45rem;
    font-weight: 700;
    letter-spacing: -0.025em;
    margin: 0;
    color: #ffffff;
    line-height: 1.2;
  }

  .device-count-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #d1d5db;
    font-size: 0.75rem;
    font-weight: 550;
    letter-spacing: 0.01em;
  }

  .status-dot-active {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #34c759;
  }

  .hub-subtitle {
    margin: 6px 0 0;
    font-size: 0.84rem;
    color: #8e8e93;
    line-height: 1.4;
  }

  .security-notice-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-top: 10px;
    padding: 4px 11px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 0.74rem;
    color: #8e8e93;
    letter-spacing: 0.01em;
  }

  .pill-lock-icon {
    color: #98989d;
    flex-shrink: 0;
  }

  .nav-right-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 36px;
    padding: 6px 15px;
    border-radius: 10px;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.16s ease, transform 0.16s ease, opacity 0.16s ease;
  }

  .action-btn .btn-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .secondary-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
  }

  .secondary-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.14);
  }

  /* Apple System Blue Action Button - Clean, Native, NOT Neon Green */
  .primary-pair-btn {
    background: #0071e3;
    border: 1px solid rgba(255, 255, 255, 0.18);
    color: #ffffff;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.35);
  }

  .primary-pair-btn:hover:not(:disabled) {
    background: #0077ed;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 113, 227, 0.45);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: default;
    transform: none;
    box-shadow: none;
  }

  /* Scroll Area */
  .devices-scroll-area {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 24px 28px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.16) transparent;
  }

  /* Empty State */
  .empty-devices-canvas {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 64px 24px;
    margin: 12px 0 24px;
    background: rgba(255, 255, 255, 0.025);
    border: 1px dashed rgba(255, 255, 255, 0.12);
    border-radius: 20px;
  }

  .empty-graphic-wrap {
    position: relative;
    width: 96px;
    height: 96px;
    display: grid;
    place-items: center;
    margin-bottom: 20px;
  }

  .radar-circle {
    position: absolute;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.15);
  }

  .radar-outer {
    width: 96px;
    height: 96px;
    opacity: 0.25;
    animation: radar-pulse 3.5s infinite ease-out;
  }

  .radar-middle {
    width: 70px;
    height: 70px;
    opacity: 0.45;
  }

  .radar-inner {
    width: 48px;
    height: 48px;
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .empty-phone-icon {
    position: relative;
    z-index: 2;
    color: #f5f5f7;
    display: grid;
    place-items: center;
  }

  @keyframes radar-pulse {
    0% { transform: scale(0.92); opacity: 0.4; }
    50% { transform: scale(1.08); opacity: 0.1; }
    100% { transform: scale(0.92); opacity: 0.4; }
  }

  .empty-title {
    font-size: 1.25rem;
    font-weight: 650;
    margin: 0 0 8px;
    color: #ffffff;
  }

  .empty-subtitle {
    max-width: 440px;
    font-size: 0.86rem;
    color: #8e8e93;
    line-height: 1.45;
    margin: 0 0 24px;
  }

  .empty-pair-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: #0071e3;
    border: 1px solid rgba(255, 255, 255, 0.18);
    color: #ffffff;
    font-size: 0.92rem;
    font-weight: 600;
    border-radius: 11px;
    padding: 10px 22px;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(0, 113, 227, 0.35);
    transition: background 0.16s ease, transform 0.16s ease;
  }

  .empty-pair-btn:hover:not(:disabled) {
    background: #0077ed;
    transform: translateY(-1px);
  }

  /* Hardware Device Cards - Clean Neutral Elevated Surface */
  .device-cards-list {
    display: flex;
    flex-direction: column;
    gap: 22px;
  }

  .device-card {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.035);
    padding: 22px 24px;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.22);
    transition: border-color 0.18s ease;
  }

  .device-card:hover {
    border-color: rgba(255, 255, 255, 0.14);
  }

  .device-card.is-revoked {
    opacity: 0.65;
    border-color: rgba(255, 69, 58, 0.2);
  }

  .device-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    padding-bottom: 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .device-header-left {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 220px;
  }

  .device-avatar {
    position: relative;
    width: 48px;
    height: 48px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
  }

  .device-avatar.offline {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
    color: #8e8e93;
  }

  .device-beacon {
    position: absolute;
    bottom: -2px;
    right: -2px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid #161917;
  }

  .beacon-online {
    background: #34c759;
  }

  .beacon-revoked {
    background: #ff453a;
  }

  .device-identity {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .device-name-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .device-name {
    font-size: 1.25rem;
    font-weight: 650;
    color: #ffffff;
    margin: 0;
    letter-spacing: -0.015em;
  }

  .status-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 999px;
  }

  .chip-active {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
    border: 1px solid rgba(52, 199, 89, 0.25);
  }

  .chip-active .chip-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #34c759;
  }

  .chip-revoked {
    background: rgba(255, 69, 58, 0.12);
    color: #ff453a;
    border: 1px solid rgba(255, 69, 58, 0.25);
  }

  .device-meta-chips {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .meta-chip {
    font-size: 0.74rem;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.07);
    color: #8e8e93;
  }

  .meta-chip.last-seen {
    color: #8e8e93;
  }

  .device-header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pill-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 5px 13px;
    border-radius: 999px;
    font-size: 0.78rem;
    font-weight: 550;
    cursor: pointer;
    transition: all 0.16s ease;
  }

  .rename-btn {
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #f5f5f7;
  }

  .rename-btn:hover {
    background: rgba(255, 255, 255, 0.13);
  }

  .revoke-btn {
    background: rgba(255, 69, 58, 0.08);
    border: 1px solid rgba(255, 69, 58, 0.22);
    color: #ff453a;
  }

  .revoke-btn:hover:not(:disabled) {
    background: rgba(255, 69, 58, 0.16);
  }

  .revoke-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Permissions Section */
  .permissions-section {
    margin-top: 18px;
  }

  .permissions-header {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #8e8e93;
    margin-bottom: 12px;
  }

  .permissions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
  }

  .permissions-grid.disabled-grid {
    pointer-events: none;
    opacity: 0.55;
  }

  .perm-tile {
    display: flex;
    align-items: center;
    gap: 13px;
    padding: 13px 16px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    color: inherit;
    text-align: left;
    cursor: pointer;
    min-height: 64px;
    transition: background 0.16s ease, border-color 0.16s ease;
  }

  .perm-tile:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.065);
    border-color: rgba(255, 255, 255, 0.14);
  }

  .perm-tile.is-active {
    background: rgba(255, 255, 255, 0.045);
    border-color: rgba(255, 255, 255, 0.11);
  }

  .perm-tile:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Category Squircle Icon Tiles (Apple Palette) */
  .perm-icon-box {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    transition: all 0.18s ease;
  }

  .perm-icon-box.is-inactive {
    background: rgba(255, 255, 255, 0.05);
    color: #8e8e93;
  }

  .perm-icon-box.accent-blue {
    background: rgba(10, 132, 255, 0.16);
    color: #0a84ff;
  }

  .perm-icon-box.accent-teal {
    background: rgba(48, 176, 199, 0.16);
    color: #30b0c7;
  }

  .perm-icon-box.accent-purple {
    background: rgba(175, 82, 222, 0.16);
    color: #af52de;
  }

  .perm-icon-box.accent-orange {
    background: rgba(255, 149, 0, 0.16);
    color: #ff9500;
  }

  .perm-icon-box.accent-indigo {
    background: rgba(88, 86, 214, 0.16);
    color: #5856d6;
  }

  .perm-info {
    flex: 1;
    min-width: 0;
  }

  .perm-title {
    font-weight: 600;
    font-size: 0.9rem;
    color: #ffffff;
    margin-bottom: 2px;
  }

  .perm-desc {
    font-size: 0.76rem;
    color: #8e8e93;
    line-height: 1.35;
    white-space: normal;
    overflow-wrap: break-word;
  }

  /* Apple-style Native Toggle Switch */
  .apple-switch {
    position: relative;
    width: 42px;
    height: 24px;
    border-radius: 999px;
    background: #39393d;
    flex-shrink: 0;
    transition: background-color 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .apple-switch.is-on {
    background: #34c759;
  }

  .apple-switch-thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
    transition: transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .apple-switch.is-on .apple-switch-thumb {
    transform: translateX(18px);
  }

  /* Security Activity Timeline (Clean Grouped Table) */
  .activity-section {
    margin-top: 36px;
    padding-top: 24px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .activity-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .activity-title-wrap {
    display: flex;
    align-items: center;
    gap: 9px;
    color: #ffffff;
  }

  .activity-title {
    font-size: 1.15rem;
    font-weight: 650;
    margin: 0;
    letter-spacing: -0.015em;
  }

  .activity-empty-card {
    padding: 24px;
    text-align: center;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.025);
    border: 1px dashed rgba(255, 255, 255, 0.08);
    color: #8e8e93;
    font-size: 0.86rem;
  }

  .activity-card-container {
    display: flex;
    flex-direction: column;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
    overflow: hidden;
  }

  .timeline-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    transition: background 0.15s ease;
  }

  .timeline-row:last-child {
    border-bottom: none;
  }

  .timeline-row:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .timeline-badge {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.06);
    color: #8e8e93;
  }

  .timeline-badge.badge-pair {
    background: rgba(10, 132, 255, 0.14);
    color: #0a84ff;
  }

  .timeline-badge.badge-perm {
    background: rgba(175, 82, 222, 0.14);
    color: #af52de;
  }

  .timeline-badge.badge-revoke {
    background: rgba(255, 69, 58, 0.14);
    color: #ff453a;
  }

  .timeline-badge.badge-settings {
    background: rgba(255, 255, 255, 0.07);
    color: #98989d;
  }

  .timeline-content {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 1;
    min-width: 0;
    gap: 12px;
  }

  .timeline-main {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex-wrap: wrap;
  }

  .event-action {
    font-weight: 550;
    font-size: 0.88rem;
    color: #ffffff;
  }

  .event-device-badge {
    font-size: 0.74rem;
    padding: 1px 7px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
    color: #98989d;
  }

  .event-timestamp {
    font-size: 0.78rem;
    color: #8e8e93;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  /* Light Theme Adjustments */
  :global(:root[data-os-theme="light"]) .devices-app {
    background: #fbfbfd;
    color: #1d1d1f;
  }

  :global(:root[data-os-theme="light"]) .devices-header {
    background: #ffffff;
    border-bottom-color: #e5e5ea;
  }

  :global(:root[data-os-theme="light"]) .hub-title,
  :global(:root[data-os-theme="light"]) .device-name,
  :global(:root[data-os-theme="light"]) .perm-title,
  :global(:root[data-os-theme="light"]) .event-action,
  :global(:root[data-os-theme="light"]) .activity-title,
  :global(:root[data-os-theme="light"]) .empty-title {
    color: #1d1d1f;
  }

  :global(:root[data-os-theme="light"]) .device-card {
    background: #ffffff;
    border-color: #e5e5ea;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04);
  }

  :global(:root[data-os-theme="light"]) .perm-tile {
    background: #f5f5f7;
    border-color: #e5e5ea;
  }

  :global(:root[data-os-theme="light"]) .perm-tile:hover:not(:disabled) {
    background: #ebebf0;
  }

  :global(:root[data-os-theme="light"]) .apple-switch {
    background: #e5e5ea;
  }

  :global(:root[data-os-theme="light"]) .apple-switch.is-on {
    background: #34c759;
  }

  :global(:root[data-os-theme="light"]) .activity-card-container {
    background: #ffffff;
    border-color: #e5e5ea;
  }

  :global(:root[data-os-theme="light"]) .timeline-row {
    border-bottom-color: #f2f2f7;
  }

  :global(:root[data-os-theme="light"]) .empty-devices-canvas {
    background: #ffffff;
    border-color: #d1d1d6;
  }

  :global(:root[data-os-theme="light"]) .secondary-btn,
  :global(:root[data-os-theme="light"]) .rename-btn {
    background: #f5f5f7;
    border-color: #d1d1d6;
    color: #1d1d1f;
  }

  :global(:root[data-os-theme="light"]) .security-notice-pill {
    background: #f5f5f7;
    border-color: #e5e5ea;
  }

  @media (max-width: 640px) {
    .devices-header {
      padding: 16px 18px 14px;
    }

    .devices-scroll-area {
      padding: 16px 18px;
    }

    .permissions-grid {
      grid-template-columns: 1fr;
    }

    .device-card-header {
      flex-direction: column;
      align-items: flex-start;
    }

    .device-header-actions {
      width: 100%;
      justify-content: flex-end;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .radar-outer {
      animation: none !important;
    }

    .action-btn,
    .empty-pair-btn,
    .perm-tile,
    .apple-switch,
    .apple-switch-thumb {
      transition: none !important;
    }
  }
</style>
