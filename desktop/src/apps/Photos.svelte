<script>
  import { api, items, bytes, native, albumsList, albumsCreate, albumsUpdate, albumsDelete, albumsItemsList, albumsItemsAdd, albumsItemsRemove, backupSummary } from '../api.js';
  import { onMount } from 'svelte';
  import Details from '../Details.svelte';
  import PhotoTile from '../PhotoTile.svelte';
  import { routes, icons } from '../i18n.js';
  export let route;
  export let busy;
  export let connected;
  export let overview;
  export let settings;
  export let data;
  export let cursor;
  export let photoMonth;
  export let t;
  export let date;
  export let run;
  export let load;
  export let more;
  export let mutate;
  export let viewFile;
  export let go;
  export let confirm;
  let albums = [], album = null, albumItems = [], albumCursor = null, albumListCursor = null;
  let albumName = '', renameValue = '', renaming = false, albumError = '', backup = null;
  const alphabet = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
  function newAlbumId() {
    let time = Date.now(), id = '';
    for (let i = 0; i < 10; i++) { id = alphabet[time % 32] + id; time = Math.floor(time / 32); }
    const random = crypto.getRandomValues(new Uint8Array(16));
    return id + Array.from(random, value => alphabet[value % 32]).join('');
  }
  async function refreshAlbums() {
    if (!native) return;
    try { const page = await albumsList(); albums = items(page); albumListCursor = page.next_cursor; albumError = ''; }
    catch (e) { albumError = String(e?.message || e); }
  }
  async function openAlbum(row) {
    album = row;
    try { const page = await albumsItemsList(row.id); albumItems = items(page); albumCursor = page.next_cursor; albumError = ''; }
    catch (e) { albumError = String(e?.message || e); }
  }
  async function albumAction(action) {
    try { await action(); await refreshAlbums(); if (album) await openAlbum(album); }
    catch (e) { albumError = String(e?.message || e); }
  }
  onMount(() => {
    if (route !== 'Photos' || !native) return;
    void refreshAlbums();
    backupSummary().then(value => backup = value).catch(e => albumError = String(e?.message || e));
  });
</script>

{#if route === 'Home'}
      <section class="welcome"><div><h2>{overview.name || 'Home Hub'}</h2><p>{t('Your private home cloud')}</p></div><button disabled={busy || !connected} on:click={() => run(async () => { await api('/api/pause-sharing', 'POST', { paused: !settings.pause_sharing }); await load(); })}>{t(settings.pause_sharing ? 'Resume sharing' : 'Pause sharing')}</button></section>
      <div class="metrics"><div class="card"><span>{t('Free space')}</span><strong>{bytes(overview.free_bytes)}</strong><small>{bytes(overview.total_bytes)}</small></div><div class="card"><span>{t('Connected devices')}</span><strong>{overview.devices ?? '—'}</strong></div><div class="card"><span>{t('Active transfers')}</span><strong>{overview.active_transfers ?? '—'}</strong></div><div class="card"><span>{t('Verification')}</span><strong>{overview.copies ?? '—'}</strong><small>{t('A single drive is not a backup.')}</small></div></div>
      <div class="launcher">{#each routes.slice(1) as item, index}<button class="card app-tile" on:click={() => go(item)}><span aria-hidden="true">{icons[index + 1]}</span><strong>{t(item)}</strong></button>{/each}</div>
      <section class="card"><h2>{t('Alerts')}</h2>{#each data.alerts || [] as alert}<div class="row"><div><strong>{alert.message}</strong><small>{date(alert.created_at)}</small></div><button on:click={() => run(() => mutate(`/api/alerts/${alert.id}/ack`))}>{t('Confirm')}</button></div>{:else}<p class="muted">{t('Nothing here yet.')}</p>{/each}</section>
{:else}
      <p class="muted">{t('Phone storage cleanup requires verified copies and confirmation on your phone.')}</p>
      <div class="toolbar months"><button class:active={!photoMonth} on:click={() => { photoMonth = ""; run(load); }}>{t("All months")}</button>{#each items(data.years) as month}<button class:active={photoMonth === `${month.year}-${month.month}`} on:click={() => { photoMonth = `${month.year}-${month.month}`; run(load); }}>{month.year} / {month.month} · {month.count}</button>{/each}</div>
      <section class="photo-grid">{#each data.photos || [] as photo}<button class="photo" on:click={() => viewFile({ ...photo, id: photo.file_id, mime: photo.mime || (photo.type_ === 'video' ? 'video/mp4' : 'image/jpeg'), name: photo.name || date(photo.taken_at) })}><PhotoTile {photo} label={date(photo.taken_at)} {t}/></button>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>{#if cursor}<button on:click={() => run(more)}>{t('Load more')}</button>{/if}{#if album}<section class="card"><h3>{t('Add to album')}: {album.name}</h3>{#each data.photos || [] as photo}<button on:click={() => albumAction(() => albumsItemsAdd(album.id, [photo.file_id]))}>{photo.name || date(photo.taken_at)}</button>{/each}</section>{/if}
  {#if native}
    <section class="card"><h2>{t('Albums')}</h2>
      {#if albumError}<p role="alert">{albumError}</p>{/if}
      <form class="toolbar" on:submit|preventDefault={() => albumAction(async () => { await albumsCreate(newAlbumId(), albumName.trim()); albumName = ''; })}>
        <label>{t('New album')}<input bind:value={albumName} maxlength="120" required/></label><button class="primary" disabled={!albumName.trim()}>{t('Create')}</button>
      </form>
      <div class="toolbar"><button class:active={!album} on:click={() => album = null}>{t('All months')}</button>{#each albums as row}<button class:active={album?.id === row.id} on:click={() => openAlbum(row)}>{row.name} · {row.item_count}</button>{/each}</div>
      {#if albumListCursor}<button on:click={async () => { try { const page = await albumsList(albumListCursor); albums = [...albums, ...items(page)]; albumListCursor = page.next_cursor; } catch (e) { albumError = String(e?.message || e); } }}>{t('Load more')}</button>{/if}
      {#if album}
        <div class="toolbar"><button on:click={() => { renameValue = album.name; renaming = true; }}>{t('Rename')}</button><button class="danger" on:click={() => confirm(t('Delete album? Photos stay in your library.'), async () => { await albumsDelete(album.id); album = null; albumItems = []; await refreshAlbums(); }, album.name)}>{t('Delete')}</button></div>
        {#if renaming}<form class="toolbar" on:submit|preventDefault={() => albumAction(async () => { album = await albumsUpdate(album.id, album.name, renameValue.trim()); renaming = false; })}><label>{t('Name')}<input bind:value={renameValue} maxlength="120" required/></label><button disabled={!renameValue.trim()}>{t('Save')}</button><button type="button" on:click={() => renaming = false}>{t('Cancel')}</button></form>{/if}
        {#each albumItems as file}<div class="row"><span>{file.name}</span><button on:click={() => albumAction(() => albumsItemsRemove(album.id, [file.id || file.file_id]))}>{t('Remove')}</button></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}
        {#if albumCursor}<button on:click={async () => { try { const page = await albumsItemsList(album.id, albumCursor); albumItems = [...albumItems, ...items(page)]; albumCursor = page.next_cursor; } catch (e) { albumError = String(e?.message || e); } }}>{t('Load more')}</button>{/if}
      {/if}
    </section>
    {#if backup}<section class="card"><h2>{t('Backup summary')}</h2><Details value={backup} {t}/></section>{/if}
  {/if}
{/if}
