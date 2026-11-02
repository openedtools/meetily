# Plan: Quire desktop, Phase A (from spec.md, accepted 2026-11-01)
Status: approved by Ryan on 2026-11-01 at the plan gate. P1 (identifier `com.openedtools.notes`), P2 (branch `enhance/quire-phase-a`) and P3 (classify the failing tests, fix causes, delete only tests of removed code) accepted as recommended. The spec amendments below are applied to `spec.md` section 11.


## Context

The Stage 2 spec was accepted on 2026-11-01. This plan starts Stage 3 (Build). The spec has six phases (A to F). This plan details **Phase A: identity, safety, clean-up and re-skin**. Phases B to F get their own plan sections, written and approved when each starts, so that each phase stays a separate gate (spec section 9).

Phase A is the largest phase and the one that changes the most files, but it adds no new user feature. It makes the app a separate product (own name, own data folder, no upstream scaffolding) and gives it the paper look. Everything later builds on it.

Plan-mode exploration found facts that change the spec's assumptions. They are listed under "Spec amendments" and will be applied to `spec.md` in the same commit as `plan.md`.

## Decisions for you at this gate

* **P1 Bundle identifier: `com.openedtools.notes`.** It is deliberately not derived from the app name. The identifier names the data folder, so if Quire later becomes Marginalia, a name-derived identifier would force a second data move. With this one, a rename only changes display names.
* **P2 Branch:** work on `enhance/quire-phase-a`, created from the current `feature/speakers-folders-people` head, so the speaker features stay a clean base. One commit per work package. No pull request unless you ask.
* **P3 Pre-existing failing tests.** 19 of 45 frontend tests already fail on the current branch (details below). Recommended: classify them in WP1, delete only tests of code we remove, and bring every other failing test to green by fixing the cause. You see the list at checkpoint A1.

## Baseline recorded during planning

| Check | Result now |
|---|---|
| `npx tsc --noEmit` (frontend) | Clean, 7.5 s |
| `bun test` (frontend) | 26 pass, **19 fail**, 45 total. Failing files: `tests/lib/analytics.test.ts`, `tests/hooks/summary-generation.test.tsx`, `tests/hooks/meeting-details-refresh.test.tsx`, `tests/lib/summary-language-preferences.test.js` (a "Consumer failed" error and mock failures). Whether they also fail on the upstream base commit is unknown. |
| `pnpm lint` | Does not work. `eslint` and `eslint-config-next` are not installed. |
| Rust tests | About 330 inline tests (245 `#[test]`, 85 `#[tokio::test]`). WP1 records the passing count with the known environment recipe (system ONNX Runtime, stub `llama-helper`). |
| CI | None runs tests. All 8 workflows are manual builds. |
| Rust dialog plugin | `tauri-plugin-dialog 2.3.0` already in `Cargo.toml`. |

## WP1 baseline results (recorded 2026-11-01)

| Check | Result |
|---|---|
| `cargo test --lib` (frontend/src-tauri) | **322 passed, 0 failed, 4 ignored**, exit 0. Two ignored tests need the speaker model and recordings, two need audio hardware or files. |
| `bun test` (plain) | 26 pass, 19 fail. The same 19 fail at upstream's release commit `a2cb62e`, so they predate this fork. |
| Cause of the 19 | Not code defects. `mock.module` leaks between test files in one bun process. Every file passes alone (45 of 45). |
| Fix | `pnpm test` runs `bun test --isolate` (fresh global per file) plus `node --test` for the one node test file. No test was edited or removed. 45 + 1 pass. |
| `tsc --noEmit` | Clean. |

P3 therefore needed no test deletions. The two tests in `tests/lib/analytics.test.ts` go away in WP4 together with the module they test.

## Spec amendments found while planning

