# Phase 8 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [~] Browser address/search, tabs, navigation, bookmarks, session restore, downloads, permission prompts and clearing. Code complete, untested in BUILD; denied website and download requests now prompt the owner to grant per-tab permissions and retry, while native history/session/download controls retain existing behavior.
- [~] YouTube isolated window, playback/fullscreen/sign-in where supported and installed-browser fallback. Code complete, untested in BUILD; YouTube route opens its isolated WebView2 profile, gives a fullscreen control, and explains the default-browser fallback when WebView2 cannot play or sign in.
- [~] Music library playback, queue, seek, volume, repeat/shuffle and private playlists. Code complete, untested in BUILD; private playlist order is honored, an explicit up-next queue is editable, native media controls supply seek/volume, and playback failures stop with an error.
- [~] Notes offline autosave, search, rename, trash/restore and Markdown/text import/export. Code complete, untested in BUILD; debounced saves serialize writes, failed saves keep the editor and roll back Trash selection, while native import/export uses bounded UTF-8 and durable local writes.
- [~] Calculator arithmetic, percentages, parentheses, keyboard, history and safe expression parsing. Code complete, untested in BUILD; bounded recursive parser handles malformed input and zero division without eval, while calculations persist a capped history with a clear control.
- [~] Integrate all five apps into the launcher and common window behavior. Code complete, untested in BUILD; all five routes share the shell, route titles track the active language, skip navigation reaches content, and dashboard views explain native-only private features.
