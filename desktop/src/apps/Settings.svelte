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
  export let busy;
  export let settings;
  export let setup;
  export let setupStep;
  export let hubName;
  export let copyFolder;
  export let privateTrash;
  export let updateCheck;
  export let t;
  export let run;
  export let load;
  export let confirm;
  export let saveSettings;
  export let flushNote;
  export let savePreferences;
  export let diagnostics;
  export let checkUpdates;
  export let loadPrivateTrash;
  let backup = null, network = null, summaryError = '';
  onMount(() => {
    if (native) backupSummary().then(value => backup = value).catch(e => summaryError = String(e?.message || e));
    api('/api/network').then(value => network = value).catch(e => summaryError = String(e?.message || e));
  });
</script>

      <section class="card"><h2>{t('Settings')}</h2><form on:submit|preventDefault={() => run(saveSettings,t('Saved'))}><label>{t('Hub name')}<input bind:value={hubName} maxlength="120" required/></label><label>{t('Library folder')}<input value={settings.library_root || ''} readonly/></label><label>{t('Second-copy folder')}<input bind:value={copyFolder}/></label><h3>{t('Backup and second-copy schedules')}</h3><label>{t('Backup interval (minutes)')}<input type="number" min="1" max="10080" bind:value={settings.backup_interval_minutes}/></label><label>{t('Scrub interval (hours)')}<input type="number" min="1" max="720" bind:value={settings.scrub_interval_hours}/></label><label>{t('Second-copy interval (minutes)')}<input type="number" min="1" max="43200" bind:value={settings.second_copy_interval_minutes}/></label><label class="check"><input type="checkbox" bind:checked={settings.screen_enabled}/>{t('Allow screen sharing')}</label><label class="check"><input type="checkbox" bind:checked={settings.wake_enabled}/>{t('Request scheduled wake')}</label>{#if settings.wake_enabled}<label>{t('Wake time')}<input type="time" bind:value={settings.wake_time}/></label>{/if}<label class="check"><input type="checkbox" bind:checked={settings.update_enabled}/>{t('Check for signed updates')}</label>{#if settings.update_enabled}<label>{t('Update manifest URL')}<input type="url" bind:value={settings.update_manifest_url}/></label><label>{t('Update public key')}<input maxlength="64" bind:value={settings.update_pubkey_hex}/></label>{/if}<button class="primary" disabled={busy}>{t('Save')}</button></form><button on:click={() => { setup = true; setupStep = 0; }}>{t('Setup')}</button></section><section class="card"><h2>{t('Language')}</h2><label>{t('Language')}<select bind:value={language} on:change={savePreferences}><option value="en">English</option><option value="hi">हिन्दी</option></select></label><h3>{t('Notification preferences')}</h3>{#each [['large','Larger text'],['contrast','High contrast'],['reduced','Reduce motion'],['notifications','Notifications in this app']] as option}<label class="check"><input type="checkbox" checked={option[0] === 'large' ? large : option[0] === 'contrast' ? contrast : option[0] === 'reduced' ? reduced : notifications} on:change={event => { if (option[0] === 'large') large = event.currentTarget.checked; else if (option[0] === 'contrast') contrast = event.currentTarget.checked; else if (option[0] === 'reduced') reduced = event.currentTarget.checked; else notifications = event.currentTarget.checked; savePreferences(); }}/>{t(option[1])}</label>{/each}</section><section class="card"><h2>{t('Protect your memories')}</h2><p>{t('Turn on BitLocker in Windows to protect data at rest.')}</p><p>{t('A single drive is not a backup.')}</p><p>{t('Diagnostics include system status and omit file names and contents.')}</p><div class="toolbar"><button on:click={() => run(diagnostics)}>{t('Export diagnostics')}</button>{#if native}<button on:click={() => run(async () => { await flushNote(); await exportPrivate(); })}>{t('Export private apps')}</button><button on:click={() => run(async () => { await importPrivate(); await loadPrivateTrash(); })}>{t('Import private apps')}</button>{/if}</div></section>
      <section class="card"><h2>{t('Privacy')}</h2><p>{t('No telemetry is sent by default.')}</p><button disabled={busy} on:click={() => run(async () => { await api('/api/pause-sharing', 'POST', { paused: !settings.pause_sharing }); await load(); })}>{t(settings.pause_sharing ? 'Resume sharing' : 'Pause sharing')}</button></section>
      <section class="card"><h2>{t('Updates')}</h2><button disabled={busy} on:click={() => run(checkUpdates)}>{t('Check now')}</button>{#if updateCheck}<p role="status">{updateCheck.available ? `${t('Update available')}: ${updateCheck.latest_version}` : updateCheck.reason === 'updates_disabled' ? t('Turn on signed update checks and save settings first.') : updateCheck.reason === 'channel_not_configured' ? t('Set the update URL and public key, then save settings.') : updateCheck.reason ? t('Update check failed.') : t('Up to date.')}</p>{/if}</section>
    {#if native}<section class="card"><h2>{t('Private app trash')}</h2>{#each privateTrash as row}<div class="row"><span>{row.title} · {t(row.kind)}</span><div class="toolbar"><button on:click={() => run(async () => { await command('local_restore',{id:row.id}); await loadPrivateTrash(); })}>{t('Restore')}</button><button class="danger" on:click={() => confirm(t('Delete permanently'),async () => { await command('local_purge',{id:row.id}); await loadPrivateTrash(); },row.title)}>{t('Delete permanently')}</button></div></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>{/if}

<section class="card"><h2>{t('Storage and backup')}</h2>{#if summaryError}<p role="alert">{summaryError}</p>{/if}{#if backup}<Details value={backup} {t}/>{/if}</section>
<section class="card"><h2>{t('Network')}</h2>{#if network}<Details value={network} {t}/>{/if}</section>