1. **F-HK-6:** `docs/ARCHITECTURE.md` would collide with the existing `docs/architecture.md` on case-insensitive Windows. Upstream's long CLAUDE.md moves to `docs/UPSTREAM_NOTES.md` instead. Its link to the missing `AUDIO_MODULARIZATION_PLAN.md` is dropped.
2. **AC-HK-6 hook:** the hook locks existing tests when `LOCK_TESTS=1` is set (for bug-fix tasks), not when `ALLOW_TEST_EDITS` is unset. The spec's wording would block writing new tests during feature work.
3. **F-UI-4 fonts:** bundled through pinned `@fontsource-variable/source-serif-4` and `@fontsource-variable/source-sans-3` packages (both OFL). The files ship in the build and no network is needed at build or run time after install. BlockNote's own Inter font import is overridden.
4. **Token name:** the spec token `muted` is `ink-muted` in CSS and Tailwind, because shadcn already uses `muted` for a background.
5. **F-ID-4 adoption** is more than a data-folder copy. A new identifier looks like a first launch, so onboarding would run and the models (several GB) would be re-downloaded. Adoption therefore also copies the stores (`onboarding-status.json`, `recording_preferences.json`), copies model files (hard links would let one app truncate a file the other uses), reads the old database through `VACUUM INTO` (consistent even if the old app is open), and runs **before** any app state exists. Three data locations are hard-coded and do not follow the identifier: `Meetily/templates`, `meetily/notifications.json`, and `meetily-recordings`. All move into the app data folder (existing recordings keep their saved absolute path).
6. **F-HK-1:** also remove `test_backend_connection`, `debug_backend_connection`, `get_server_address`, the `localhost:5167` CSP entry and the sidebar's `serverAddress`, which only served the archived backend.
7. **F-HK-3:** the dead list is longer: `audio/stt.rs`, `audio/system_audio_stream.rs`, `audio/system_audio_types.ts`, `whisper_engine/_stderr_suppressor.rs`, `audio_v2/`, `CLEANUP_PLAN.md`, `build_backup.bat`, `app/metadata.ts` and `.tsx`, the empty `AudioPlayer.tsx`. Each is deleted only after `cargo check` and `tsc` confirm nothing references it. The new migration drops both `licensing` and `meeting_notes`. Old migrations are never edited (sqlx checks their checksums).
8. **About screen:** the upstream marketing block ("Chat with the Zackriya team") and the analytics switch are removed, not restyled.
9. **Acceptance criteria by phase:** AC-HK-3 (network inventory) moves to Phase C and AC-HK-4 (logs) to Phase B. AC-NF-1 is split by phase. The mapping is at the end of this plan.
10. **Parakeet v3 download host:** the model comes from an upstream-hosted address (`meetily.towardsgeneralintelligence.com`). If that host disappears, onboarding breaks. This is recorded for Phase C (F-MOD-8), not acted on here.

## Order of work

Four checkpoints. After each one you can build the installer (`frontend\build_installer_windows.bat`) and try it. Every step leaves `cargo check --lib` and `tsc` green.

The order is chosen so that **no new build touches the data of the Meetily you use today**: the identifier switch and the database backup come first.

### A1. Safety and isolation

**WP1. Baseline and process** (no behaviour change)
* Record Rust and frontend baselines. Classify the 19 failing frontend tests by running them in a temporary worktree at the upstream base commit.
* Replace `CLAUDE.md` with a one-page version: build, test and lint commands, the Rust environment recipe, conventions, the audio-pipeline map, things Claude gets wrong, and a verify-before-done block. Move upstream's to `docs/UPSTREAM_NOTES.md`.
* Add `.claude/settings.json` and `.claude/hooks/protect-tests.sh` (blocks edits to existing test files when `LOCK_TESTS=1`, exits 2 with a reason, `onFailure: block`), and deny reads of `.env*`.
* Add scripts: `typecheck` (`tsc --noEmit`), and a `lint` runner that executes the custom checks as they land (dashes, brand, colours, contrast, brand drift). `lint:dashes` lands now with an allowlist that shrinks to empty in WP4.
* Files: `CLAUDE.md`, `docs/UPSTREAM_NOTES.md`, `.claude/*`, `frontend/package.json`, `frontend/scripts/lint/*`.

