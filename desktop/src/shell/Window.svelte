<script>
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { fit } from './windows.js';
  export let win, t, active, desktop, order, focus, minimize, close, busy;
  let dragging = null;
  function constrain() { win.rect = fit(win.rect, window.innerWidth, window.innerHeight); }
  function maximize() { win.maximized = !win.maximized; }
  function start(event, resize = false) {
    if (event.button !== 0 || !active || win.maximized) return;
    event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId);
    dragging = { x: event.clientX, y: event.clientY, rect: { ...win.rect }, resize };
  }
  function move(event) {
    if (!dragging) return;
    const dx = event.clientX - dragging.x, dy = event.clientY - dragging.y;
    win.rect = fit(dragging.resize ? { ...dragging.rect, width: Math.max(win.app === 'Calculator' ? 320 : 480, dragging.rect.width + dx), height: Math.max(360, dragging.rect.height + dy) } : { ...dragging.rect, x: dragging.rect.x + dx, y: dragging.rect.y + dy }, window.innerWidth, window.innerHeight);
  }
  function keyboard(event, resize = false) {
    if (!event.key.startsWith('Arrow') || win.maximized) return;
    event.preventDefault();
    const step = event.shiftKey ? 40 : 10;
    const dx = event.key === 'ArrowRight' ? step : event.key === 'ArrowLeft' ? -step : 0;
    const dy = event.key === 'ArrowDown' ? step : event.key === 'ArrowUp' ? -step : 0;
    win.rect = fit(resize ? { ...win.rect, width: Math.max(win.app === 'Calculator' ? 320 : 480, win.rect.width + dx), height: Math.max(360, win.rect.height + dy) } : { ...win.rect, x: win.rect.x + dx, y: win.rect.y + dy }, window.innerWidth, window.innerHeight);
  }
  onMount(() => { constrain(); window.addEventListener('resize', constrain); return () => window.removeEventListener('resize', constrain); });
</script>
<section class="os-window" class:focused={active} class:maximized={win.maximized} class:compact={win.app === 'Calculator'} hidden={win.minimized || desktop} style:left={`${win.rect.x}px`} style:top={`${win.rect.y}px`} style:width={`${win.rect.width}px`} style:height={`${win.rect.height}px`} style:z-index={order + 1} aria-label={t(win.app)}>
  <div class="os-titlebar">
    <button class="os-title" aria-label={`${t('Move window')}: ${t(win.app)}`} title={t('Move with arrow keys; Shift for larger steps.')} on:click={() => !active && focus()} on:dblclick={maximize} on:pointerdown={event => start(event)} on:pointermove={move} on:pointerup={() => dragging = null} on:pointercancel={() => dragging = null} on:keydown={event => keyboard(event)}><Icon name={win.app} size={16}/><span>{t(win.app)}</span></button>
    <div class="os-window-controls">
      <button class="minimize" on:click={minimize} disabled={busy} aria-label={`${t('Minimize')} ${t(win.app)}`} title={t('Minimize')}><Icon name="Minimize" size={12}/></button>
      <button class="maximize" on:click={maximize} aria-label={`${t(win.maximized ? 'Restore window' : 'Maximize')} ${t(win.app)}`} title={t(win.maximized ? 'Restore window' : 'Maximize')}><Icon name={win.maximized ? 'Restore' : 'Maximize'} size={12}/></button>
      <button class="close" on:click={close} disabled={busy} aria-label={`${t('Close')} ${t(win.app)}`} title={t('Close')}><Icon name="Close" size={12}/></button>
    </div>
  </div>
  <div class="os-window-body" inert={!active}><slot/></div>
  {#if !active}<button class="os-focus-window" on:click={focus} aria-label={`${t('Focus window')}: ${t(win.app)}`}></button>{/if}
  {#if !win.maximized && active}<button class="os-resize" aria-label={`${t('Resize window')}: ${t(win.app)}`} on:pointerdown={event => start(event, true)} on:pointermove={move} on:pointerup={() => dragging = null} on:pointercancel={() => dragging = null} on:keydown={event => keyboard(event, true)}><Icon name="Resize" size={14}/></button>{/if}
</section>
