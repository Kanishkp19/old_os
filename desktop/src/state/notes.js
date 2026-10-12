import { command } from '../api.js';

// Serial flushes ensure a slow save cannot overwrite a newer draft.
export function createNoteEditor(get, set, reload, translate, onError) {
  let timer, queue = Promise.resolve(), revision = 0;
  function queueNote() {
    revision++;
    set({ saveState: 'saving' });
    clearTimeout(timer);
    timer = setTimeout(() => { void flushNote(); }, 650);
  }
  async function flushNote() {
    clearTimeout(timer);
    const state = get();
    if (!state.note || !['saving', 'error'].includes(state.saveState)) return queue;
    const { note, noteTitle, noteBody } = state;
    const savedRevision = revision;
    queue = queue.catch(() => {}).then(async () => {
      try {
        const row = await command('local_save', { id: note.id, kind: 'note', title: noteTitle, body: noteBody });
        const latest = get();
        set({ records: latest.records.map(item => item.id === row.id ? row : item) });
        if (latest.note?.id === note.id && revision === savedRevision) set({ note: row, saveState: 'saved' });
      } catch (error) { set({ saveState: 'error' }); onError(error); }
    });
    return queue;
  }
  async function chooseNote(row) {
    await flushNote();
    if (get().saveState === 'error') return;
    set({ note: row, noteTitle: row.title, noteBody: row.body, saveState: 'saved' });
  }
  async function newNote() {
    await flushNote();
    if (get().saveState === 'error') return;
    const row = await command('local_save', { id: null, kind: 'note', title: translate('Untitled note'), body: '' });
    set({ records: [row, ...get().records] });
    await chooseNote(row);
  }
  async function noteAction(restore) {
    await flushNote();
    if (get().saveState === 'error') return;
    await command(restore ? 'local_restore' : 'local_trash', { id: get().note.id });
    set({ note: null });
    await reload();
  }
  async function noteFile(format) {
    await flushNote();
    if (get().saveState === 'error') return;
    await command('note_export', { id: get().note.id, format });
  }
  async function noteImport() {
    const row = await command('note_import');
    if (row) { set({ noteTrash: false }); await reload(); await chooseNote(row); }
  }
  return { queueNote, flushNote, chooseNote, newNote, noteAction, noteFile, noteImport, dispose: () => clearTimeout(timer) };
}
