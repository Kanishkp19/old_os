<script>
  import Icon from './Icon.svelte';
  import { apps } from './labels.js';
  export let t, windows, active, go;
  $: shelf = [...apps, ...windows.map(w => w.app).filter(app => !apps.includes(app))];
</script>
<nav class="os-dock" aria-label={t('App shelf')}>
  {#each shelf as app}
    <button class="os-dock-item" class:current={active === app} class:home={app === 'Home'} on:click={() => go(app)} aria-label={t(app)} aria-pressed={active === app}>
      <span class="os-app-icon" data-app={app}><Icon name={app} size={25}/></span>
      <span class="os-tooltip">{t(app)}</span>
      <span class="os-running" class:running={windows.some(w => w.app === app)} aria-hidden="true"></span>
    </button>
  {/each}
</nav>
