# CLAUDE.md

Private, local-first meeting recorder. Tauri 2 desktop app: Rust core in `frontend/src-tauri`, Next.js 14 static-export UI in `frontend/src`. Working name Quire, built on the MIT-licensed Meetily. Personal use, unclassified information only.

Read `intent/private-meeting-recorder/` before changing behaviour: `intent.md` (why), `spec.md` (what, with acceptance criteria), `sync-format.md` (shared vault), `plan.md` (how, per phase). No product code without an accepted plan. Long upstream notes are in `docs/UPSTREAM_NOTES.md` (partly obsolete).

## Commands

Frontend, from `frontend/`:
- `pnpm install`, `pnpm dev` (port 3118), `pnpm tauri:dev`, `pnpm tauri:build`
- `pnpm typecheck` (tsc), `pnpm lint` (custom repo checks), `pnpm test`, `pnpm verify` (all three)
- Windows installer: `frontend\build_installer_windows.bat`

Rust, from `frontend/src-tauri/`:
- `cargo check --lib`, `cargo test --lib`
- Linux needs `ORT_STRATEGY=system` and `ORT_LIB_LOCATION` plus `LD_LIBRARY_PATH` pointing at a directory holding an ONNX Runtime shared library (with `.so.1` and versioned symlinks for tests; an empty directory is enough for `cargo check`), and placeholder `binaries/llama-helper-<target>` and `binaries/ffmpeg-<target>` files (gitignored). Keep the cargo target dir outside the repo (`CARGO_TARGET_DIR`).

## Conventions

- Rust: `anyhow::Result`, `perf_debug!` for hot paths, log module context but never transcript text, names or keys. Register commands in `src/lib.rs`. Use Tauri path APIs, never hard-coded paths.
- Naming: "microphone" and "system" for audio sources.
- No em dashes in code, text or docs (`pnpm lint` enforces it).
- Rust tests are inline `#[cfg(test)]` modules. Frontend tests are in `frontend/tests` (bun, plus one node test file).
- Branches: `enhance/*` for features, `fix/*` for bugs. One commit per work package.

## Architecture in five lines

- `audio/pipeline.rs` splits mic and system audio into a mixed recording path and a VAD-filtered transcription path (Whisper via whisper-rs, Parakeet via ort).
- `diarization/` detects speakers after the fact from saved audio; `people/` and `folders/` hold the directory and organization; `database/` is SQLite (sqlx, `migrations/`).
- `summary/` chunks the transcript and calls a provider (BuiltInAI sidecar, Ollama, or a cloud provider with a user key).
- The UI talks to Rust only through Tauri commands and events.
- Recordings are files (audio plus `metadata.json` and `transcripts.json` per meeting folder); everything else is in SQLite.

## Things Claude gets wrong

- Never edit an applied migration in `src-tauri/migrations/` (sqlx checks checksums). Add a new one.
- Run frontend tests with `pnpm test`. Plain `bun test` leaks `mock.module` between files and shows false failures; `--isolate` fixes it.
- The UI is a static export: no server routes, pass ids as `?id=`.
- `api/recording_files.rs::delete_recording_files` removes transcripts and the folder too. Do not use it to delete only audio.
- `pkill -f <pattern>` can kill your own shell. Find the pid with `ss -ltnp` and kill that.
- The first Next.js dev run compiles cold and can time out a UI test. Rerun when warm.
- Do not use em dashes, even in comments.

## Verifying your work

Before reporting any task complete, run these and paste the output:
1. `cd frontend && pnpm verify`
2. When Rust changed: `cd frontend/src-tauri && cargo check --lib && cargo test --lib` with the environment above.
3. When the UI changed: the Playwright screenshots, reviewed.

If a test fails, fix the code, not the test. For a bug: write a failing test first, run it and see it fail for the expected reason, commit it, then fix with `LOCK_TESTS=1` set so the hook blocks edits to existing tests. A phase is not done until an independent reviewer (a subagent or fresh session that did not see the build) has reviewed the diff against `spec.md` and `plan.md`.
