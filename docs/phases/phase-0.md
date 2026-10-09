# Phase 0 task inventory

Source: `docs/WINDOWS_PRODUCT_TODO.md` detailed remaining gates. `[~]` is partial source, not acceptance.

- [x] Trace and preserve the running dashboard binary, configuration, database, source snapshot and user data.
- [x] Reconcile the existing GitHub history, create the dedicated implementation branch, and exclude local secrets/build products.
- [x] Record Windows scope, phase order, known defects, implementation TODOs and the acceptance matrix.
- [~] Repair baseline syntax, dependencies, configuration loading and API contract mismatches.
  - [x] Config compatibility slice: load persisted partial feature flags without enabling remote or screen access. Verified with 12 `hh-core` tests and `cargo check -p hh-core` on macOS.
    - Done test `partial_feature_flags_keep_safe_defaults`: a saved object containing only `remote` loads; omitted `photos` and `wol` retain defaults, while `screen` remains off.
    - Done test `omitted_feature_flags_keep_defaults`: older config without `features` loads the normal defaults.
  - [x] Custom data-directory defaults slice: missing persisted fields use the service-supplied defaults. Verified with 14 `hh-core` tests on macOS.
    - Done test `load_or_create_honors_supplied_defaults`: an older named config inherits the selected data, library and log paths.
    - Done test `invalid_config_does_not_overwrite`: malformed config returns an error and retains the original bytes.
  - [x] API error contract slice: align storage-full documentation and rate-limit response headers with the existing wire behavior. Focused `hh-net` tests and check pass on macOS.
    - Done test `rate_limited_response_includes_retry_after`: HTTP 429 carries `Retry-After: 1`.
    - Done test `storage_full_error_is_507`: status/code stay `507 STORAGE_FULL`, matching Android handling and transfer preflight.
    - Done check: API specification describes `507 STORAGE_FULL` and `503 STORAGE_UNAVAILABLE`.
- [ ] Reproduce the final Windows build from this branch and confirm its relationship to the running baseline on the user's laptop.
