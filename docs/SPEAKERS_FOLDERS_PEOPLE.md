# Speakers, People and Folders (fork additions)

Features added in this fork on top of Meetily v0.4.1.

## What you get
- **Speaker labels** on every transcript line. Click a label to name the speaker (all of their lines
  update), or move a single line to someone else.
- **Automatic speaker detection** (Meeting page → *Speakers* → *Detect speakers…*). Runs on this computer.
  Choose how many people spoke when you know it; that is more reliable than the estimate.
- **People** (sidebar). Everyone you have named, their meetings, notes, and a search of what they said.
  Typing a name that is close to an existing one ("Major Smith" vs "Maj Smith") offers the existing person.
- **Voice memory.** Detected speakers keep an average voice. In a later meeting, unnamed speakers whose voice
  matches someone you named are *suggested* (never assigned automatically).
- **Folders** for meetings (sidebar), and sidebar search by a speaker's name.
- Transcript search no longer crashes on, or misses, non-English text (Cyrillic, accents).

## Where things are stored (all local)
- Everything lives in the app's SQLite database (`meeting_minutes.sqlite` in `%APPDATA%\com.meetily.ai`).
- **Voice memory** is a 256-number average per detected speaker (`meeting_speakers.voiceprint`). It cannot be
  turned back into audio. Erase it for one person on the People page (*Forget voice*), or leave
  *Remember these voices* unticked when detecting. Deleting a meeting or a person removes their voices.
- The voice model (`wespeaker_en_voxceleb_resnet34.onnx`, ~26 MB) is downloaded once from GitHub into
  `%APPDATA%\com.meetily.ai\models\speaker\` and checked against a pinned SHA-256. If your network blocks the
  download, copy the file there yourself. Audio is never uploaded.

## Honest limits
- A transcript line gets one speaker. Two people talking inside one line are not separated.
- Very similar voices can be merged into one speaker; re-run with the number of people set.
- Name suggestions are guesses ("likely" / "possible"). Accuracy was measured on public recordings and
  synthetic voices, not on your meetings.
- Re-transcribing a meeting clears its line-to-speaker assignments (names and people are kept).

## Checking accuracy on your own recordings
`cargo test --lib speaker_accuracy -- --ignored --nocapture` and `... voice_memory ...` run the real pipeline on a
folder of recordings with known speakers. See `frontend/src-tauri/src/diarization/accuracy_check.rs`.

## Building a shareable Windows installer
Run `frontend\build_installer_windows.bat` (needs Node, pnpm, Rust, the Visual Studio C++ Build Tools and CMake).
It applies the project's portable-build settings so the installer runs on other computers; the target PC needs an
AVX2-capable x64 CPU (virtually anything from the last ~10 years).
It builds a single NSIS `.exe` installer and prints where it is. The build is CPU-only and not code-signed, so
Windows SmartScreen will say "Windows protected your PC" on first run: choose *More info* then *Run anyway*.

This fork's `tauri.conf.json` differs from upstream in two ways: update checks look at
`github.com/openedtools/meetily` instead of the original project (so the app is never offered the stock build
as an "update"), and updater artifacts are not generated (that needs the original project's signing key).
Installing replaces a stock Meetily install (same app id) and keeps its data. When uninstalling the old version,
leave "delete application data" **unticked**.

## Security review (before sharing the installer)
Two independent reviewers (app attack surface; supply chain/installer) plus adversarial tests of the new code.

**Fixed in this fork**
- `open_external_url` ran `cmd /C start <url>`, so a link like `https://x.com/&calc.exe` could run a command. Now only
  http/https/mailto links are opened, without a shell.
- Deleting a meeting left its audio and full transcript on disk. It now removes the app's own files from the
  recording folder (only files the app creates, only from a folder carrying its marker files) and the database
  uses `secure_delete`, so deleted text is also erased from the database file.
- The Windows `ffmpeg.exe` bundled into the installer was downloaded with no integrity check. It is now pinned by
  SHA-256 (archive and binary); a cached copy is hash-checked before it is ever run. The silent runtime download of
  an unverified FFmpeg was removed.
- The speaker model is hash-checked every time it is loaded (not only when downloaded) and the download is size-capped.
- Telemetry can no longer reach the original project's PostHog account (its key was hard-coded). Update checks no
  longer run at every launch. Transcript text is no longer written to the INFO log.
- Names and notes strip invisible/control/text-direction characters; stale detection results cannot wipe a
  meeting's speakers; corrupt audio/embeddings cannot poison clustering; folder opening only opens folders.

**Tested and not a problem**
- No script-injection path found: strict CSP, no raw-HTML rendering, all SQL parameterised. A browser attack test
  (HTML/script in names, notes, folder names, search) executed nothing.
- A crafted file disguised as `.mkv` does not make ffmpeg contact a server (tested).

**Known and accepted (decide before wider use)**
- API keys for cloud summaries are stored in plaintext in the local database. Use a local model (Ollama) or accept this.
- Nothing is encrypted at rest (audio, transcripts, voiceprints). Turn on BitLocker/device encryption.
- The webview has broader file permissions than it needs (`fs:*`); only matters if script could run, which was not found.
- Whisper, Parakeet and built-in summary models are downloaded without a hash check (voice model and ONNX Runtime are pinned).
- The updater still carries the original project's public key; this fork has no update feed, so friends reinstall manually.
- Dependency advisories exist (mostly build-time or server-side code that is not shipped). Notable: the `prosemirror-view`
  override should be raised to 1.42.3 or later. Fixing it needs a re-test, so it was not changed blind.
- The installer is unsigned: Windows SmartScreen will warn. Send the SHA-256 printed by the build script separately.
