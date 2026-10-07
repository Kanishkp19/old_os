# Preserved baseline

On 2026-10-08, the running demonstrated dashboard was served by PID 28324 at `127.0.0.1:47801`. The executable was `hub/target/debug/hh-service` in this checkout, with SQLite at `/private/tmp/hh-test-mac/data/hub.db`. The executable was built before the inspected source snapshot; there was no Git history in this folder, so source-to-binary reproducibility is unproven.

A sibling snapshot directory `home-hub-baseline-20261008` preserves the source archive, executable, original APK, runtime non-secret files and an online SQLite backup. SHA-256 fingerprints are in its `source/provenance.json`. Credentials remain in the original protected runtime directory; they are excluded from Git. The running service and originals have not been stopped, replaced or migrated.

GitHub responded successfully with no refs to both HEAD and all-ref queries. A new root baseline commit therefore preserves the snapshot without replacing remote history. Further delivery uses `implementation/windows-home-hub` only. Build provenance for the new product will include exact commit, lockfiles, toolchain versions and artifact hashes after validation.
