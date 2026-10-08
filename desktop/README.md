# Home Hub Windows launcher

The installed launcher is a Tauri 2 application using system WebView2 and Svelte. The same Vite build writes `hub/dashboard/index.html`, `app.js` and `style.css` for embedding in the Rust service. The transitional dashboard remains functional with an explicit local session exchange until that release build occurs.

Implemented source, awaiting the approved deferred validation phase:

- Home, Photos, Files, Transfers, Storage, Remote, Browser, YouTube, Music, Notes, Calculator, Devices, Hardware and Settings.
- Guided setup, English/Hindi UI, larger text, high contrast, reduced motion, keyboard navigation, modal focus trapping and in-app status/error announcements.
- Files search/sort/pagination, download, rename/copy/move, trash restore/purge; folder import with Keep/Copy/Later; integrity/second-copy jobs, cancellation and duplicate review.
- Private offline Notes with serialized autosave, trash/restore, Markdown/text file import/export. Private playlists and bookmarks persist per Windows user; private JSON exports exclude browser sessions, download paths and preferences. Import is transactional and preserves existing IDs.
- Bounded arithmetic parser with precedence, powers, percentage, parentheses and history; no code evaluation.
- Native website tabs, navigation, bookmarks, per-tab website grants, downloads saved in private storage and explicit export/discard. Internet website windows cannot invoke native commands or read Hub media. YouTube uses a separate private profile, with default-browser fallback.
- Remote panel reports actual helper capabilities and sessions; stop actions call the service. Start casting from the companion. Native `hh-session` owns the video receiver; the Svelte shell does not create another peer or inject input.

The main window alone has the custom command capability. Every command also checks its native window label and URL. The proxy reads the same-user credential natively and only sends it to a fixed loopback destination with redirects/proxies disabled. Neither webpage embeds a session key. Browser dashboard authorization uses a user-supplied key exchanged for an HttpOnly session cookie and an in-memory CSRF value. A page reload requires a fresh exchange for writes.

Closing the launcher requests a Notes flush, cancels active Hub file saves and waits for partial-file cleanup before exiting the Tauri process, including its browser windows. A failed note save or a drive that cannot finish within the bounded close wait keeps the app open and shows an error. Large Hub file saves show byte progress and an explicit cancellation control; stalled reads fail safely. The background Hub service and session helper remain owned by the service.

Limits: 12 native tabs; 64 current website downloads; 1 MB per note; 1,500 portable private records including trash; 8 MB portable JSON imports/exports. Individual Notes export is available when combined export is too large. No automatic note deletion or overwrite occurs on import. Preview responses are capped at 16 MB and open-ended video ranges at 8 MB; use Download for larger images or unsupported codecs. Permission changes explicitly reset the tab's isolated website session and reload it. Permission grants cover the website content in that tab and expire on tab close; they are not a system-wide grant. Literal private/local addresses are blocked; DNS resolution and subresource filtering are browser-runtime behavior, while the Hub's host/origin/auth protections remain the security boundary.

After all implementation phases finish, run `npm install`, `npm run check`, `npm test`, `npm run build`, then desktop Rust checks/tests and the Windows packaging/UI matrix. None of those checks or builds were run during this implementation pass. WebView2 permission/cache behavior, codecs, downloader failures, shutdown process cleanup, Windows ACL enforcement, physical hardware and screen receiving still require Windows validation. Do not deploy an unvalidated build over the preserved working baseline.
