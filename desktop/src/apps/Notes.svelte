<script>
  import { native } from '../api.js';
  export let error;
  export let busy;
  export let query;
  export let note;
  export let noteTitle;
  export let noteBody;
  export let noteTrash;
  export let saveState;
  export let t;
  export let visibleNotes;
  export let date;
  export let run;
  export let load;
  export let confirm;
  export let queueNote;
  export let flushNote;
  export let chooseNote;
  export let newNote;
  export let noteAction;
  export let noteFile;
  export let importNote;
</script>

      {#if native}<p class="muted">{t('Private to this Windows user. Autosaves offline.')}</p><div class="toolbar"><button class="primary" on:click={() => run(newNote)}>{t('New note')}</button><button on:click={() => run(importNote)}>{t('Import note')}</button><label class="check"><input type="checkbox" bind:checked={noteTrash} on:change={() => run(async () => { await flushNote(); if (saveState === "error") { noteTrash = !noteTrash; return; } note = null; await load(); })}/>{t('Trash')}</label><label>{t('Search')}<input bind:value={query}/></label></div><div class="notes-layout"><section class="card note-list">{#each visibleNotes as row}<button class:active={note?.id === row.id} on:click={() => run(() => chooseNote(row))}><strong>{row.title || t('Untitled note')}</strong><small>{date(row.updated_at)}</small><p>{row.body.slice(0,90)}</p></button>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section><section class="card editor">{#if note}<label>{t('Note title')}<input bind:value={noteTitle} maxlength="120" on:input={queueNote} readonly={noteTrash || busy}/></label><label class="note-body">{t('Note text')}<textarea bind:value={noteBody} on:input={queueNote} readonly={noteTrash || busy} spellcheck="true"></textarea></label><div class="toolbar"><span role="status">{saveState === 'saving' ? t('Saving…') : saveState === 'saved' ? t('Saved') : error}</span>{#if saveState === 'error'}<button on:click={flushNote}>{t('Retry')}</button>{/if}<button on:click={() => run(() => noteFile('md'))}>{t('Export Markdown')}</button><button on:click={() => run(() => noteFile('txt'))}>{t('Export text')}</button><button on:click={() => confirm(t(noteTrash ? 'Restore' : 'Trash'), () => noteAction(noteTrash),noteTitle)}>{t(noteTrash ? 'Restore' : 'Trash')}</button></div>{:else}<p>{t('New note')}</p>{/if}</section></div>{:else}<section class="card">{t('Private apps are available in the installed Windows app.')}</section>{/if}
