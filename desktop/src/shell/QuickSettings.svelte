<script>
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  export let t, connected, overview, free, used, volume, dark, contrast, busy, pair, go, close, preference, changeVolume;
  let panel;
  onMount(() => {
    const previous = document.activeElement;
    panel.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  });
  function keys(event) {
    if (event.key === 'Escape') { event.preventDefault(); close(); }
    if (event.key !== 'Tab') return;
    const nodes = [...panel.querySelectorAll('button:not(:disabled),input:not(:disabled)')];
    const first = nodes[0], last = nodes.at(-1);
    if (event.shiftKey && (document.activeElement === first || document.activeElement === panel)) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
</script>
<button class="os-panel-backdrop" tabindex="-1" aria-label={t('Close')} on:click={close}></button>
<div id="quick-settings" class="os-quick" role="dialog" aria-modal="true" aria-labelledby="quick-title" tabindex="-1" bind:this={panel} on:keydown={keys}>
  <div class="os-panel-heading"><div><p class="eyebrow">HOME HUB</p><h2 id="quick-title">{t('Control Center')}</h2></div><button on:click={close} aria-label={t('Close')}><Icon name="Close" size={18}/></button></div>
  <div class="os-quick-grid">
    <button class="os-quick-tile" on:click={() => { close(); go('Hardware'); }}><Icon name="Network"/><strong>{t('Network')}</strong><small>{t(connected ? 'Connected locally' : 'Service unavailable')}</small></button>
    <button class="os-quick-tile" on:click={() => { close(); go('Hardware'); }}><Icon name="Devices"/><strong>{t('Hotspot')}</strong><small>{t('Manage connection')}</small></button>
  </div>
  <label class="os-slider"><span><Icon name="Volume" size={18}/>{t('App volume')}<output>{volume}%</output></span><input type="range" min="0" max="100" bind:value={volume} on:input={event => changeVolume(Number(event.currentTarget.value))}/></label>
  <label class="os-slider"><span><Icon name="Sun" size={18}/>{t('Brightness')}<small>{t('Not available')}</small></span><input type="range" min="0" max="100" value="100" disabled aria-describedby="display-help"/></label>
  <p id="display-help" class="muted os-help">{t('Display controls are managed by your system.')}</p>
  <div class="os-quick-storage"><div><Icon name="Storage" size={18}/><strong>{free} {t('Free')}</strong><button on:click={() => { close(); go('Storage'); }}>{t('Storage')}</button></div><progress max="100" value={used} aria-label={t('Storage usage')}></progress></div>
  <button class="os-pair" disabled={busy || !connected} on:click={() => { close(); pair(); }}><Icon name="Devices"/><span>{t('Pair a device')}<small>{overview.devices ?? '—'} · {t('Connected devices')}</small></span><span aria-hidden="true">＋</span></button>
  <div class="os-quick-grid os-toggles"><label class="check"><input type="checkbox" bind:checked={dark}/>{t('Dark mode')}</label><label class="check"><input type="checkbox" bind:checked={contrast} on:change={event => { contrast = event.currentTarget.checked; preference(); }}/>{t('High contrast')}</label></div>
</div>
