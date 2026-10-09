# Phase 0 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [x] Trace and preserve the running dashboard binary, configuration, database, source snapshot and user data.
- [x] Reconcile the existing GitHub history, create the dedicated implementation branch, and exclude local secrets/build products.
- [x] Record Windows scope, phase order, known defects, implementation TODOs and the acceptance matrix.
- [~] Repair baseline syntax, dependencies, configuration loading and API contract mismatches.
  - [x] Config compatibility slice: load persisted partial feature flags without enabling remote or screen access. Verified with 12 `hh-core` tests and `cargo check -p hh-core` on macOS.
    - Done test `partial_feature_flags_keep_safe_defaults`: a saved object containing only `remote` loads; omitted `photos` and `wol` retain defaults, while `screen` remains off.
    - Done test `omitted_feature_flags_keep_defaults`: older config without `features` loads the normal defaults.
- [ ] Reproduce the final Windows build from this branch and confirm its relationship to the running baseline on the user's laptop.
