# Windows Home Hub implementation state — 2026-10-08

The approved objective is the Windows Home Hub release in `WINDOWS_PRODUCT_TODO.md`, followed by its full acceptance matrix, physical-device validation, and household pilot. M6/OS work remains outside scope. The original demonstrated service and its data have not been replaced. A separate baseline snapshot is retained outside Git.

The branch contains ongoing changes to the Rust service and helper, Android companion, Apple companion projects, Tauri/Svelte desktop app, shared dashboard, and Inno installer. This is an implementation checkpoint, not a release. Local checks passed for the Rust workspace and doc tests on macOS, the Tauri native crate on macOS, desktop frontend build/check and calculator tests, and Android debug unit tests. See `WINDOWS_VALIDATION.md` for exact results and limits.

The independent diff review found two upgrade defects. The Inno `AppId` is restored to the previous value, and rollback now retains the attempted upgrade database before restoring old binaries and database together. These installer scripts still require execution on Windows.

Required next work: finish the remaining `[~]` items in `WINDOWS_PRODUCT_TODO.md`, review all new contracts against API/security docs, run Windows and full Xcode builds, complete the comprehensive transfer/backup/storage/security/UI test matrix, fix defects and rerun regressions, produce signed release artifacts when credentials are available, validate on the Windows laptop and Android phone, then conduct the documented four-week 20-household pilot. Do not mark those external checks passed from macOS source checks.