**WP2. Database backup before migrations** (F-HK-9). Tests first.
* In `DatabaseManager::new` (`src-tauri/src/database/manager.rs`), after the legacy copy or create step and before `sqlx::migrate!(...).run`: if `meeting_minutes.sqlite` exists and `_sqlx_migrations` is behind the newest embedded migration, copy the database (with its `-wal` and `-shm` files, before the pool opens) to `backups/meeting_minutes.<target version>.<utc>.bak`, then keep the newest 3.
* Idempotent: `new()` can run twice through the existing retry path, so a backup for the same target version is not repeated and never rotates out a good one. Do the backup before the retry code that deletes WAL files.
* Tests: none for a fresh database; none when up to date; created when behind; WAL included; second call no-op; rotation keeps 3.

**WP3. Brand config, identifier, data adoption** (F-ID-1 to F-ID-4, F-ID-9)
* `brand/brand.json`: `name`, `slug`, `bundleId` (P1), `windowTitle`, `tagline`, `vaultDirName`, `upstreamCredit`, and a `legacy` block (old identifier `com.meetily.ai`, old template and notification directories, old recordings folder name, old temp prefix).
* `frontend/scripts/brand-apply.mjs` writes `productName`, `mainBinaryName`, `identifier` and the window title into `tauri.conf.json` by targeted key edits (no reformatting), `frontend/src/lib/brand.generated.ts`, and `src-tauri/src/brand_generated.rs`. `brand:check` fails on drift. Tests with `bun:test`.
* Replace every user-visible "Meetily" (about 39 strings: `Logo.tsx`, `Info.tsx`, `LegacyDatabaseImport.tsx`, `notifications/types.rs`, `notifications/commands.rs`, `tray.rs`, two network `User-Agent` strings and the test that asserts one) with brand constants. The macOS `log stream --process meetily` commands use the brand slug.
* Legacy constants move into `legacy.rs` and `legacy.ts`. The `.meetily_decode_` prefix is created with a new prefix, and the deletion allowlist in `api/recording_files.rs` matches both. The IndexedDB name and the two storage keys are renamed (the new identifier has a fresh WebView profile, so the old ones are unreachable anyway).
* Path module: templates (`summary/templates/loader.rs`), notification settings (`notifications/settings.rs`) and the engine fallback paths move under `app_data_dir`.
* **Adoption** (`src-tauri/src/adopt.rs`), called at the top of `run()` before the Tauri builder, using `rfd` (already a transitive dependency of the dialog plugin) for one Copy or Start fresh prompt:
  1. Conditions: the old folder has `meeting_minutes.sqlite`, the new folder has none, and no marker file exists.
  2. Copy into a temporary sibling folder, then rename into place, so an interrupted copy leaves nothing half-made.
  3. Database through read-only `VACUUM INTO`, refused if it comes from a newer version than this build understands. Model files, stores and the legacy template and notification folders by copy (recordings are not copied; both apps keep using the same audio files, and the offer says so).
  4. Write an `.adopted` or `.declined` marker. The old folder is never modified or deleted.
* Tests first, in a temporary tree: copy correctness, source byte-identical afterwards, interrupted copy leaves no partial target, second run is a no-op, declined is remembered, pure decision function covers each condition.
* Not testable here (manual Windows checklist below): the real dialog, the old app running while adoption happens, and the WebView profile (not copied; the dialog says unsaved recovery data does not carry over).
* Files: `brand/*`, `frontend/scripts/brand-apply.mjs`, `tauri.conf.json`, `src-tauri/src/{adopt,legacy,brand_generated,lib,paths}.rs`, the string sites above.

**Checkpoint A1.** Ryan builds and runs the Windows checklist. Success: the new app starts beside the old one, offers to bring the data over, shows the meetings, people and folders, skips onboarding, and the old app still works unchanged.

### A2. Clean-up (no visible change)

