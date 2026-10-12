<script>
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  export let t, language, active, connected, connectionChecked, overview, free, quickOpen, go, toggleQuick, toggleLanguage, lock;
  let now = new Date();
  $: locale = language === 'hi' ? 'hi-IN' : 'en-IN';
  onMount(() => {
    let timer;
    const tick = () => { now = new Date(); timer = setTimeout(tick, 60000 - Date.now() % 60000); };
    const visible = () => { if (!document.hidden) now = new Date(); };
    tick(); document.addEventListener('visibilitychange', visible);
    return () => { clearTimeout(timer); document.removeEventListener('visibilitychange', visible); };
  });
</script>
<header class="os-status">
  <button class="os-identity" on:click={() => go('Home')} aria-label={t('Home')}><Icon name="Home" size={19}/><strong>{active === 'Home' ? t('Home Computer') : t(active)}</strong></button>
  <time class="os-clock" datetime={now.toISOString()}><strong>{now.toLocaleTimeString(locale,{hour:'numeric',minute:'2-digit',hour12:true}).toUpperCase()}</strong><span>{now.toLocaleDateString(locale,{weekday:'short',day:'numeric',month:'short'})}</span></time>
  <div class="os-system-cluster">
    <button class="os-network" on:click={toggleQuick} title={t('Local connection')}><span class:online={connected} class="os-status-dot"></span>{t(connected ? 'Connected locally' : connectionChecked ? 'Service unavailable' : 'Private apps ready')}</button>
    <button class="os-storage" on:click={() => go('Storage')} title={t('Free space')}><Icon name="Storage" size={15}/>{free} <span>{t('Free')}</span></button>
    <button class="os-devices" on:click={() => go('Devices')} aria-label={`${t('Connected devices')}: ${overview.devices ?? '—'}`}><Icon name="Devices" size={17}/>{overview.devices ?? '—'}</button>
    <button on:click={toggleLanguage} aria-label={t('Language')}>{language === 'hi' ? 'हिन्दी' : 'EN'}</button>
    <button id="quick-toggle" class:pressed={quickOpen} on:click={toggleQuick} aria-label={t('Control Center')} aria-expanded={quickOpen} aria-controls="quick-settings"><Icon name="Settings" size={19}/></button>
    <button on:click={lock} aria-label={t('Lock')} title={t('Lock')}><Icon name="Lock" size={17}/></button>
  </div>
</header>
