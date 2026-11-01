# Spec: Quire desktop and shared vault (from intent.md 2026-11-01)

Status: draft for the Stage 2 gate. Date: 2026-11-01. Mode: solo (accept or revise).
Working name: Quire (backup: Marginalia). Companion file: `sync-format.md` (the shared vault contract).
Use limit: personal use, unclassified information only. Never for classified or CUI discussions.

## 0. How this spec relates to the intent

* `intent.md` is centred on the iPhone app and lists the desktop as "parity" for three features (live Q&A, transcript-only, retention). The chat brief adds desktop work that intent.md does not mention: a rename, a visual re-skin, a consent reminder redesign and a model-choice screen.
* This spec covers the desktop app and the shared vault format. The iPhone app is a separate project built on a Mac and is not specified here. `sync-format.md` is the contract it will implement.
* Proposed addendum to intent.md (not applied until you say yes): add "Desktop identity, design and model-choice screen" to Proposed outcome, add "iPhone implementation (separate repository and spec)" to Out of scope, and add open question 7 "Files as the source of truth or SQLite as the source of truth" (decision D1 below).

## 1. Current state of the fork (answers open question 1)

Inspection of branch `feature/speakers-folders-people` at commit `818ef52`.

| Area | Today |
|---|---|
| Notes, transcripts, folders, people | One SQLite database, `meeting_minutes.sqlite`, in the app data folder (`%APPDATA%\com.meetily.ai\`). Not files. |
| Tables in use | `meetings` (id `meeting-<uuid>`, title, timestamps, `folder_path` = on-disk recording folder, `folder_id`), `transcripts` (one row per line: text, timestamp, `audio_start_time`, `audio_end_time`, `duration`, `speaker` = slot key), `summary_processes` (status plus a JSON `result` whose `markdown` field is the summary text), `transcript_chunks`, `settings` and `transcript_settings` (provider, model, and API keys in plaintext columns), `people` (with notes), `meeting_speakers` (slot, person link, local voiceprint), `folders`. |
| Folders | Flat. Unique by name ignoring case. `meetings.folder_id` points at one; deleting a folder returns its meetings to unfiled. |
| Unused or dead | `meeting_notes` table (created, never referenced by Rust code). `licensing` table and `api_get_profile`, `api_save_profile`, `api_update_profile` commands carrying a `license_key` (upstream Pro scaffolding). `lib_old_complex.rs` and `core-old.rs` (old files). |
| Audio | Files, not database. Each meeting has a folder under the recordings folder (default `%USERPROFILE%\Music\meetily-recordings\<name>\`) holding the audio file, `metadata.json`, `transcripts.json` and incremental checkpoints. `meetings.folder_path` points to it. When `auto_save` is false the recorder discards audio chunks and still writes the transcript and metadata files. |
| Crash recovery | A WebView IndexedDB store (`MeetilyRecoveryDB`) holds live transcript lines until the meeting is saved to SQLite. |
| Transcription pipeline | Mic and system audio go to the audio pipeline (`audio/pipeline.rs`): a recording path (professional mixing, ducking) and a transcription path (voice activity detection, then Whisper via whisper-rs or Parakeet via ort). Defaults: `large-v3-turbo` and `parakeet-tdt-0.6b-v3-int8`. Speaker detection runs after the fact on the saved audio (WeSpeaker ONNX, clustering) and keeps voiceprints locally. |
| Summary pipeline | Transcript is chunked (default 40,000 characters), each chunk summarised, then combined with a template into markdown, optionally in a chosen language. Providers: OpenAI, Claude, Groq, Ollama, OpenRouter, CustomOpenAI, and BuiltInAI (a bundled `llama-helper` sidecar running GGUF models). |
| Model selection | Transcription provider and model are in `transcript_settings`, summary provider and model in `settings`. Built-in summary models: Qwen 3.5 2B (1,221 MB), Qwen 3.5 4B (2,614 MB), Gemma 3 4B (2,374 MB), Gemma 3 1B (1,019 MB), all with a 32,768 token context. The transcription settings screen and database already have slots for Deepgram, ElevenLabs, Groq and OpenAI transcription. I have not verified that these work end to end. |
| Network posture | The app opens no listening sockets (only tests bind to localhost). Telemetry code (PostHog) exists but the key is empty in this fork, so nothing is sent. The updater config points at this fork's releases but carries the upstream project's signing public key. |
| Consent notice | `ComplianceNotification.tsx`: a small popup saying "Recording Notice", "Inform participants about recording." and "US compliance required", with Later and Done buttons. No "never show again". |
| Branding in code | Name `meetily` in `tauri.conf.json` (productName, window title), `Cargo.toml`, `package.json`; identifier `com.meetily.ai`; 86 case-insensitive mentions in `src` and `src-tauri/src`; default folder `meetily-recordings`; temp prefix `.meetily_decode_`; IndexedDB name `MeetilyRecoveryDB`. |
| Styling | Tailwind with shadcn variables in `globals.css` (grey scale). 1,283 uses of raw palette classes (`bg-gray-100`, `text-blue-600` and so on) in 81 `.tsx` files, plus 5 hard-coded hex colours. Body font Source Sans 3 loaded through `next/font/google`. |
| Em dashes | 119 in `.ts`, `.tsx` and `.rs` source (29 and 90), 0 in `docs/`, `README.md` and `CLAUDE.md`. Other markdown, scripts and the archived backend are not counted yet. |
| Repo guidance | `CLAUDE.md` is upstream's 409-line document. It describes a legacy backend and is far over the one-page guidance. |

Answer to open question 4: yes. The desktop needs (a) a vault module that reads and writes the shared format, (b) new meeting columns for retention and mode, (c) a retention sweep, (d) a Sync settings screen, and (e) a Retention setting. Windows needs the same retention setting as every other platform.

## 2. Requirements

Priority: M = must for this spec, S = should (deferrable at the gate), C = could.

### 2.1 Identity (F-ID)
1. **F-ID-1 (M)** The app name, slug, bundle identifier, window title, installer name, default recordings folder name, vault folder name and About text come from one file, `brand/brand.json`. Changing the name is a one-line edit plus one command.
2. **F-ID-2 (M)** A generator, `pnpm brand:apply`, writes the brand values into `tauri.conf.json` and a generated frontend module. `pnpm brand:check` fails if the generated files drift from `brand.json`.
3. **F-ID-3 (M)** No user-visible string hard-codes the app name. They read it from the brand module.
4. **F-ID-4 (M)** On first launch under a new identifier, if the old data folder (`com.meetily.ai`) exists and the new one does not, the app offers to copy the database, settings and downloaded models into the new folder. It copies, never moves, and leaves the old folder untouched. Legacy names stay recognised: the `.meetily_decode_` temp prefix in the deletion allowlist, and the `MeetilyRecoveryDB` store for unsaved-meeting recovery.
5. **F-ID-5 (M)** An About screen shows the name, version, "Based on Meetily by Zackriya Solutions, MIT licence", a link to the full MIT text, and third-party notices.
6. **F-ID-6 (M)** The upstream MIT copyright and permission notice stays in `LICENSE`. A second copyright line for your modifications may be added (your choice of holder name).
7. **F-ID-7 (M)** No Meetily logo, icon or name is used as branding. All icons (app, tray, installer images) are replaced by the original icon (section 3.2).
8. **F-ID-8 (M)** No code, text or assets from Meetily's paid Pro tier. The Pro scaffolding found in section 1 is removed (F-HK-1).
9. **F-ID-9 (M)** The remaining allowed mentions of "Meetily" are listed in `brand/allowed-mentions.txt`: `LICENSE`, the About credit, third-party notices, legacy-compatibility constants, and documents that describe upstream. A lint fails on any other mention.

### 2.2 Visual design (F-UI)
1. **F-UI-1 (M)** Colours, type, radii and spacing are design tokens in one file, `frontend/src/styles/tokens.css`. Tailwind and the shadcn variables read from it. No component sets a raw colour.
2. **F-UI-2 (M)** Light and dark themes. Default follows the operating system. The user can force light or dark in Settings.
3. **F-UI-3 (M)** Every text and control colour pair meets WCAG 2.2 AA: 4.5:1 for normal text, 3:1 for large text, icons and control borders. A script computes the ratios from `tokens.css` and fails the build below the thresholds.
4. **F-UI-4 (M)** Serif for the wordmark and headings, sans for body text. Both are open-licence fonts bundled in the repository, so the app and its build need no network.
5. **F-UI-5 (M)** Generous spacing on an 8 px grid, soft corners (10 px cards, 8 px controls), no drop shadows except a visible focus ring.
6. **F-UI-6 (M)** Few controls: each main screen has one primary action, and secondary actions sit in an overflow menu or settings.
7. **F-UI-7 (M)** The 1,283 raw palette class uses are replaced with semantic token classes by a codemod, screen by screen. A lint rule then forbids raw palette classes and hex colours outside `tokens.css`.
8. **F-UI-8 (M)** Reduced-motion and keyboard use work: visible focus on every control, no interaction needs a pointer, animations stop when the system asks.
9. **F-UI-9 (S)** An original icon concept (section 3.2) is finalised as an SVG master, and all raster sizes and the `.ico` are generated from it by a script.

### 2.3 Audio retention (F-RET)
1. **F-RET-1 (M)** A Retention setting with six values: Off, 24 hours, 7 days, 14 days, 30 days, Keep. Plain-language description under each. The default for new installs is 7 days (decision D7).
2. **F-RET-2 (M)** At recording end the app saves the policy on the meeting: `audio_retention`, `audio_expires_at` (UTC, recording end plus the period, null for Keep), `audio_state` (`none`, `present`, `deleted`), `audio_deleted_at`, and `mode` (`audio` or `transcript_only`).
3. **F-RET-3 (M)** Off means: delete the audio as soon as the transcript is safely saved. "Safely saved" means the transcript rows are committed to SQLite, the vault copy is written if sync is on, and automatic speaker detection has finished if the user has it enabled. If any of these fails, the audio is kept and the user is told. Audio is never deleted before the save is confirmed.
4. **F-RET-4 (M)** A cleanup sweep runs at app launch (after the database is ready), every 60 minutes while the app runs, and when the window regains focus after being hidden. It compares `audio_expires_at` with the current UTC time, so a sleeping computer catches up on wake.
5. **F-RET-5 (M)** Deleting audio removes every audio artefact for the meeting: the final audio file, checkpoints, temporary decode files, and the synced copy in the vault if audio sync is on.
6. **F-RET-6 (M)** Deletion is verified. After removing a file the sweep checks that it no longer exists. Only then is `audio_state` set to `deleted`. A locked or failed deletion stays `present`, is retried on the next sweep, shows as "deletion pending" in the meeting, and is logged without any transcript text.
7. **F-RET-7 (M)** Safety: deletion only ever touches files inside a meeting folder that carries the app's marker files, only known audio and temp extensions, never follows symbolic links or reparse points, and never leaves the recordings root (the existing allowlist in `api/recording_files.rs` is reused and extended).
8. **F-RET-8 (M)** Changing the setting affects future recordings. "Apply to existing recordings" is a separate button that shows how many recordings and how much space it affects and asks for confirmation. A single meeting can be set to "Keep this one" or "Delete audio now" from its menu. Extending is possible only before expiry.
9. **F-RET-9 (M)** Every meeting shows its audio status in words: "Audio kept", "Audio deleted on 8 Nov", "No audio saved".
10. **F-RET-10 (M)** The settings text says plainly what deletion cannot do: it removes the file from this computer and from the vault folder, but cloud recovery areas such as iCloud's 30-day "Recently Deleted", backups and low-level disk recovery are outside the app's control. It also says the sweep only runs while the app runs.
11. **F-RET-11 (S)** An opt-in Windows Scheduled Task runs `quire.exe --retention-sweep` once a day with no window, so deadlines are enforced even if the app is not opened. The setting explains what it installs and has a one-click remove.
12. **F-RET-12 (M)** Deleting a meeting deletes its audio and updates the vault (the audio part of this already exists).

### 2.4 Transcript-only mode (F-TO)
1. **F-TO-1 (M)** A "Transcript only: do not save audio" switch on the record screen, remembered between sessions.
2. **F-TO-2 (M)** In this mode no audio file, checkpoint or temporary audio file is created at any point during or after the recording. Audio is processed in memory for transcription and then discarded. The meeting is saved with `mode: transcript_only` and `audio_state: none`.
3. **F-TO-3 (M)** Features that need audio are disabled with a reason: detecting speakers after the fact, Enhance or retranscription, and audio playback. Manual speaker labels, renaming, folders, people and summaries all still work.
4. **F-TO-4 (M)** Wording, in the app: "Transcript only saves no audio file. It does not change your duty to tell people the conversation is being recorded or transcribed." No claim that it avoids legal risk.
5. **F-TO-5 (M)** Transcript-only is not identical to retention Off (decision D4). Off briefly stores audio so speaker detection and voice memory can run before deletion. Transcript-only never stores it. The UI shows them as two clear options and explains the difference in one sentence.

### 2.5 Live Q&A (F-QA)
1. **F-QA-1 (M)** An "Ask" box, collapsed by default, on the live recording screen and on saved meetings. A question returns an answer based on the transcript so far (live) or the whole transcript (saved).
2. **F-QA-2 (M)** The default model is local (BuiltInAI, or Ollama on this computer). With local models no network request is made. A cloud provider is used only when the cloud switch (F-MOD-5) is on and that provider is selected, and the Ask box then shows "Sends the transcript to <provider>".
3. **F-QA-3 (M)** The prompt contains the question, the most recent part of the transcript that fits about 60 percent of the model's context, and the best keyword matches (BM25) from earlier in the transcript that fit about 25 percent. The answer says which span it used when the transcript is longer than the window ("Answered from the last 25 minutes and 4 earlier passages").
4. **F-QA-4 (M)** Answers cite timestamps. The app supplies the segments with their times, and keeps only citations that fall inside a supplied segment. Clicking a citation scrolls to that line.
5. **F-QA-5 (M)** A recording is never interrupted by Q&A. Q&A is cancellable, runs at lower priority than transcription, and is refused with a clear message if the transcription backlog grows past a limit set after benchmarking (section 4, AC-QA-3).
6. **F-QA-6 (M)** Question and answer history is kept in memory for the session only. "Add to note" appends a labelled, AI-generated Q and A to the note body.
7. **F-QA-7 (M)** The model has no tools and cannot take actions. Transcript text is treated as untrusted content. Answers are rendered through the sanitizing markdown path and labelled as AI generated.
8. **F-QA-8 (M)** With no model installed, the box says "Download a model in Settings" and links to it.

### 2.6 Consent reminder (F-CON)
1. **F-CON-1 (M)** When a recording starts, a small non-blocking reminder appears near the record controls. It never blocks or delays the start.
2. **F-CON-2 (M)** It has a primary "Got it", a "Never show this again" checkbox, and a "Read more" link. Closing it without choosing just dismisses it for this recording.
3. **F-CON-3 (M)** "Never show this again" is stored locally and can be reversed in Settings under Privacy.
4. **F-CON-4 (M)** "Read more" opens an in-app page with the plain-language guidance and copy-ready wording (section 3.5). It also links to an external state-by-state reference, through the existing external-link allowlist.
5. **F-CON-5 (M)** The old text "US compliance required" is removed. No sentence in the app says the user is or is not compliant with any law.
6. **F-CON-6 (M)** A one-time first-run screen says: "For unclassified information only. Do not use this app for classified or controlled unclassified information." It asks for one tap to continue and stays readable in Settings under Privacy. This is a reminder, not enforcement.

### 2.7 Model choice (F-MOD)
1. **F-MOD-1 (M)** A Models screen with two parts: Transcription and Summaries and Q&A. Each model row shows name, download size, languages, a source link, and a status (installed, download, remove). Parakeet is listed first, then Whisper models.
2. **F-MOD-2 (M)** Language guidance: Parakeet v3 covers 25 European languages. When the chosen transcription language is outside that list, the screen recommends a Whisper model. The language list lives in a config file, not in code paths.
3. **F-MOD-3 (S)** API transcription (Deepgram, ElevenLabs, Groq, OpenAI slots already exist in the settings screen) is verified end to end. Each provider is labelled "Sends audio to <provider>" and is unavailable until the cloud switch is on. If verification shows a provider does not work, that provider is hidden rather than repaired (decision D8).
4. **F-MOD-4 (M)** Summaries and Q&A: local models first (the four built-in models with sizes, and Ollama), then cloud providers with a user-supplied key (OpenAI, Claude, Groq, OpenRouter, CustomOpenAI).
5. **F-MOD-5 (M)** A master "Allow sending data to cloud services" switch, off by default. Turning it on shows one confirmation that names what is sent (audio for transcription, transcript text for summaries and Q&A). Cloud providers are not selectable while it is off.
6. **F-MOD-6 (M)** API keys move from plaintext database columns to Windows Credential Manager. Existing keys are migrated on first launch and then cleared from the database. If the credential store is unavailable the app says so and does not fall back to plaintext.
7. **F-MOD-7 (M)** The Ollama and CustomOpenAI endpoints default to localhost. Saving a non-local host asks for confirmation, because the transcript then crosses the network.
8. **F-MOD-8 (S)** Model downloads verify a SHA-256 from an in-repository manifest (`models.json`), as the speaker model and ffmpeg already do. A mismatch deletes the file and refuses to load it.

### 2.8 Shared vault and sync (F-SYNC)
1. **F-SYNC-1 (M)** A Sync screen. Nothing is written anywhere until the user chooses a vault folder and confirms. On Windows the screen offers the iCloud Drive folder if it exists (`%USERPROFILE%\iCloudDrive`).
2. **F-SYNC-2 (M)** First enabling shows a plain disclosure: files in the vault are uploaded to iCloud by Apple's software, not by this app, and without Apple's Advanced Data Protection Apple holds the encryption keys. The user can cancel.
3. **F-SYNC-3 (M)** Once on, transcripts, notes, people and folders sync by default. Audio sync is a separate switch, off by default, with the limits in `sync-format.md` section 10 stated beside it.
4. **F-SYNC-4 (M)** Phase E: one-way export (database to vault) with the verification step in `sync-format.md` section 16. The database is not modified except for the new `sync_id` columns.
5. **F-SYNC-5 (M)** Phase F: two-way sync with the conflict, deletion and versioning rules in `sync-format.md` sections 11 to 13. Phase F does not start until the unverified items in `sync-format.md` section 15 are tested on the real PC and iPhone.
6. **F-SYNC-6 (M)** A Conflicts count and viewer. A conflict never discards data (the losing copy lands in `_conflicts`).
7. **F-SYNC-7 (M)** Files from the vault are untrusted input. The rules in `sync-format.md` section 14 are implemented and tested.
8. **F-SYNC-8 (M)** Retention is enforced on synced audio (F-RET-5) and by deadline on every device (`sync-format.md` section 10).
9. **F-SYNC-9 (M)** Turning sync off stops watching and writing and leaves the files in place. A separate, confirmed action removes the app's files from the vault.
10. **F-SYNC-10 (M)** Voiceprints, keys, models and settings never enter the vault.

### 2.9 Housekeeping (F-HK)
1. **F-HK-1 (M)** Remove Pro scaffolding: the `licensing` table (new migration that drops it), the profile commands and `license_key` structures, and the remote profile calls.
2. **F-HK-2 (M)** Remove the telemetry code, its dependency and its settings toggles, so that "nothing is sent" is provable from the dependency list.
3. **F-HK-3 (M)** Remove files confirmed unreferenced (`lib_old_complex.rs`, `core-old.rs`) and the unused `meeting_notes` table if nothing reads it.
4. **F-HK-4 (M)** Disable the updater until the fork has its own signing key. The upstream public key in `tauri.conf.json` means updates from this fork could not verify anyway.
5. **F-HK-5 (M)** Remove em dashes from app text and from documentation, and add a lint that fails on them (decision D5 sets the scope).
6. **F-HK-6 (M)** Replace `CLAUDE.md` with a one-page version: build, test and lint commands, conventions, folder map, things Claude gets wrong, and a verify-before-done block. Move upstream's long document to `docs/ARCHITECTURE.md`.
7. **F-HK-7 (S)** Move the default recordings folder for new installs to the app data folder (`recordings`), and warn when a chosen folder is inside OneDrive, iCloud Drive or Dropbox. Existing installs keep their configured folder (decision D3 in section 7).
8. **F-HK-8 (S)** Verify the licence of the bundled Windows ffmpeg build (section 7, concern 17).

### 2.10 Non-functional (NF)
1. **NF-1 Offline by default.** With sync off and the cloud switch off, the only network traffic is a model download the user starts. A network inventory test lists every outbound call site with the setting that enables it, and fails when a call site is added without an entry.
2. **NF-2 Accessibility.** WCAG 2.2 AA for contrast and keyboard use (F-UI-3, F-UI-8). Touch and click targets at least 24 by 24 px.
3. **NF-3 Performance.** A sweep over 1,000 meetings takes under 2 s when nothing is due. The consent reminder adds under 100 ms to starting a recording. Opening the app is not more than 10 percent slower than at commit `818ef52`. The one-way export of 500 meetings finishes in under 60 s on the target PC.
4. **NF-4 Reliability.** A crash or power loss during a sweep or an export leaves no half-written note or transcript (atomic writes) and no audio marked `deleted` that still exists.
5. **NF-5 Portability.** Windows 11 first. No Windows-only code outside `cfg(windows)` modules, and paths come from Tauri's path APIs, so a Mac build stays possible.
6. **NF-6 Privacy in logs.** Logs at every level contain no transcript text, question text, names, or API keys. A test searches captured logs after a scripted session.

## 3. Design

### 3.1 Brand configuration and rename
* `brand/brand.json` holds: `name`, `slug`, `bundleId`, `windowTitle`, `installerName`, `recordingsDirName`, `vaultDirName`, `tagline`, `upstreamCredit`. `bundleId` defaults to `com.<owner>.<slug>`.
* `scripts/brand-apply.mjs` reads it and writes `productName`, `mainBinaryName`, `identifier` and window titles into `tauri.conf.json`, and `frontend/src/lib/brand.generated.ts` for the UI. `--check` compares without writing. Crate and package names stay as they are, because renaming the Rust library breaks `RUST_LOG` filters and build scripts for no user benefit.
* Rust code that needs the name reads it from the generated module at compile time, not from string literals.
* The vault's format identifiers (`quire-vault`, `quire-format.json`, the `quire` front matter key) are deliberately not brand values and do not change on a rename (see `sync-format.md`).
* Data adoption (F-ID-4): the data folder name follows the Tauri identifier. On first launch under the new identifier the app finds the old folder, shows its size, and asks to copy it. The copy runs in a temporary folder and is moved into place only when complete, so an interrupted copy cannot leave a half database.
* Legacy constants stay in a single `legacy.rs` and `legacy.ts` with a comment saying why, and appear in `brand/allowed-mentions.txt`.

### 3.2 Design tokens and look

Corrected token values. The two values you suggested for muted text and accent text do not reach AA for small text (muted 3.64 on the background, accent 3.77). I kept your accent as a decorative colour and added a stronger terracotta for text and button fills. All ratios were computed.

Light theme

| Token | Value | Use | Contrast |
|---|---|---|---|
| `paper` | `#FAF6EE` | page background | n/a |
| `surface` | `#F2EBDD` | cards, sidebar, inputs | n/a |
| `line` | `#E4DACA` | decorative dividers | decorative only (1.28 on paper) |
| `line-strong` | `#897D6D` | borders of inputs and controls | 3.73 on paper, 3.39 on surface |
| `ink` | `#2B2622` | body text | 13.89 on paper, 12.62 on surface |
| `muted` | `#6B6155` (was `#8A7F72`) | secondary text | 5.62 on paper, 5.11 on surface |
| `accent` | `#C4623F` | icon, large shapes, large headings, focus accents | 3.77 on paper (passes 3:1 only) |
| `accent-strong` | `#A94E2B` | links, small accent text, primary button fill | 5.11 on paper, 4.64 on surface |
| `on-accent` | `#FAF6EE` | label on a primary button | 5.11 on `accent-strong` |
| `danger` | `#9E2B1F` | errors | 6.91 paper, 6.28 surface |
| `success` | `#3F6B3A` | confirmations | 5.77 paper, 5.24 surface |
| `warning` | `#8A5A00` | cautions | 5.50 paper, 5.00 surface |

Dark theme (warm charcoal, no pure black)

| Token | Value | Contrast |
|---|---|---|
| `paper` | `#1F1B17` | n/a |
| `surface` | `#2A2520` | n/a |
| `line` | `#3A332B` | decorative |
| `line-strong` | `#857A6B` | 4.07 on paper, 3.61 on surface |
| `ink` | `#EFE8DC` | 14.05 on paper, 12.46 on surface |
| `muted` | `#A89D8E` | 6.42 on paper, 5.69 on surface |
| `accent` and `accent-strong` | `#E08A66` | 6.52 on paper, 5.78 on surface |
| `on-accent` | `#1F1B17` | 6.52 on `accent` |
| `danger` | `#F08A7A` | 7.02 paper, 6.23 surface |
| `success` | `#8FC285` | 8.33 paper, 7.39 surface |
| `warning` | `#E0B05A` | 8.58 paper, 7.61 surface |

Values are starting points to adjust by eye. Any change must keep the contrast script green.

Type: Source Serif 4 (open licence) for the wordmark and headings, Source Sans 3 (already the app's body font) for everything else, both bundled as local font files through `next/font/local`. Scale 14, 16, 20, 28, 40 px; body line height 1.55. Spacing on an 8 px grid. Cards and panels 10 px radius, controls 8 px. No shadows; separation comes from `surface` against `paper` and from `line`.

Shape of the work: tokens first, then the codemod from raw palette classes to semantic classes, one screen at a time (record, meeting, sidebar, people, settings, dialogs), each reviewed with before and after screenshots in light and dark. The shadcn variables in `globals.css` become aliases of the tokens, so existing shadcn components inherit the look without edits.

Icon concept: `intent/private-meeting-recorder/assets/icon-concept.svg`. A terracotta Q on a cream tile, with a pale second ring behind it that suggests stacked folded sheets (a quire is a gathered set of folded sheets). It reads as a Q at 16 px, is a single flat shape, and is original. It becomes the SVG master in `brand/`.

### 3.3 Retention and the sweep
* New module `retention` in the Rust core with a pure decision function `due(meeting, now) -> Action` and a thin runner that does file operations. The clock is injected, so tests can set "now".
* Migration adds the columns in F-RET-2 to `meetings`. Existing meetings get `audio_retention = 'keep'`, `audio_state = 'present'` when they have a folder with audio, so nothing existing is deleted by upgrading.
* Off mode: the recording is written as usual (checkpoints make crash recovery possible). When the meeting is saved, a finalizer runs: save transcript, run automatic speaker detection if enabled, write the vault copy if sync is on, then call the deletion routine. Failure at any step leaves the audio and shows a notice.
* Transcript-only mode: the recorder is started with `auto_save = false` (the existing path that discards audio chunks), and the metadata files record `mode`. The work is mainly the switch, the metadata, the disabled-feature states, and tests that prove no audio file appears.
* Deletion routine: list candidate files in the allowlisted folder, remove each, check each is gone, update `audio_state` and `audio_deleted_at` in one transaction, write the vault note if sync is on. Locked files are retried on the next sweep.
* Sweep runner: tokio interval of 60 minutes plus launch plus window focus, guarded by a busy flag like the existing detection guard, so two sweeps never overlap.
* Optional scheduled task (F-RET-11): a `--retention-sweep` command line mode that opens the database, runs one sweep and exits without creating a window.

### 3.4 Live Q&A
* New Tauri command `api_ask_transcript(meeting_id, question)` and an event for streamed tokens. For a live meeting it reads the lines already committed to the live buffer; for a saved meeting it reads SQLite.
* Context builder (pure, tested): sort lines by time, take the newest lines up to the recent budget, run BM25 over the rest for the question, take top matches up to the retrieval budget, label every line with its speaker name and `mm:ss`.
* It reuses the existing LLM client and provider selection. The BuiltInAI sidecar handles one request at a time, so a Q&A request queues behind a running summary and says so.
* Citations: the model is asked to cite `[mm:ss]`. The app keeps only citations that fall inside a supplied segment's time range.
* Benchmark before the thresholds are fixed (section 4, AC-QA-3): measure first-token time and transcription backlog on your PC with Parakeet running and each built-in model answering over a 30, 60 and 120 minute transcript.

### 3.5 Consent reminder copy

Draft text, no legal claims. Final wording is yours to edit.

Reminder (small card):
* Title: "Tell people you're recording"
* Body: "Rules about recording and transcribing conversations differ by place, and some can apply even when no audio is saved. A good habit: tell everyone, get a clear yes, and follow your organization's policy. This is general guidance, not legal advice."
* Actions: "Got it", checkbox "Never show this again", link "Read more".

Read more page:
* Why it matters, in three short points: laws differ between states and countries; some laws cover learning the contents of a conversation, not only recording it, and courts are still working out how they apply to AI notetakers; people on the call may be in a different place than you.
* What to do: say it out loud at the start, ask for a yes, put it in the invitation, stop if someone objects, follow your workplace rules.
* Copy-ready lines. Spoken: "I'm using a tool to transcribe this meeting so I can take notes. Is that okay with everyone?" Invitation: "I'll be using a local transcription tool during this meeting. Reply if you'd rather I didn't."
* One line on transcript-only: "Not saving audio keeps less on your computer. It does not remove the need to ask."
* Link to an external state-by-state reference. The Reporters Committee for Freedom of the Press guide is a candidate. The link and its wording are verified live during the build, not copied from memory.

### 3.6 Model choice screen
* A static `models.json` lists transcription models (Parakeet v3 int8, Whisper sizes) with size, languages, source URL and SHA-256 when known, and summary models. Source URLs open through the existing external-link allowlist.
* The cloud switch is one stored boolean read by a single helper, `cloud_allowed()`, which every cloud code path calls first. A test enumerates the provider enum and fails for any cloud variant that does not call it.
* Key storage uses the `keyring` crate on Windows (Credential Manager). Migration moves each key, verifies a read back, then nulls the database column.

### 3.7 Sync architecture on the desktop
* New `vault` module: `format` (parse and write, pure, fixture-tested), `export`, `import`, `state` (the `sync_state` table), `scan` (watcher plus periodic rescan), `conflicts`.
* Recommended option (D1): SQLite stays the working store, and the vault is the shared exchange. Phase E exports one way. Phase F imports through the same repository functions the interface uses, with conflict rules from `sync-format.md` section 11.
* Alternative: files as the only store, SQLite as a rebuildable index. Cleaner, but it rewrites most of the persistence layer, the 46 repository tests and the speaker, people and voice code. I do not recommend it for the first version.

### 3.8 Data flow summary
Recording audio goes to the pipeline. Mixed audio goes to the recorder (absent in transcript-only mode). Speech segments go to the transcription engine, then to the live buffer and the Ask box. On stop, the finalizer saves the transcript to SQLite, optionally detects speakers, exports to the vault, then the retention routine deletes audio according to policy. The sweep and the vault scanner run in the background. Nothing leaves the computer except through iCloud's own client (if sync is on) and the cloud providers the user explicitly enabled.

### 3.9 Brand image prompts (for ChatGPT's image generator)

Use these to explore looks. Treat the output as concept art. The production icon is redrawn as a clean SVG from the chosen idea, because generated rasters are rarely clean at 16 px and cannot be recoloured exactly. Do not feed Meetily's logo or any other app's logo to the generator as a reference, since the icon has to be original. Keep the prompt, tool, date and your choice in `brand/ASSETS.md`. Check the generator's current terms before an App Store release. For the wordmark, set the text in Source Serif 4 inside the app, since image generators misspell text.

Common style line to paste at the end of each prompt: "Flat vector style, limited palette of warm cream #FAF6EE, sand #E4DACA, warm charcoal #2B2622 and terracotta #C4623F, no gradients, no shadows, no gloss, no photographic textures, no text, no logos of existing apps."

1. App icon, main concept: "A minimal app icon on a rounded-square cream tile. A thick terracotta ring forming the letter Q with a short diagonal tail at lower right. A second ring in pale sand sits slightly offset behind it, suggesting stacked folded sheets of paper. Centered, generous padding, simple enough to recognise at 16 pixels. Square, 1024 by 1024."
2. App icon, alternative idea: "A minimal app icon on a cream rounded square showing three sheets of paper folded in half and nested together, seen from the edge, in terracotta, sand and charcoal, like the folded sheets of a handmade book. One clear silhouette, no detail finer than a thick line. Square, 1024 by 1024."
3. App icon, alternative idea: "A minimal app icon on a cream rounded square: a single terracotta page with one folded top-right corner and three short charcoal lines of text, slightly rotated, with a soft sand page behind it. One clear silhouette. Square, 1024 by 1024."
4. Dark-mode icon: "The same icon on a warm charcoal #1F1B17 rounded square, with the ring in lighter terracotta #E08A66 and the second ring in dark sand #3A332B. Flat, no glow. Square, 1024 by 1024."
5. Empty state illustration for a screen that says no meetings yet: "A quiet line illustration of an open notebook with a pencil resting on it, drawn with a single terracotta line and a few warm charcoal strokes on transparent or cream background. No people, no faces, calm and uncluttered, lots of empty space. Landscape 1200 by 800."
6. Installer side panel (NSIS, tall): "A tall narrow panel in warm cream with a very subtle paper grain, one large pale sand circle partly off the edge, and a small terracotta folded-corner shape near the bottom. Mostly empty space. Portrait, 328 by 628 pixels, to be scaled down to 164 by 314."
7. Release banner for the repository page: "A wide calm banner on warm cream: on the left a terracotta Q ring mark with a pale sand ring behind it, on the right generous empty space for a title. Landscape 1600 by 500."

## 4. Acceptance criteria

All checks are run in Stage 4 and their output is pasted into the pull request. Commands assume the repository root.

| ID | Check | Method | Pass condition |
|---|---|---|---|
| AC-ID-1 | Rename is one line | Change `name` in `brand.json` to `Marginalia`, run `pnpm brand:apply`, run `pnpm tauri build` | Window title, installer file name, productName and About text show the new name; `git diff --stat` touches only `brand.json` and the generated files |
| AC-ID-2 | No stray old name | `pnpm lint:brand` | Zero matches for `meetily` (case-insensitive) outside `brand/allowed-mentions.txt` |
| AC-ID-3 | Data adoption | Test with a seeded old data folder | New folder holds an identical database; old folder byte-identical; an interrupted copy leaves no half database; legacy temp prefix and recovery store still honoured |
| AC-ID-4 | Credit and licence | Inspect About and `LICENSE` | About shows "Based on Meetily by Zackriya Solutions" and the MIT text; `LICENSE` still contains the upstream notice |
| AC-ID-5 | No Pro | `rg -i "license_key\|licensing\|/get-profile"` in source | Zero matches |
| AC-UI-1 | Contrast | `pnpm lint:contrast` | Every pair in the token tables at or above 4.5 (text), 3.0 (large text, icons, control borders), in both themes |
| AC-UI-2 | No raw colours | `pnpm lint:colors` | Zero raw palette classes or hex colours outside `tokens.css` |
| AC-UI-3 | Visual review | Playwright screenshots of record, meeting, sidebar, people, settings, dialogs in light and dark, at 1280 by 800 | Reviewed against the token tables; no clipped text; no component using the old grey palette |
| AC-UI-4 | Keyboard | Playwright script tabbing through each screen | Every interactive element reachable and shows a visible focus ring |
| AC-UI-5 | Fonts offline | Build with the network disabled | Build succeeds; fonts render from local files |
| AC-UI-6 | Icon | Render the SVG at 16, 32, 64, 256 px | Still recognisable as a Q at 16 px; `.ico` contains 16, 32, 48, 256 |
| AC-RET-1 | Expiry decision | Rust unit tests with an injected clock | For each of the six settings, audio is due exactly at `ended` plus the period, not one millisecond before; Keep is never due |
| AC-RET-2 | Sweep deletes | Test with fixture folders (audio, checkpoints, temp files) | After a sweep all audio artefacts for expired meetings are gone, transcript and metadata files remain, `audio_state` is `deleted` |
| AC-RET-3 | Verified deletion | Test with a locked file (Windows) and a read-only file | State stays `present`, "deletion pending" shown, a later sweep succeeds when the lock clears |
| AC-RET-4 | Off mode safety | Tests: save succeeds, save fails, detection fails | Audio deleted only in the first case; kept and reported in the others |
| AC-RET-5 | Allowlist safety | Tests: symlink and junction inside the folder, folder without markers, path outside the recordings root, file with unknown extension | None of these is deleted |
| AC-RET-6 | Launch and schedule | Test starting the app with overdue meetings; test the interval with a fake timer | Overdue audio is deleted within 30 s of launch; a due item is deleted at the next 60-minute tick |
| AC-RET-7 | Upgrade safety | Migration test on a database from commit `818ef52` | No existing audio is scheduled for deletion; all columns present |
| AC-RET-8 | Wording | Review of settings text | States the three limits in F-RET-10 |
| AC-TO-1 | No audio ever | Test with a synthetic audio source, polling the recording folder every 100 ms during a 2-minute recording and after | Zero files with audio or temp-audio extensions at any sample |
| AC-TO-2 | Disabled features | UI test | Detect speakers, Enhance and playback show disabled with a reason; manual labels work |
| AC-QA-1 | Context builder | Unit tests | Never exceeds the token budget; includes the newest lines; BM25 picks the line containing the asked keyword from a 2-hour transcript; output deterministic |
| AC-QA-2 | Citations | Unit tests | A citation outside every supplied segment is dropped; a valid one links to its line |
| AC-QA-3 | Performance | Benchmark on the target PC, 30, 60 and 120 minute transcripts, each built-in model, Parakeet running | Thresholds recorded in plan.md after the first run, then enforced: no dropped audio chunks, transcription backlog under 5 s, and first token within the agreed limit |
| AC-QA-4 | Local only | Run with the network blocked | A question is answered; zero outbound connection attempts |
| AC-QA-5 | No cloud by default | Test: fresh settings, cloud switch off | No cloud provider selectable; `cloud_allowed()` is false; every cloud code path returns an error before any request |
| AC-CON-1 | Reminder | UI test | Appears on start, does not delay start (under 100 ms), "Never show again" persists across restart and can be reset in Settings |
| AC-CON-2 | Copy | `pnpm lint:copy` and review | No em dash; no sentence saying the user is or is not compliant; the phrase "US compliance required" is absent; guidance page present |
| AC-CON-3 | First-run notice | UI test | Shown once; readable again in Settings |
| AC-MOD-1 | Models screen | UI test and review | Parakeet first; sizes, languages and links shown; a language outside Parakeet's list produces the Whisper recommendation |
| AC-MOD-2 | Keys | Test with seeded plaintext keys | Keys read back from Credential Manager; database columns null; no key in logs |
| AC-MOD-3 | Endpoints | Test | A non-local Ollama or CustomOpenAI host requires confirmation |
| AC-SYNC-1 | Round trip | Export a corpus of at least 50 generated meetings, import into an empty database | Titles, summaries, segments, speakers, people, folders and timestamps equal; second export writes zero files |
| AC-SYNC-2 | Shared fixtures | Run the fixtures in `sync-format.md` section 18 | All pass; unknown keys survive; byte-stable rewrite |
| AC-SYNC-3 | Conflicts | Fixture tests | Winner deterministic; loser in `_conflicts`; iCloud-style duplicate handled; nothing lost |
| AC-SYNC-4 | Deletion | Fixture tests | Delete propagates; edit-after-delete restores; audio delete beats extend; audio file removed from the vault |
| AC-SYNC-5 | Hostile input | Fixture tests and a fuzz run of 10,000 mutated files | No panic; no file deleted or overwritten that could not be parsed; no path outside the vault touched; markdown from the vault cannot run script |
| AC-SYNC-6 | Windows rules | Tests | Reserved names, trailing dots, long and Unicode folder names, and two folders with the same cleaned name all handled; paths under 260 characters |
| AC-SYNC-7 | Real devices | Manual checklist on your PC and iPhone | The four unverified items in `sync-format.md` section 15 are answered and recorded before Phase F starts |
| AC-HK-1 | Telemetry gone | `rg -i posthog` in source and `Cargo.lock` | Zero matches |
| AC-HK-2 | Em dashes | `pnpm lint:dashes` | Zero U+2014 in tracked files outside the allowlist |
| AC-HK-3 | Offline inventory | Network inventory test | Every outbound call site has a tag; test fails on an untagged one |
| AC-HK-4 | Logs | Scripted session with debug logging, then search logs | No transcript text, question text or key |
| AC-PROC-1 | Build and test | `cargo test --lib`, `pnpm exec tsc --noEmit`, `pnpm lint`, UI tests | All green, output pasted |
| AC-PROC-2 | Independent review | A reviewer subagent or fresh session that did not see the build reviews the diff against this spec and plan.md | Findings recorded; every Important finding fixed or accepted by you |

## 5. Data handling

| Data | Where | Who can see it | Retention | Sensitivity |
|---|---|---|---|---|
| Audio | Per-meeting folder under the recordings folder (new installs: app data folder). In the vault `_audio/` only if audio sync is on. | You, anyone with access to your Windows account. With audio sync on, Apple's servers. | By the Retention setting. | High. Can contain other people's voices. |
| Transcripts and summaries | SQLite in app data. In the vault as markdown and JSON if sync is on. | You and anyone with your account. With sync on, Apple's servers. | Until you delete the meeting. | High. |
| People, notes about people | SQLite. In the vault `_people/` if sync is on. | As above. | Until deleted. | Medium. |
| Voiceprints | SQLite only (`meeting_speakers`). Never synced. | You. | Until the meeting or person is deleted. | High. Biometric-like data about other people. |
| API keys | Windows Credential Manager (after F-MOD-6). Today plaintext in SQLite. | You. | Until removed. | High. |
| Model files | App data. | You. | Until removed. | Low. |
| Crash-recovery lines | WebView IndexedDB, until the meeting is saved. | You. | Cleared on save; legacy name honoured. | High while present. |
| Logs | Files under app data. | You. | Rotated. Must contain no content (NF-6). | Medium. |
| Q&A history | Memory only. | You. | Session. "Add to note" writes to the note. | High. |
| Cloud-provider traffic | Only if the cloud switch is on and a provider is chosen. | The provider. | Provider's policy. | High. |

Sensitive items flagged: other people's voices and words (consent, concern 20 and 21), voiceprints (concern 21), plaintext keys today (concern 15), anything in iCloud (concerns 4 and 5), and work meetings on a personal device (concern 19).

## 6. Policies applied

* `intent.md`, accepted 2026-11-01: local first, sync format constraints, retention behaviour, consent wording, unclassified only, no em dashes.
* Repository `CLAUDE.md` (upstream version): Tauri command and event pattern, audio pipeline structure, `perf_debug!` for hot paths, Rust `anyhow::Result`, naming "microphone" and "system", no dependence on the archived backend.
* The AI-native SDLC skill (Anthropic playbook, as installed in this session): artifact chain, solo mode gates, failing test first, independent review, CLAUDE.md verify block.
* WCAG 2.2 level AA for contrast, focus and target size.
* Upstream MIT licence terms (keep notice, credit).
* The previous security review (`docs/SPEAKERS_FOLDERS_PEOPLE.md`): SHA-256 pinning, allowlist-based deletion, URL scheme allowlist, invisible-character stripping, no shell from user strings. These are extended to the vault.
* Apple iCloud documentation and sources checked in this session for iCloud for Windows behaviour and the 30-day recovery window. Re-verified on devices at AC-SYNC-7.

Not applied, because none exist in this session: any organization security, brand or data-handling policy skill, and any review by a policy owner such as your security manager. Nothing here is legal advice.

## 7. Flagged concerns

"[Cannot fully satisfy]" marks a requirement that the physical or legal situation prevents from being met exactly as written. The resolution says what the spec does instead.

| # | Concern | Policy or source | Owner | Resolution |
|---|---|---|---|---|
| 1 | Scope mismatch: intent.md is iPhone-first; the desktop rename, re-skin, consent redesign and model screen are not in it. | intent.md | Ryan | Section 0 addendum, applied only on your yes. |
| 2 | [Cannot fully satisfy] intent says transcript-only "is the same as" retention Off. In code they differ: Off writes audio then deletes it, transcript-only never writes it. | intent.md, Proposed outcome | Ryan | Two options with one-sentence explanation (F-TO-5). Decision D4. |
| 3 | Speaker detection, Enhance, voice memory and playback need audio. Off and transcript-only disable some of them. | Design | Ryan | Off runs detection before deleting. Transcript-only disables them with a stated reason (F-TO-3). Live detection from in-memory audio is a possible later feature. |
| 4 | [Cannot fully satisfy] "Nothing leaves the device by default" and "transcripts and notes sync by default". Synced files reach Apple's servers. | intent.md, Constraints | Ryan | Sync is off until you pick a vault and confirm (F-SYNC-1, F-SYNC-2). After that, transcripts and notes are included by default. This reading of "by default" needs your accept (D3b). |
| 5 | [Cannot fully satisfy] Deleting audio cannot be guaranteed unrecoverable. iCloud keeps deleted files for up to 30 days, offline devices keep copies until they reconnect, backups and SSD wear leveling are outside the app. | Apple docs; intent.md | Ryan | Audio sync off by default. "Deleted" is defined as removed from every place the app can reach, and the UI says so (F-RET-10). |
| 6 | [Cannot fully satisfy] The sweep only runs while the app runs. iOS and Windows give no timer guarantee. | intent.md, Constraints | Ryan | Launch, hourly and on focus; optional daily scheduled task on Windows (F-RET-11); deadlines live in metadata so every device enforces them (sync-format section 10). Stated in the settings text. |
| 7 | The default recordings folder is under Music, which OneDrive known-folder backup may cover, copying audio to the cloud without you choosing it. | Windows behaviour | Ryan | New installs default to the app data folder; warning when a chosen folder is inside a known sync root (F-HK-7). Existing installs keep their folder. |
| 8 | Changing the bundle identifier changes the data folder, so the existing database would not be found. | Tauri | Ryan | Copy-based adoption with confirmation, old folder untouched, legacy names recognised (F-ID-4). |
| 9 | Your suggested muted and accent text colours fail AA for small text (3.64 and 3.77 against 4.5). | WCAG 2.2 | Ryan (taste) | Corrected `muted` and new `accent-strong`; the original accent stays as a decorative colour (section 3.2). |
| 10 | Re-skin is large: 1,283 raw palette class uses in 81 files. | Codebase | Ryan | Tokens, then codemod, then lint, one screen at a time. Phase A is the largest phase. |
| 11 | Fonts load from Google at build time today. | intent.md (local first) | Ryan | Bundle Source Serif 4 and Source Sans 3 locally (F-UI-4). |
| 12 | Name collisions. A task-management product named Quire (quire.io) exists, and several apps named Marginalia exist. No trademark search was run. | intent.md open question 5 | Ryan | Not a blocker for personal use and TestFlight. Decide before an App Store release. The brand config makes switching cheap. |
| 13 | Pro scaffolding remains (licensing table, profile commands with `license_key`, remote profile calls). | "Do not copy Pro" | Ryan | Removed (F-HK-1, AC-ID-5). |
| 14 | Telemetry code remains. The key is empty so nothing is sent, but that is a config fact, not a code fact. | Local first | Ryan | Removed (F-HK-2, AC-HK-1). |
| 15 | API keys are stored in plaintext columns in SQLite. | Security | Ryan | Windows Credential Manager with migration (F-MOD-6). No plaintext fallback. |
| 16 | Updater has upstream's public key with this fork's endpoint, so updates cannot verify. | Security | Ryan | Updater disabled until the fork has its own signing key (F-HK-4). |
| 17 | The Windows ffmpeg bundled in the installer is the "essentials" build from gyan.dev. I believe that build is GPL v3. Sharing the installer with your friend is distribution, which would carry GPL obligations (provide source, licence text). I have not verified this. | Licensing | Ryan | Check with `ffmpeg -version` (look for `--enable-gpl`). If GPL: switch to an LGPL build, or download ffmpeg on first run instead of bundling, or ship the licence and source offer (F-HK-8). Applies to the installer you already built. |
| 18 | The intent's "send the transcript to my desktop" for the phone implies a listening server on the desktop. That conflicts with "no server I run" and with the app's current no-listening-sockets posture. | intent.md | Ryan | Not in this spec. The iOS app can use a local model or a user-supplied Ollama address. Revisit as its own intent. |
| 19 | Work use on a personal device, plus iCloud, may implicate records and information-handling rules. | intent.md open question 6 | Ryan and your security manager | Carried forward. The app shows the unclassified-only notice (F-CON-6) and cannot enforce it. Not a build blocker, a use blocker until answered. |
| 20 | The existing consent notice makes a legal claim ("US compliance required"). | intent.md, Constraints | Ryan | Removed. Guidance only, not legal advice, with a verified external reference (F-CON-5). |
| 21 | Voiceprints are biometric-like data about other people. Some state laws name voiceprints as biometric identifiers (for example Illinois and Texas), mostly for commercial use. Personal use is lower risk. | Privacy law, not legal advice | Ryan | Voiceprints stay local, never sync, and are removed with the meeting or person. Revisit before distributing the app beyond personal use. A "remember voices" switch already exists in the detection dialog. |
| 22 | Em dash rule scope is unclear. The code has 119 in source (comments and some user-visible strings). Upstream documents and LICENSE may have more. | intent.md | Ryan | Decision D5. Proposed: clean all app text and all documents we author, exempt `LICENSE` and third-party notices, lint the rest (F-HK-5). |
| 23 | iCloud for Windows behaviour is only partly documented: placeholder attribute, conflict copies, dot-names, directory rename handling. | Apple docs gaps | Ryan (needs devices) | Phase E (export only) ships first. Phase F waits for AC-SYNC-7. |
| 24 | Running transcription and a language model at once may overheat or slow the PC. Thresholds are an open question. | intent.md open question 3 | Ryan | Benchmark first (AC-QA-3). I need your PC's CPU, GPU and RAM (D9). |
| 25 | Someone could say "ignore your instructions" during a meeting and have it reach the model. | Security | Ryan | The model has no tools or actions; answers are labelled and sanitized (F-QA-7). Residual risk is a wrong answer. |
| 26 | Logs may carry transcript text at debug level. | Privacy | Ryan | Audit and test (NF-6, AC-HK-4). |
| 27 | Cloud model providers and custom endpoints can send transcript text off the computer. | intent.md | Ryan | Master cloud switch, labels, confirmation, non-local host warning (F-MOD-5, F-MOD-7). |
| 28 | Process: the repository CLAUDE.md is upstream's, 409 lines, and describes a legacy backend. | SDLC skill | Ryan | Replaced in Phase A (F-HK-6). Not written yet because Stage 3 has not started. |
| 29 | Model downloads for Whisper and Parakeet are not hash-pinned like the speaker model and ffmpeg. | Security review | Ryan | Manifest with SHA-256 (F-MOD-8, should). |

Decisions needed at the gate

* **D1** Source of truth for sync. Recommended: SQLite stays, vault is the exchange, built in two phases (section 3.7). Alternative: files first.
* **D2** Bundle identifier. Recommended: new identifier plus data adoption (F-ID-4). Alternative: keep `com.meetily.ai` internally and change only visible names.
* **D3** Defaults. (a) New installs record to the app data folder and default retention is 7 days; existing installs keep their folder and Keep. (b) "Sync by default" means "after you enable sync, transcripts and notes are included".
* **D4** Keep retention Off and transcript-only as two options (recommended), or merge them and accept that detection after the fact is lost.
* **D5** Em dash scope: app text and documents we author, with `LICENSE` and third-party notices exempt (recommended), or everything including upstream files.
* **D6** Apply the intent.md addendum in section 0.
* **D7** Default retention for new installs: 7 days (recommended) or Keep.
* **D8** Desktop API transcription: verify and keep behind the cloud switch, or hide it for now (recommended: hide unless it already works).
* **D9** Your Windows PC's CPU, GPU and RAM, so the Q&A benchmark thresholds and the default model can be set.

## 8. Open questions from intent

1. **How the fork stores notes and folders.** Answered in section 1. Reading and writing the shared format is designed in `sync-format.md` and section 3.7. The first-pass decision is D1.
2. **Which on-device LLM.** Desktop: the four built-in models already in the app (Qwen 3.5 2B at 1.2 GB, Qwen 3.5 4B at 2.6 GB, Gemma 3 4B at 2.4 GB, Gemma 3 1B at 1.0 GB, 32k context) plus Ollama. Proposed default: Qwen 3.5 2B, with 4B suggested when the PC has 16 GB or more (D9). Carried to the iOS spec: candidates are Apple's on-device Foundation Models framework (no download, availability to be verified on the target iPhone and OS) versus a small downloaded model. Decide after a test on the device.
3. **Battery and heat.** Carried. Desktop: measured in AC-QA-3. Proposed starting thresholds for the iOS spec, to confirm: a 60-minute live transcription uses no more than 25 percent battery with the screen off, and the system thermal state never reaches serious.
4. **Desktop changes for the shared format and retention.** Answered in section 1 (yes), designed in sections 2.3, 2.8 and 3.
5. **Final name and icon.** Carried. Working name Quire, backup Marginalia. Findings in concern 12. An icon concept exists (section 3.2) and prompts for exploring more are in section 3.9.
6. **Local policy on recording work meetings with a personal device.** Carried unchanged. Owner: you, with your chain of command or security manager. It gates work use of the app, not the build.

## 9. Suggested delivery phases (input to plan.md, after you accept)

Each phase is independently shippable, ends in its own tests and an independent review, and is a separate gate.

* **A. Identity, tokens, housekeeping.** Brand config and generator, data adoption, tokens, fonts, codemod and lint, icon, About, removal of Pro, telemetry and dead files, updater off, em dash lint, one-page CLAUDE.md with verify block and a hook that locks tests during bug-fix tasks. Largest phase because of the re-skin.
* **B. Privacy core.** Retention setting, sweep, finalizer, transcript-only, consent reminder, first-run notice. Failing tests first for every deletion rule.
* **C. Models.** Models screen, cloud switch, Credential Manager migration, model manifest.
* **D. Live Q&A.** Benchmark, context builder, Ask box, citations.
* **E. Vault export.** Format library with fixtures, one-way export, Sync screen, disclosure.
* **F. Two-way sync.** Import, conflicts, deletion propagation, real-device verification first.

Rule for bugs found during any phase: reproduce as a failing test, commit it, then fix without editing that test.