**WP4. Removals**, in this order, each its own commit:
1. Dead files (list in amendment 7). Delete, then `cargo check` and `tsc` decide.
2. Updater: `lib.rs` plugin lines for `updater` and `process`, both Cargo dependencies, the tray item `check_updates` and its handler, `tauri.conf.json` (`updater:default`, `process:default`, the plugin block), `UpdateDialog`, `UpdateNotification`, `updateService`, `useUpdateCheck`, `UpdateCheckProvider` (and its layout wiring), the About button, and both npm packages.
3. Pro and backend leftovers: the three profile commands with their structs and registration, `make_api_request`, the two backend-connection commands, `APP_SERVER_URL`, the CSP entry, and the sidebar `serverAddress`. Check `ModelSettingsModal.tsx` before removing the prop.
4. Telemetry in three stages so `tsc` guards each one: (a) replace `lib/analytics.ts` with a no-op stub; (b) remove the 94 call sites in 26 files, plus `AnalyticsProvider`, `AnalyticsConsentSwitch`, `AnalyticsDataModal` and the meeting-count helpers in `useRecordingStop.ts`; (c) delete the stub, the `analytics/` Rust module, its 25 registered commands, the `posthog-rs` dependency, the analytics block in `recording_commands.rs` (about lines 902 to 1002, with `classify_device_type`), and rewrite `PRIVACY_POLICY.md` to what the app actually does. Delete the analytics test file and fix the two tests that import it.
5. Migration `20261102000000_drop_unused_tables.sql`: `DROP TABLE IF EXISTS licensing; DROP TABLE IF EXISTS meeting_notes;`. The WP2 backup runs before it. Test that it applies after all earlier migrations (four existing tests already run the full chain in memory).
6. Em dashes (119 in 21 files): comments and log strings become ` - ` mechanically. The 6 user-visible strings, the 3 error strings and the 3 LLM prompt lines in `summary/processor.rs` are edited by hand, and the processor tests (30) must stay green. Then `lint:dashes` has an empty allowlist.
* Proof: `cargo test --lib` count equals baseline minus the 3 analytics tests plus new tests; `tsc` clean; frontend tests as classified in WP1.

**Checkpoint A2.** Build, behaviour unchanged. Settings, recording and summaries still work.

### A3. Look and feel

**WP5. Tokens, fonts, theme**
* `frontend/src/styles/tokens.css` (CSS variables for light and dark, the spec 3.2 values plus `sage`, `sage-tint`, `scrim`, `disabled`, hover and pressed variants) and `tokens.contrast.json` (each pair with its role). `globals.css` aliases the shadcn variables to the tokens. `tailwind.config.js` adds semantic colours (`paper`, `surface`, `line`, `line-strong`, `ink`, `ink-muted`, `accent`, `accent-strong`, `on-accent`, `sage`, `sage-tint`, `danger`, `success`, `warning`) and a serif family. Retoken the `.bn-*`, `.ProseMirror` and scrollbar rules in `globals.css`.
* Fonts via the two fontsource packages. Heading serif is a base style on `h1` to `h5` (74 headings), not per-component edits.
* **Theme provider (new, there is none today):** a small provider sets `.dark` on `<html>`, follows `prefers-color-scheme`, stores the override, and calls the Tauri window `setTheme` (adds `core:window:allow-set-theme`). Remove the forced `"theme": "Light"`. A Light, Dark, System control goes in the General settings tab. Wire BlockNote's `theme` prop (hard-coded `light` in `BlockNoteEditor/Editor.tsx` and `AISummary/BlockNoteSummaryView.tsx`) and the sonner toaster.
* `scripts/lint/contrast.mjs` reads the two token files and fails below role thresholds. `bun:test` unit tests pin known ratios (ink on paper 13.89, `accent-strong` on paper 5.11, and so on).

**WP6. Re-skin.** `scripts/codemod/palette.mjs` rewrites raw classes using a mapping table and prints a report of anything it could not map. The mapping, in principle:

| Raw class family | Becomes |
|---|---|
| `gray-50`, `gray-100` backgrounds | `surface` |
| `white` backgrounds | `paper` (cards) |
| `gray-200` borders and dividers | `line` |
| `gray-300` control borders | `line-strong` |
| `gray-600` to `gray-900` text | `ink` or `ink-muted` by shade |
| `blue-600` fills with white text | `accent-strong` with `on-accent` |
| `blue-50` to `blue-100` fills | `sage-tint` (info) or an accent tint |
| red, green, amber, yellow, orange | `danger`, `success`, `warning` |
| `bg-black/80` overlays (dialog, sheet) | `scrim` |

