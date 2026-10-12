<script>
  import { api } from '../api.js';
  export let data;
  export let form;
  export let formValue;
  export let t;
  export let date;
  export let mutate;
  export let confirm;
  export let changeScope;
</script>

      <section class="card">{#each data.devices || [] as device}<article class="device"><div class="row"><div><h2>{device.name}</h2><small>{device.platform} · {device.status === 'revoked' ? t('revoked') : `${t('Last seen')}: ${date(device.last_seen_at)}`}</small></div><button class="danger" disabled={device.status === 'revoked'} on:click={() => confirm(t('Revoke device'),() => mutate(`/api/devices/${device.id}`,'DELETE'),device.name)}>{t('Revoke device')}</button></div><fieldset disabled={device.status === 'revoked'}><legend>{t('Device permissions')}</legend>{#each ['files','transfer','photos','remote','admin'] as scope}<label class="check"><input type="checkbox" checked={(Array.isArray(device.scopes) ? device.scopes : String(device.scopes || '').split(',')).includes(scope)} on:change={event => changeScope(device,scope,event.currentTarget.checked)}/>{t(scope === 'files' ? 'Files' : scope === 'transfer' ? 'Transfers' : scope === 'photos' ? 'Photos' : scope === 'remote' ? 'Remote' : 'Admin')}</label>{/each}</fieldset><button on:click={() => { form = { action:'device',file:device }; formValue = device.name; }}>{t('Rename')}</button></article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
      <section class="card"><h2>{t('Security activity')}</h2>{#each data.activity || [] as event}<div class="row"><span>{t(event.action)} · {event.device}</span><small>{date(event.ts)}</small></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>
