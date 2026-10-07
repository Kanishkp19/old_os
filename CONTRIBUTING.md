# Contributing to Home Hub

Read `docs/AGENTS.md` first — its hard rules apply to humans and coding agents alike.

## Workflow
1. Pick a task from `docs/IMPLEMENTATION_PLAN.md` (reference the task ID in your PR).
2. Branch: `feat/<task-id>-short-name`.
3. Conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `perf:`, `chore:`.
4. Before opening a PR:
   ```bash
   cd hub
   cargo fmt --all && cargo clippy --workspace -- -D warnings && cargo test --workspace
   cargo audit && cargo deny check
   ```
5. Schema change? Update `docs/BACKEND_SCHEMA.md` in the same PR. API change? Update `docs/API_SPEC.md`.
6. Tick the security checklist (`docs/SECURITY.md` §10) for any network or data-path change.

## Review bar
- Definition of done: `docs/IMPLEMENTATION_PLAN.md` §12.
- Never-ask-permission list (dependencies with telemetry, protocol changes, deletion behavior): `docs/AGENTS.md` §8.