* Hand work the codemod cannot do: the 8 speaker colours in `lib/speakers.ts` (a designed set of speaker tokens with 4.5 text contrast), about 70 state-dependent colour ternaries, 10 gradients, 2 SVG strokes in `ModelDownloadProgress.tsx`, and the 5 `button.tsx` variants.
* One directory per commit, in this order: Sidebar and Logo (98), settings (`app/_components` 86, `app/settings`, preference components), People and Speakers (171), MeetingDetails and AISummary and the BlockNote views (98), onboarding (94), model managers and dialogs (the large top-level group, 714), `components/ui`. Then `lint:colors` turns on with an empty allowlist.
* Screens are verified with a repo-owned Playwright harness (`frontend/tests/ui/`, a mocked Tauri `invoke` like the one used for the earlier browser checks). It captures record, meeting, sidebar, people, five settings tabs, onboarding steps and the main dialogs, in light and dark at 1280 by 800. I review them against the spec tokens and the aesthetic reference, and the screenshots are attached to the pull request.
* Keyboard and target-size checks run in the same harness.

**WP7. Identity surfaces**
* About rewritten: wordmark, version from `getVersion()`, "Based on Meetily by Zackriya Solutions, MIT licence", the full MIT text (bundled `LICENSE.md`), and a third-party notices page (`THIRD_PARTY_NOTICES.md`: Parakeet CC BY 4.0 attribution, WeSpeaker, Whisper, ONNX Runtime, llama.cpp, Qwen, Gemma terms, ffmpeg).
* `Logo.tsx` becomes a serif wordmark. Delete `public/logo.png` and `logo-collapsed.png` if unreferenced. Metadata and window title come from brand.
* Icons: render `icon-concept.svg` to 1024 px with Playwright, then `pnpm tauri icon` produces the full set (`.ico`, `.icns`, Store logos). Add `bundle.windows.nsis` with `installerIcon`, `sidebarImage` and `headerImage`, using plain on-brand BMPs generated from the SVG until you supply your own generated images (sizes are in spec section 3.9).
* Optional (F-UI-10, could): sidebar illustration if you provide it.

**Checkpoint A3.** Build and look. Review light and dark by eye.

### A4. Remaining Phase A items and close-out

**WP8. Recordings folder setting** (F-HK-7, should). Tests first.
* `recording_saver.rs` (`initialize_meeting_folder`) and `audio/import.rs` load the saved preference instead of calling `get_default_recordings_folder()`.
* Settings gets a folder picker (`select_recording_folder` is a stub today; use `tauri-plugin-dialog`). The picker warns when the path is inside OneDrive, iCloud Drive or Dropbox (detected from the known folder names under the user profile).
* New installs default to `<app data>/recordings`. Adopted or older installs keep their saved absolute path, because adoption copies `recording_preferences.json`.
* Deleting a meeting removes its audio. Recordings from an adopted install live in the earlier app's folder and are shared with it, so the new app must not delete files outside its own recordings folder without asking. Until then the adoption offer warns about it.
* Add the folder to the asset and fs scopes only if a later phase needs the frontend to read it (none today).

**WP9. Close-out**
* `scripts/check-traceability.mjs` (spec section 4), wired into `lint`.
* Full verification (below), independent review by a subagent that did not see the build, review findings fixed or accepted, and the manual Windows checklist handed to you.
* Ask you to run `ffmpeg -version` on the bundled binary for AC-HK-8 (the build environment cannot reach the download host).

## Files that change (patterns)

* Rust: `lib.rs`, `database/manager.rs`, `api/api.rs`, `audio/{recording_saver,import,decoder,recording_commands,recording_preferences}.rs`, `tray.rs`, `notifications/*`, `summary/templates/loader.rs`, new `adopt.rs`, `legacy.rs`, `paths.rs`, `brand_generated.rs`; deletions listed in WP4.
* Frontend: about 84 files for colours (one pattern, described once above); 26 files for analytics call sites; `layout.tsx`, `globals.css`, `tailwind.config.js`, `About.tsx`, `Logo.tsx`, `Sidebar/*`, settings components; new `styles/`, `scripts/`, `tests/ui/`, `brand/`.
* Config: `tauri.conf.json`, `package.json`, `Cargo.toml`, `.claude/*`, `docs/*`.

