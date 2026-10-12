<script>
  import { api, saveFile, bytes } from '../api.js';
  export let data;
  export let query;
  export let sort;
  export let category;
  export let cursor;
  export let t;
  export let run;
  export let load;
  export let more;
  export let mutate;
  export let confirm;
  export let viewFile;
  export let fileForm;
</script>

      <form class="toolbar filters" on:submit|preventDefault={() => run(load)}><label>{t('Search')}<input bind:value={query}/></label><label>{t('Category')}<select bind:value={category} on:change={() => run(load)}><option value="">{t('All')}</option>{#each [['photo','Photos'],['video','Videos'],['document','Documents'],['music','Music'],['backup','Backups'],['download','Downloads']] as cat}<option value={cat[0]}>{t(cat[1])}</option>{/each}</select></label><label>{t('Sort')}<select bind:value={sort} on:change={() => run(load)}><option value="newest">{t('Newest')}</option><option value="oldest">{t('Oldest')}</option><option value="name">{t('Name')}</option><option value="size">{t('Size')}</option></select></label><button>{t('Search')}</button></form>
      <section class="card table-wrap"><table><thead><tr><th>{t('Name')}</th><th>{t('Size')}</th><th>{t('Category')}</th><th>{t('Second copy')}</th><th><span class="sr-only">{t('Settings')}</span></th></tr></thead><tbody>{#each data.files || [] as file}<tr><td><button class="link" on:click={() => viewFile(file)}>{file.name}</button></td><td>{bytes(file.size)}</td><td>{t(({photo:'Photos',video:'Videos',document:'Documents',music:'Music',download:'Downloads',backup:'Backups'})[file.category] || file.category)}</td><td>{t(file.second_copy_status === 'verified' ? 'Verified' : file.second_copy_status === 'disconnected' ? 'Drive disconnected' : 'Needs copy')}</td><td><div class="row-actions"><button on:click={() => run(() => saveFile(file))}>{t('Download')}</button><button on:click={() => fileForm(file,'Rename')}>{t('Rename')}</button><button on:click={() => fileForm(file,'Copy')}>{t('Copy')}</button><button on:click={() => fileForm(file,'Move')}>{t('Move')}</button><button on:click={() => confirm(t('Trash'), () => mutate(`/api/files/${file.id}`,'DELETE'),file.name)}>{t('Trash')}</button></div></td></tr>{:else}<tr><td colspan="5">{t('Nothing here yet.')}</td></tr>{/each}</tbody></table></section>
      {#if cursor}<button on:click={() => run(more)}>{t('Load more')}</button>{/if}
