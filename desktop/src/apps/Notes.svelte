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

  async function setTrash(target) {
    if (noteTrash === target) return;
    await run(async () => {
      await flushNote();
      if (saveState === 'error') return;
      noteTrash = target;
      note = null;
      await load();
    });
  }

  async function handleNewNote() {
    if (noteTrash) {
      noteTrash = false;
      await load();
    }
    await run(newNote);
  }

  function formatBadgeDate(value) {
    if (!value) return '—';
    try {
      const d = new Date(value);
      if (isNaN(d.getTime())) return date(value);
      const now = new Date();
      if (d.toDateString() === now.toDateString()) {
        return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
      }
      if (d.getFullYear() === now.getFullYear()) {
        return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
      }
      return d.toLocaleDateString([], { month: 'short', day: 'numeric', year: '2-digit' });
    } catch {
      return date(value);
    }
  }

  $: wordCount = (noteBody || '').trim() ? (noteBody || '').trim().split(/\s+/).length : 0;
  $: charCount = (noteBody || '').length;
</script>

{#if native}
  <div class="notes-app">
    <!-- Master Column: Sidebar (290px) -->
    <aside class="notes-sidebar">
      <div class="sidebar-header">
        <div class="search-bar">
          <svg class="search-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M11.742 10.344a6.5 6.5 0 1 0-1.397 1.398h-.001c.03.04.062.078.098.115l3.85 3.85a1 1 0 0 0 1.415-1.414l-3.85-3.85a1.007 1.007 0 0 0-.115-.1zM12 6.5a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0z"/>
          </svg>
          <input
            type="search"
            bind:value={query}
            placeholder={t('Search notes…')}
            aria-label={t('Search notes…')}
            class="search-input"
          />
          {#if query}
            <button class="clear-search-btn" on:click={() => query = ''} aria-label={t('Clear search')}>
              &times;
            </button>
          {/if}
        </div>

        <div class="sidebar-controls">
          <div class="view-switcher" role="tablist">
            <button
              class="pill-btn"
              class:active={!noteTrash}
              role="tab"
              aria-selected={!noteTrash}
              on:click={() => setTrash(false)}
            >
              {t('All Notes')}
            </button>
            <button
              class="pill-btn"
              class:active={noteTrash}
              role="tab"
              aria-selected={noteTrash}
              on:click={() => setTrash(true)}
            >
              <span>{t('Trash')}</span>
              <span class="trash-emoji" aria-hidden="true">🗑️</span>
            </button>
          </div>

          <button
            class="new-note-btn"
            on:click={handleNewNote}
            disabled={busy}
            title={t('New note')}
            aria-label={t('New note')}
          >
            <svg viewBox="0 0 16 16" fill="currentColor" width="13" height="13" aria-hidden="true">
              <path d="M8 2a.75.75 0 0 1 .75.75v4.5h4.5a.75.75 0 0 1 0 1.5h-4.5v4.5a.75.75 0 0 1-1.5 0v-4.5h-4.5a.75.75 0 0 1 0-1.5h4.5v-4.5A.75.75 0 0 1 8 2z"/>
            </svg>
            <span>{t('New note')}</span>
          </button>
        </div>
      </div>

      <div class="notes-list">
        {#each visibleNotes as row (row.id)}
          <button
            class="note-card"
            class:active={note?.id === row.id}
            on:click={() => run(() => chooseNote(row))}
          >
            <div class="note-card-header">
              <span class="note-card-title">{row.title || t('Untitled note')}</span>
              <span class="note-card-date">{formatBadgeDate(row.updated_at)}</span>
            </div>
            <p class="note-card-snippet">{row.body ? row.body.slice(0, 160) : t('No additional text')}</p>
          </button>
        {:else}
          <div class="empty-notes-list">
            <div class="empty-list-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.042A8.967 8.967 0 006 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 016 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 016-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0018 18a8.967 8.967 0 00-6 2.292m0-14.25v14.25"/>
              </svg>
            </div>
            <p class="empty-list-label">{query ? t('No notes found') : t('Nothing here yet.')}</p>
            {#if !query && !noteTrash}
              <button class="empty-list-create-btn" on:click={handleNewNote}>
                {t('Create a note')}
              </button>
            {/if}
          </div>
        {/each}
      </div>

      <footer class="sidebar-footer">
        <button class="import-btn" on:click={() => run(importNote)} disabled={busy} title={t('Import note')}>
          <svg viewBox="0 0 16 16" fill="currentColor" width="13" height="13" aria-hidden="true">
            <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z"/>
            <path d="M7.646 1.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1-.708.708L8.5 2.707V10.5a.5.5 0 0 1-1 0V2.707L5.354 4.854a.5.5 0 1 1-.708-.708l3-3z"/>
          </svg>
          <span>{t('Import file')}</span>
        </button>
        <span class="note-count-badge">
          {visibleNotes.length} {visibleNotes.length === 1 ? t('note') : t('notes')}
        </span>
      </footer>
    </aside>

    <!-- Detail Column: Editor Canvas (Flex 1) -->
    <main class="notes-canvas">
      {#if note}
        <header class="editor-meta-bar">
          <div class="meta-left">
            <div class="autosave-status" role="status">
              {#if saveState === 'saved'}
                <span class="status-indicator saved" aria-hidden="true"></span>
                <span class="status-text">{t('Saved')}</span>
              {:else if saveState === 'saving'}
                <span class="status-indicator saving" aria-hidden="true"></span>
                <span class="status-text">{t('Saving…')}</span>
              {:else if saveState === 'error'}
                <span class="status-indicator error" aria-hidden="true"></span>
                <span class="status-text error">{error || t('Save error')}</span>
                <button class="retry-save-btn" on:click={flushNote}>{t('Retry')}</button>
              {/if}
            </div>
            <span class="meta-divider" aria-hidden="true">•</span>
            <span class="stat-counts">
              {wordCount} {t('words')}, {charCount} {t('chars')}
            </span>
          </div>

          <div class="meta-right">
            <div class="export-pills">
              <button
                class="meta-pill"
                on:click={() => run(() => noteFile('md'))}
                title={t('Export Markdown')}
                disabled={busy}
              >
                MD
              </button>
              <button
                class="meta-pill"
                on:click={() => run(() => noteFile('txt'))}
                title={t('Export text')}
                disabled={busy}
              >
                TXT
              </button>
            </div>

            <button
              class="trash-action-btn"
              class:is-restore={noteTrash}
              on:click={() => confirm(t(noteTrash ? 'Restore' : 'Trash'), () => noteAction(noteTrash), noteTitle)}
              title={t(noteTrash ? 'Restore' : 'Trash')}
              aria-label={t(noteTrash ? 'Restore' : 'Trash')}
              disabled={busy}
            >
              {#if noteTrash}
                <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14" aria-hidden="true">
                  <path fill-rule="evenodd" d="M8 3a5 5 0 1 1-4.546 2.914.5.5 0 0 0-.908-.417A6 6 0 1 0 8 2v1z"/>
                  <path d="M8 4.466V.534a.25.25 0 0 0-.41-.192L5.23 2.308a.25.25 0 0 0 0 .384l2.36 1.966A.25.25 0 0 0 8 4.466z"/>
                </svg>
              {:else}
                <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14" aria-hidden="true">
                  <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z"/>
                  <path fill-rule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"/>
                </svg>
              {/if}
            </button>
          </div>
        </header>

        <div class="editor-scroll-area">
          <div class="editor-document">
            <input
              type="text"
              class="seamless-title-input"
              bind:value={noteTitle}
              maxlength="120"
              placeholder={t('Untitled note')}
              on:input={queueNote}
              readonly={noteTrash || busy}
              aria-label={t('Note title')}
            />
            <textarea
              class="seamless-body-textarea"
              bind:value={noteBody}
              placeholder={t('Start writing your note here…')}
              on:input={queueNote}
              readonly={noteTrash || busy}
              spellcheck="true"
              aria-label={t('Note text')}
            ></textarea>
          </div>
        </div>
      {:else}
        <div class="empty-canvas-container">
          <div class="empty-canvas-content">
            <div class="empty-notebook-art" aria-hidden="true">
              <svg viewBox="0 0 80 80" fill="none" class="notebook-art-svg">
                <rect x="18" y="10" width="44" height="60" rx="8" fill="#1C201D" stroke="rgba(229,176,92,0.4)" stroke-width="2"/>
                <rect x="12" y="18" width="6" height="44" rx="3" fill="#E5B05C" opacity="0.8"/>
                <line x1="28" y1="26" x2="52" y2="26" stroke="#E5B05C" stroke-width="2.5" stroke-linecap="round"/>
                <line x1="28" y1="36" x2="48" y2="36" stroke="rgba(236,239,234,0.3)" stroke-width="2" stroke-linecap="round"/>
                <line x1="28" y1="44" x2="50" y2="44" stroke="rgba(236,239,234,0.3)" stroke-width="2" stroke-linecap="round"/>
                <line x1="28" y1="52" x2="40" y2="52" stroke="rgba(236,239,234,0.3)" stroke-width="2" stroke-linecap="round"/>
              </svg>
            </div>
            <h2 class="empty-canvas-title">{t('No note selected')}</h2>
            <p class="empty-canvas-sub">{t('Choose a note from the left, or create a new one.')}</p>
            <button class="empty-canvas-btn" on:click={handleNewNote} disabled={busy}>
              <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14" aria-hidden="true">
                <path d="M8 2a.75.75 0 0 1 .75.75v4.5h4.5a.75.75 0 0 1 0 1.5h-4.5v4.5a.75.75 0 0 1-1.5 0v-4.5h-4.5a.75.75 0 0 1 0-1.5h4.5v-4.5A.75.75 0 0 1 8 2z"/>
              </svg>
              <span>{t('New note')}</span>
            </button>
          </div>
        </div>
      {/if}
    </main>
  </div>
{:else}
  <section class="notes-non-native">
    <div class="card">
      <p>{t('Private apps are available in the installed Windows app.')}</p>
    </div>
  </section>
{/if}

<style>
  .notes-app {
    display: flex;
    width: 100%;
    height: 100%;
    min-height: 0;
    flex: 1;
    background: var(--os-bg, #121413);
    color: var(--os-ink, #ECEFEA);
    font-family: inherit;
    overflow: hidden;
  }

  /* --- LEFT COLUMN: SIDEBAR (290px) --- */
  .notes-sidebar {
    width: 290px;
    min-width: 290px;
    max-width: 290px;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: rgba(22, 26, 24, 0.75);
    border-right: 1px solid var(--os-divider, #2A2E2B);
    backdrop-filter: blur(20px);
    overflow: hidden;
  }

  .sidebar-header {
    padding: 14px 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-bottom: 1px solid var(--os-divider, #2A2E2B);
  }

  .search-bar {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    width: 14px;
    height: 14px;
    color: var(--os-ink-secondary, #A3ABA5);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    height: 32px;
    padding: 0 28px 0 32px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    color: var(--os-ink, #ECEFEA);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: all 140ms ease;
  }

  .search-input:focus {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(229, 176, 92, 0.5);
    box-shadow: 0 0 0 2px rgba(229, 176, 92, 0.15);
  }

  .search-input::placeholder {
    color: var(--os-ink-secondary, #A3ABA5);
    opacity: 0.8;
  }

  .clear-search-btn {
    position: absolute;
    right: 8px;
    background: transparent;
    border: none;
    color: var(--os-ink-secondary, #A3ABA5);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
  }

  .clear-search-btn:hover {
    color: var(--os-ink, #ECEFEA);
  }

  .sidebar-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .view-switcher {
    display: inline-flex;
    background: rgba(0, 0, 0, 0.25);
    padding: 2px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .pill-btn {
    border: none;
    background: transparent;
    color: var(--os-ink-secondary, #A3ABA5);
    font-size: 12px;
    font-weight: 500;
    padding: 4px 10px;
    border-radius: 6px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transition: all 120ms ease;
  }

  .pill-btn:hover {
    color: var(--os-ink, #ECEFEA);
  }

  .pill-btn.active {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  .trash-emoji {
    font-size: 11px;
  }

  .new-note-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: #E5B05C;
    color: #121413;
    font-size: 12px;
    font-weight: 650;
    padding: 5px 11px;
    border-radius: 7px;
    border: none;
    cursor: pointer;
    transition: all 140ms ease;
    box-shadow: 0 2px 6px rgba(229, 176, 92, 0.25);
    white-space: nowrap;
  }

  .new-note-btn:hover:not(:disabled) {
    background: #F0C070;
    transform: translateY(-1px);
    box-shadow: 0 3px 8px rgba(229, 176, 92, 0.35);
  }

  .new-note-btn:active:not(:disabled) {
    transform: translateY(0);
  }

  .new-note-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* --- NOTES LIST --- */
  .notes-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }

  .note-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
    width: 100%;
    padding: 10px 12px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 10px;
    cursor: pointer;
    transition: all 140ms ease;
  }

  .note-card:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .note-card.active {
    background: rgba(229, 176, 92, 0.14);
    border: 1px solid rgba(229, 176, 92, 0.45);
    box-shadow: 0 2px 8px rgba(229, 176, 92, 0.08);
  }

  .note-card-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    width: 100%;
    gap: 8px;
    margin-bottom: 4px;
  }

  .note-card-title {
    font-size: 14.5px;
    font-weight: 650;
    color: var(--os-ink, #ECEFEA);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .note-card.active .note-card-title {
    color: #FFFFFF;
  }

  .note-card-date {
    font-size: 11.5px;
    color: var(--os-ink-secondary, #A3ABA5);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .note-card.active .note-card-date {
    color: #E5B05C;
  }

  .note-card-snippet {
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--os-ink-secondary, #A3ABA5);
    margin: 0;
    width: 100%;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }

  .empty-notes-list {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    min-height: 180px;
    padding: 24px 16px;
    text-align: center;
    gap: 8px;
  }

  .empty-list-icon {
    width: 32px;
    height: 32px;
    color: var(--os-ink-secondary, #A3ABA5);
    opacity: 0.5;
  }

  .empty-list-label {
    font-size: 13px;
    color: var(--os-ink-secondary, #A3ABA5);
    margin: 0;
  }

  .empty-list-create-btn {
    margin-top: 6px;
    background: transparent;
    border: 1px solid rgba(229, 176, 92, 0.4);
    color: #E5B05C;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .empty-list-create-btn:hover {
    background: rgba(229, 176, 92, 0.12);
  }

  /* --- SIDEBAR FOOTER --- */
  .sidebar-footer {
    height: 38px;
    min-height: 38px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px;
    border-top: 1px solid var(--os-divider, #2A2E2B);
    background: rgba(0, 0, 0, 0.15);
  }

  .import-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: none;
    color: var(--os-ink-secondary, #A3ABA5);
    font-size: 11.5px;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 5px;
    transition: color 120ms ease;
  }

  .import-btn:hover:not(:disabled) {
    color: var(--os-ink, #ECEFEA);
    background: rgba(255, 255, 255, 0.05);
  }

  .note-count-badge {
    font-size: 11px;
    color: var(--os-ink-secondary, #A3ABA5);
    font-variant-numeric: tabular-nums;
  }

  /* --- RIGHT COLUMN: EDITOR CANVAS --- */
  .notes-canvas {
    flex: 1;
    height: 100%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--os-bg, #121413);
    position: relative;
  }

  .editor-meta-bar {
    height: 44px;
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 24px;
    border-bottom: 1px solid var(--os-divider, #2A2E2B);
    background: rgba(255, 255, 255, 0.015);
  }

  .meta-left {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--os-ink-secondary, #A3ABA5);
  }

  .autosave-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .status-indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .status-indicator.saved {
    background: #6CC070;
    box-shadow: 0 0 6px rgba(108, 192, 112, 0.4);
    animation: pulse-saved 2s infinite ease-in-out;
  }

  @keyframes pulse-saved {
    0%, 100% { opacity: 0.8; transform: scale(1); }
    50% { opacity: 1; transform: scale(1.15); }
  }

  .status-indicator.saving {
    background: #E5B05C;
    box-shadow: 0 0 6px rgba(229, 176, 92, 0.4);
    animation: pulse-saving 1s infinite ease-in-out;
  }

  @keyframes pulse-saving {
    0%, 100% { opacity: 0.5; }
    50% { opacity: 1; }
  }

  .status-indicator.error {
    background: #F2877F;
    box-shadow: 0 0 6px rgba(242, 135, 127, 0.5);
  }

  .status-text.error {
    color: #F2877F;
  }

  .retry-save-btn {
    background: rgba(242, 135, 127, 0.15);
    border: 1px solid rgba(242, 135, 127, 0.3);
    color: #F2877F;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
    cursor: pointer;
  }

  .meta-divider {
    opacity: 0.4;
  }

  .stat-counts {
    font-variant-numeric: tabular-nums;
  }

  .meta-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .export-pills {
    display: inline-flex;
    gap: 5px;
  }

  .meta-pill {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--os-ink-secondary, #A3ABA5);
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .meta-pill:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    color: #FFFFFF;
    border-color: rgba(255, 255, 255, 0.18);
  }

  .trash-action-btn {
    background: transparent;
    border: none;
    color: var(--os-ink-secondary, #A3ABA5);
    padding: 6px;
    border-radius: 6px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: all 120ms ease;
  }

  .trash-action-btn:hover:not(:disabled) {
    color: #F2877F;
    background: rgba(242, 135, 127, 0.1);
  }

  .trash-action-btn.is-restore:hover:not(:disabled) {
    color: #E5B05C;
    background: rgba(229, 176, 92, 0.1);
  }

  /* --- EDITOR SCROLL AREA & CANVAS --- */
  .editor-scroll-area {
    flex: 1;
    overflow-y: auto;
    padding: 28px 36px 36px;
    display: flex;
    justify-content: center;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }

  .editor-document {
    width: 100%;
    max-width: 820px;
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }

  .seamless-title-input {
    width: 100%;
    background: transparent;
    border: none;
    outline: none;
    font-size: 1.8rem;
    font-weight: 700;
    line-height: 1.25;
    color: #FFFFFF;
    padding: 0 0 16px 0;
    margin: 0 0 12px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    letter-spacing: -0.02em;
    font-family: inherit;
    caret-color: #E5B05C;
  }

  .seamless-title-input::placeholder {
    color: rgba(255, 255, 255, 0.28);
  }

  .seamless-body-textarea {
    width: 100%;
    flex: 1;
    min-height: 380px;
    background: transparent;
    border: none;
    outline: none;
    resize: none;
    font-size: 15px;
    line-height: 1.75;
    color: var(--os-ink, #ECEFEA);
    font-family: inherit;
    padding: 4px 0 32px 0;
    caret-color: #E5B05C;
    scrollbar-width: none;
  }

  .seamless-body-textarea::placeholder {
    color: rgba(255, 255, 255, 0.22);
  }

  /* --- EMPTY CANVAS (CALM STATE) --- */
  .empty-canvas-container {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
  }

  .empty-canvas-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 360px;
  }

  .empty-notebook-art {
    margin-bottom: 20px;
    filter: drop-shadow(0 8px 24px rgba(0, 0, 0, 0.35));
  }

  .notebook-art-svg {
    width: 72px;
    height: 72px;
  }

  .empty-canvas-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--os-ink, #ECEFEA);
    margin: 0 0 8px 0;
    letter-spacing: -0.015em;
  }

  .empty-canvas-sub {
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--os-ink-secondary, #A3ABA5);
    margin: 0 0 22px 0;
  }

  .empty-canvas-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: #E5B05C;
    color: #121413;
    font-size: 13px;
    font-weight: 650;
    padding: 9px 18px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(229, 176, 92, 0.3);
    transition: all 140ms ease;
  }

  .empty-canvas-btn:hover:not(:disabled) {
    background: #F0C070;
    transform: translateY(-1px);
    box-shadow: 0 4px 14px rgba(229, 176, 92, 0.4);
  }

  .empty-canvas-btn:active:not(:disabled) {
    transform: translateY(0);
  }

  /* --- NON-NATIVE FALLBACK --- */
  .notes-non-native {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 24px;
  }

  @media (max-width: 640px) {
    .notes-app {
      flex-direction: column;
    }
    .notes-sidebar {
      width: 100%;
      min-width: 100%;
      max-width: 100%;
      height: 40%;
      border-right: none;
      border-bottom: 1px solid var(--os-divider, #2A2E2B);
    }
    .notes-canvas {
      height: 60%;
    }
  }
</style>