## Reuse (do not rebuild)

* The allowlist and marker logic in `api/recording_files.rs`, the SHA-256 pinning pattern in `diarization/model.rs` and `build/ffmpeg.rs`, the `Busy` guard pattern in `diarization/commands.rs`, `tauri-plugin-dialog` for the picker, the existing in-memory migration test pool in `database/repositories/*`, `lucide-react` for icons.

## Risks

1. **Identifier and adoption (riskiest).** Hard to prove on Linux. Mitigations: copy-only, markers, temporary folder then rename, old data never touched, tests on a temporary tree, a Windows checklist at A1. Tripwire: if the checklist fails and the cause is not quickly fixable, fall back to D2's alternative (keep the old identifier, change only visible names), which is a one-line change in `brand.json`.
2. **Re-skin regression.** 1,441 class uses in 84 files. Mitigations: codemod with an unmapped report, one directory per commit, screenshots in both themes, no logic edits in these commits.
3. **Telemetry removal.** 94 call sites. The stub stage lets `tsc` guard each removal. The three direct `invoke` calls in the consent switch go away with it.
4. **Migration ordering.** The backup precedes the drop migration. Older builds cannot open a database migrated by newer ones, which is why the identifier switch (separate data folder) comes first.
5. **LLM prompt text.** Dash edits in `summary/processor.rs` change model input. Hand edited, with the 30 processor tests as a guard.
6. **Dependencies.** Two fontsource packages and `rfd` as a direct dependency, all pinned. `posthog-rs`, the updater and process plugins, and two npm packages are removed. Lockfiles reviewed in the diff.
7. **Not testable here:** Windows dialog, WebView profile behaviour, NSIS visuals, the installer. Covered by the checklist.
8. **Existing red tests.** The 19 failures are unexplained. If the base commit is also red, they are fixed in WP1 or reported to you, never skipped silently.

## Options rejected

