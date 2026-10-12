<script>
  import { native, command, json } from '../api.js';
  const route = 'Browser';
  export let address;
  export let tabs;
  export let previousTabs;
  export let bookmarks;
  export let downloads;
  export let t;
  export let run;
  export let load;
  export let confirm;
  export let website;
  export let restoreTabs;
  export let browserAction;
  export let bookmark;
  export let permissions;
</script>

      <p>{t('Internet required. Website windows cannot access Home Hub or private apps.')}</p>{#if route === 'Browser'}<p>{t('Searches open DuckDuckGo in a separate website window.')}</p>{/if}{#if route === 'YouTube'}<section class="card"><h2>YouTube</h2><p>{t('YouTube opens in a separate private website session. If playback or sign-in is unsupported by WebView2, use your installed browser.')}</p><div class="toolbar"><button class="primary" on:click={() => run(() => website('https://www.youtube.com', true))} disabled={!native}>{t('Open website')}</button><button on:click={() => run(() => command('open_external',{url:'https://www.youtube.com'}))} disabled={!native}>{t('Open in default browser')}</button></div></section>{/if}
      {#if native}<form class="toolbar address" on:submit|preventDefault={() => run(() => website())}><label>{t('Address or search')}<input type="text" bind:value={address} placeholder="https://example.com or family photos" required/></label><button class="primary">{t('Open website')}</button><button type="button" on:click={() => run(() => command('open_external',{url:address}))}>{t('Open in default browser')}</button></form><section class="card"><h2>{t('Tabs')}</h2>{#if previousTabs.length && !tabs.length}<button class="primary" on:click={() => run(restoreTabs)}>{t('Restore previous tabs')}</button>{/if}{#each tabs as tab}<article class="browser-tab"><strong>{tab.title}</strong><small>{tab.url}</small><div class="toolbar">{#each [['back','Back'],['forward','Next'],['reload','Reload'],['focus','Focus'],['navigate','Open website'],['close','Close']] as action}<button on:click={() => run(() => browserAction(tab,action[0]))}>{t(action[1])}</button>{/each}{#if tab.isolated}<button on:click={() => run(() => browserAction(tab,'fullscreen'))}>{t('Fullscreen')}</button>{/if}<button on:click={() => run(() => bookmark(tab))}>{t('Bookmark')}</button><button on:click={() => permissions(tab)}>{t('Website permissions')}</button></div></article>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section><section class="card"><h2>{t('Bookmarks')}</h2>{#each bookmarks as row}<div class="row"><button class="link" on:click={() => run(() => website(json(row.body).url))}>{row.title}</button><button on:click={() => run(async () => { await command('local_trash',{id:row.id}); await load(); })}>{t('Remove')}</button></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section><section class="card"><h2>{t('Downloads')}</h2>{#each downloads as row}<div class="row"><span>{row.title} · {t(json(row.body).status)}</span><div class="toolbar"><button disabled={json(row.body).status !== 'complete'} on:click={() => run(() => command('save_download',{id:row.id}))}>{t('Save')}</button><button disabled={json(row.body).status === 'downloading'} on:click={() => confirm(t('Remove'),async () => { await command('discard_download',{id:row.id}); await load(); },row.title)}>{t('Remove')}</button></div></div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section><button class="danger" on:click={() => confirm(t('Clear website data'), async () => { await command('browser_clear'); await load(); })}>{t('Clear website data')}</button>{:else}<section class="card">{t('Private apps are available in the installed Windows app.')}</section>{/if}