* Keep `com.meetily.ai` internally (D2's alternative): simpler, but leaves the upstream name in the data path and tray process. Kept only as the tripwire fallback.
* Edit or delete the old licensing and notes migrations: breaks sqlx checksum checks for every existing database.
* A TypeScript-AST codemod (ts-morph): heavy for class strings; a regex mapping with a report and screenshots is enough.
* Hand-placing font files with `next/font/local`: more steps than the pinned fontsource packages for the same result.
* Doing adoption last: the isolation it gives is worth more than deferring risk.
* A native modal dialog from inside `.setup`: blocking dialogs on the main thread can deadlock, so adoption runs before the builder.

## Proof (what runs before anything is called done)

Commands, from the repository root unless noted. The Rust recipe (system ONNX Runtime, stub `llama-helper`) goes into the new `CLAUDE.md`.

* `cargo check --lib` and `cargo test --lib` in `frontend/src-tauri`. Report passes against the WP1 baseline, with analytics tests removed and new tests added.
* In `frontend`: `pnpm typecheck`, `pnpm lint` (dashes, brand, colours, contrast, brand drift, traceability), `bun test`, and the Playwright UI suite.
* `pnpm brand:apply` with the name changed to "Marginalia", then `pnpm brand:check`: only `brand.json` and generated files differ (AC-ID-1).
* Independent review of the diff against `spec.md` and this plan, in a fresh subagent.

Windows checklist for you at each checkpoint:
* A1: install beside the old app; the adoption prompt appears; choose Copy; meetings, people, folders and models are present; no onboarding; the old app still opens its own data.
* A3: light, dark and system themes switch (including the title bar); the About screen shows the credit; the icon is the new one in the taskbar, tray and installer.
* A4: choose a recordings folder; a recording lands there; run `ffmpeg -version` on the bundled binary and send me the `configuration:` line.

## Acceptance criteria covered by Phase A

AC-ID-1 to AC-ID-6, AC-UI-1 to AC-UI-8 (AC-UI-9 only if the illustration is supplied), AC-HK-1, 2, 5, 6, 7, 8, 9, AC-NF-3, AC-PROC-1, AC-PROC-2, and the startup-time part of AC-NF-1.

## Later phases (each gets its own plan section and gate)

| Phase | Content | Acceptance criteria |
|---|---|---|
| B. Privacy core | Retention, sweep, finalizer, transcript-only, consent reminder, first-run notice | AC-RET-1 to 15, AC-TO-1 to 4, AC-CON-1 to 3, AC-HK-4, sweep and reminder parts of AC-NF-1 and AC-NF-2 |
| C. Models | Models screen, cloud switch, Credential Manager, download manifest (and the Parakeet host question) | AC-MOD-1 to 6, AC-QA-5, AC-HK-3 |
| D. Live Q&A | Benchmark in two profiles (CPU and CUDA for the RTX 3070 Ti), context builder, Ask box | AC-QA-1 to 4, AC-QA-6, AC-QA-7 |
| E. Vault export | Format library and fixtures, one-way export, Sync screen, disclosure | AC-SYNC-1, 2, 5, 6, 8, 9, export part of AC-NF-1 and AC-NF-2 |
| F. Two-way sync | Import, conflicts, deletion propagation, after the device checks | AC-SYNC-3, 4, 7 |
| WP5 | Tokens from spec 3.2 plus hover and pressed variants | Also `surface-hover`, `accent-tint`, `danger-tint`, `success-tint`, `warning-tint`, eight speaker pairs and HSL copies of eleven tokens for BlockNote. All text pairs are in `tokens.contrast.json` (89 checks) | The codemod needed tints for alerts and badges; BlockNote's precompiled CSS reads `hsl(var(--x))` |
| WP5 | Theme stored as an override | Stored in WebView local storage, applied before first paint by an inline script | No flash of the wrong theme; nothing to migrate |
| WP5 | shadcn `accent` | shadcn's own `accent` (a grey hover) is now `surface-hover` in the five ui components, so `accent` means terracotta as the spec says | Name clash |
| WP6 | One directory per commit with before and after screenshots | Seven codemod commits, then hand work, reviewed with one full screenshot pass in light and dark (before shots taken once, light only). The first codemod commit also carried the speaker tokens and the codemod script | A full capture per directory would have tripled the run time without changing the review |
| WP6 | Not in the plan | Tailwind now scans `src/lib`, `src/hooks` and `src/contexts`; BlockNote overrides moved outside `@layer base` | Speaker colours had never been generated, and Tailwind silently dropped layered rules for classes it never saw |
| WP6 | Not in the plan | Keyboard fixes: focus ring on the summary editor, 24 px speaker chips, switches and resize handle, hover-only buttons visible on focus, an accessible name on the sidebar toggle | Found by the keyboard check (AC-UI-4) |
| WP7 | Installer images until Ryan supplies generated art | Plain on-brand BMPs rendered from the icon SVG by `scripts/brand-assets.mjs` with Playwright's Chromium | No image library on the build machine |

Feedback from the A1 and A2 Windows tests, to place when those phases are planned:

* **Phase C:** a summary through a local OpenAI-compatible server (LM Studio) failed because Quire sends the whole transcript to a custom address in one request (`summary/service.rs`, threshold 100000). It needs the model's context size, from a setting or from the server, and chunking like Ollama. Also: let the built-in engine use a GGUF from a folder the person picks (for example LM Studio's), so models are not downloaded twice.
* **Speakers (phase to be decided):** speaker detection separated a male and a female voice but mixed up two female voices.
* **Build (decide before Linux builds):** there is no `.gitattributes`, so migration files have CRLF on Windows and LF on Linux, and sqlx checksums differ between the two. Databases made by Windows builds would be refused by a Linux build.

## Deviations from this plan (recorded as the work proceeds)

| WP | Plan said | What was done | Why |
|---|---|---|---|
| WP2 | Copy the database file with its `-wal` and `-shm` files before the pool opens | `VACUUM INTO` a `.part` file after the pool opens, then rename | One consistent snapshot, includes rows still in the write-ahead log, no raw file pairs to keep in step. Same guarantees, tested including a WAL case |
| WP1 | `.claude/hooks/protect-tests.sh` | `protect-tests.mjs`, run through `node` in exec form, and it also locks the inline `#[cfg(test)] mod` section of Rust files | Rust tests here are inline, so a path-only hook could not protect them. Exec form avoids quoting problems on Windows. Docs agent confirmed the settings keys and that `onFailure: block` needs Claude Code 2.1.295 or later (2.1.316 in use) |
| WP1 | A `lint` runner | Custom checks: dashes (ratchet allowlist), brand drift, brand mentions. `pnpm lint` no longer means `next lint`, which never worked (ESLint is not installed) | Plan baseline finding |
| WP3 | `legacy.rs` and `legacy.ts` modules | Legacy names live in the `legacy` block of `brand/brand.json` and surface as `LEGACY_*` constants in the generated Rust file; TypeScript needs none | One place for the names instead of two hand-written modules |
| WP3 | Move the default recordings folder name to the brand | The default folder name stays the earlier name (`LEGACY_RECORDINGS_DIR_NAME`) until WP8 | The recorder ignores the saved folder preference today, so changing the default early would silently move where adopted installs record. WP8 fixes both together |
| WP3 | `.meetily_decode_` prefix kept as a legacy constant | New prefix `.quire_decode_` (derived from the slug), and the deletion allowlist matches both | Leftovers from the earlier build are still cleaned |
| WP3 | Lint scope not stated | `lint:brand` scans product source, build scripts and configuration. README, docs, CI workflows and the archived backend are not scanned. `brand/allowed-mentions.txt` entries need a reason and a stale entry fails | Upstream documents legitimately describe the upstream product |
| WP3 | n/a | Removed a broken placeholder link (`your-org/meetily`, to a notice file that does not exist) from the Bluetooth playback warning; the macOS aggregate device name is now the neutral `system-audio-tap` | No product name where it was only decoration |
| WP3 | Models by hard link | Models are copied | Review finding: a hard link lets a failed re-download in one app truncate the file the other uses, which breaks "the old folder is never changed". Costs disk space and a minute of copying |
| WP3 | A Yes or No offer, a dismissal counts as No | Three answers: Esc or the window button is not a decision and the person is asked again; only the No button is remembered | Review finding: rfd returns Cancel on dismissal, which would have stored a permanent No |
| WP3 | Not in the plan | Adoption lock file (stale after one hour), refusal of a database from a newer version, retry of the final rename, a failure dialog that offers to open empty or close, folder size and a plain warning in the offer | Review findings on double launch, antivirus holding files on Windows, startup failing on every launch, and an offer that promised more than it did |
| WP2, WP3 | Not in the plan | A database failure at startup shows a window with the reason instead of closing silently; the backup rename retries | Review finding: a release build has no console |
| WP1 | Not in the plan | The test-lock hook normalises line endings and ignores case in paths; `brand:check` ignores line endings | Review findings for Windows checkouts |
| WP3 | n/a | `rfd` added as a direct dependency for Windows and macOS only; Linux adopts without asking | Linux is not a user platform here, and the dialog toolkit is not linked there |
| A1 (Windows) | Adoption reads the old database read-only | When the old app is closed, the old database is opened as immutable | Windows test finding: a read-only connection to a write-ahead-log database created `-wal` and `-shm` files in the old folder and could not remove them. Test added |
| WP4.3 | Remove the `APP_SERVER_URL` CSP entry and `serverAddress` | Also removed `localhost:8178` (old whisper server) from the CSP and the sidebar's `transcriptServerAddress` | Same kind of leftover, set once and never used |
| WP4.4 | Delete the analytics test file in stage (c) | Deleted in stage (a), with the stub | The stub made those tests fail; moving it keeps every commit green |
| WP4.4 | Dead-file list in amendment 7 | Also deleted `lib_old_complex.rs`, an old copy of `lib.rs` that is never compiled | Missed in the list; it was the last place naming the PostHog host |
| WP4.6 | Edit the 3 error strings by hand | The hot-swap errors are now full sentences ("the microphone switch was cancelled") | They can reach the person through the recording error path |
