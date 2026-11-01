# Shared vault format (version 1)

Status: draft for the Stage 2 gate. Part of the spec for `intent/private-meeting-recorder/`. Date: 2026-11-01.
Working app name: Quire. The identifiers `quire-vault`, `quire-format.json` and the `quire` key are fixed format identifiers, not brand values. They stay the same even if the app is renamed, because changing them would break every existing vault and the iPhone app. Only the vault root folder name (`Quire` in the examples) comes from the brand config.

This file is the contract between the desktop app and the future iPhone app. Both implement it independently, so it is written as a standalone document. Words in capitals follow the usual meaning: MUST, SHOULD, MAY.

## 1. Goals and non-goals

Goals
* Notes, transcripts, people and folders live as plain files that any device can read, with nothing but iCloud Drive between them.
* A human can open the vault in a file browser or text editor and understand it.
* Two devices editing at once never silently lose data.
* Deleting audio is a defined, testable operation, and the format says what can and cannot be promised.
* The format can grow without breaking older apps.

Non-goals (version 1)
* Real-time collaboration, merging text edits inside a note, or sharing a vault between different people.
* Syncing voiceprints, model files, API keys or app settings (see section 14).
* Nested folders (the desktop has flat folders today; the format reserves room for nesting, see section 9).

## 2. Vault layout

The vault is one directory chosen by the user. On Windows with iCloud for Windows it is a folder inside the iCloud Drive folder, for example `C:\Users\<name>\iCloudDrive\Quire`. On iPhone it is the matching folder in iCloud Drive.

```
Quire/                                  the vault root
  quire-format.json                     vault marker and version (section 5)
  01JC8ZQ4M3V7K2H9T5R6XWPD1N.md         an unfiled note
  01JC8ZQ4M3V7K2H9T5R6XWPD1N.transcript.json
  Work Meetings/                        a folder is a real directory
    _folder.json                        folder identity (section 9)
    01JC90A2T8N4Y6B3D7F1GHKQ5S.md
    01JC90A2T8N4Y6B3D7F1GHKQ5S.transcript.json
  Research/
    _folder.json
    ...
  _people/                              one file per person (section 8)
    01JC7M9X2R5T8V1B4D6F3HJKQ0.md
  _audio/                               audio, only when audio sync is on (section 10)
    01JC90A2T8N4Y6B3D7F1GHKQ5S.m4a
  _trash/                               deleted notes waiting to be purged (section 12)
  _conflicts/                           losing sides of conflicts (section 11)
  _tmp/                                 write staging, always safe to delete
```

Rules
1. Names that start with an underscore are reserved for the format. A user folder name MUST NOT start with an underscore (the app strips it).
2. Names never start with a dot. Dot-prefixed names may be skipped by iCloud Drive (to verify, section 15), so the format does not use them.
3. A note belongs to the folder whose directory contains its file. Notes at the vault root are unfiled. Moving a note between folders is a file move and does not change its file name.
4. A reader finds notes by scanning for `*.md` files outside reserved directories and reading the `id` in the front matter. The `id` is the identity. The path is only a location.
5. The desktop MUST ignore everything it does not recognise (other files, other directories) and MUST NOT delete or move anything it did not create.

## 3. Identifiers and file names

* Every note, person, folder and segment has an immutable ID that is generated once.
* Note, person and folder IDs are ULIDs: 26 characters of uppercase Crockford base32 (`0-9A-HJKMNP-TV-Z`), the first 10 encoding the creation time in milliseconds, so files sort by creation time. Regex: `^[0-9A-HJKMNP-TV-Z]{26}$`.
* Note files are named `<id>.md` and `<id>.transcript.json`. The name never changes, even when the title changes, so renames cannot cause conflicts.
* Folder directories are named after the folder's display name (section 9). Person files are named `<id>.md`.
* A device ID is `dev-` plus 8 random lowercase hex characters, generated once per install and stored locally. It is written to `updated_by` fields.
* All timestamps are UTC, ISO 8601, with milliseconds and a trailing `Z`: `2026-11-01T15:42:10.123Z`.
* All text files are UTF-8 without a byte order mark, with LF line endings, in Unicode NFC.
* File names are ASCII only except folder directory names (section 9). Because of this, case-insensitive file systems (Windows, macOS and iOS defaults) cannot make two IDs collide.

## 4. Encoding rules for front matter

Front matter is a restricted subset of YAML so that a Swift parser and a Rust parser read it the same way.

* Delimited by a first line of `---` and a closing line of `---`.
* Keys are plain lowercase words with underscores.
* Every string value is written in double quotes with JSON escaping. This avoids YAML surprises such as `no` becoming false or `1.10` becoming a number.
* Numbers, `true`, `false` and `null` are unquoted.
* Lists and maps use block style (no anchors, no tags, no multi-document files, no flow style except `[]` and `{}` for empty values).
* Writers MUST emit this subset. Readers SHOULD use a real YAML parser but MUST reject anchors, aliases and custom tags (a hostile file must not be able to expand into something huge).
* Unknown keys MUST be preserved on rewrite, in their original order (section 6).

Size limits, enforced by readers: note file 2 MB, transcript sidecar 20 MB, person file 256 KB, folder file 16 KB. A larger file is skipped and reported, never partially read.

## 5. `quire-format.json`

```json
{
  "format": "quire-vault",
  "version": 1,
  "min_reader": 1,
  "vault_id": "01JC8Z0000VAULT00000000000",
  "created": "2026-11-01T14:00:00.000Z",
  "created_by": "dev-7f3a91c2"
}
```

* `format` MUST equal `quire-vault`. A directory without this file is not a vault, and the app offers to create one only when the user asks.
* `version` is the format major version of the vault. `min_reader` is the lowest app format level that may open it for writing.
* `vault_id` identifies the vault so a device can detect that the user pointed it at a different vault.
* This file is written once. Only a version migration (section 13) rewrites it.

## 6. Note file

A note is a markdown file with front matter and a free markdown body.

```markdown
---
quire: 1
id: "01JC90A2T8N4Y6B3D7F1GHKQ5S"
title: "Weekly sync with Alex"
created: "2026-11-01T14:30:02.000Z"
updated: "2026-11-01T15:42:10.123Z"
updated_by: "dev-7f3a91c2"
rev: 3
started: "2026-11-01T14:30:02.000Z"
ended: "2026-11-01T15:22:11.000Z"
duration_s: 3129
language: "en"
source: "recorded"
mode: "audio"
speakers:
  - slot: 1
    name: "Alex Rivera"
    person: "01JC7M9X2R5T8V1B4D6F3HJKQ0"
  - slot: 2
    name: "Speaker 2"
    person: null
transcript:
  file: "01JC90A2T8N4Y6B3D7F1GHKQ5S.transcript.json"
  sha256: "9f2c...64 hex characters..."
  segments: 412
summary:
  status: "done"
  generated_at: "2026-11-01T15:25:40.000Z"
  model: "qwen3.5:4b"
audio:
  retention: "d7"
  expires: "2026-11-08T15:22:11.000Z"
  state: "present"
  file: null
---

## Summary

Decisions and action items, as markdown.

## Notes

Anything typed by hand, on any device.
```

Field reference

| Field | Type | Meaning |
|---|---|---|
| `quire` | integer | Format level of this file. Always 1 in version 1. |
| `id` | string | Immutable ULID. MUST equal the file name without extension. A mismatch makes the file invalid. |
| `title` | string | Display title. Can change without renaming the file. |
| `created` | timestamp | When the note was created. Never changes. |
| `updated` | timestamp | When any device last changed the file. Used by conflict handling. |
| `updated_by` | string | Device ID of the last writer. |
| `rev` | integer | Starts at 1, increases by 1 on every write by any device. Informational and a tie-breaker. |
| `started`, `ended`, `duration_s` | timestamp, timestamp, number | Recording times. `null` for imported or typed notes. |
| `language` | string or null | BCP 47 code of the transcript language. |
| `source` | `recorded` or `imported` or `typed` | Where the content came from. |
| `mode` | `audio` or `transcript_only` | Whether audio was ever written (see the spec, section on transcript-only). |
| `speakers` | list | One entry per speaker slot. `name` is a snapshot of the display name, `person` links to a `_people` file or is null. |
| `transcript` | map or null | Sidecar reference. `sha256` is the hash of the sidecar bytes and `segments` the segment count, so a reader can tell the sidecar has fully arrived. |
| `summary` | map or null | Status is `none`, `pending`, `done` or `failed`. `model` records the model name only. |
| `audio` | map | Retention state, section 10. |

Body rules
* The body is free markdown. The app writes a `## Summary` heading for generated text. Any other heading and any text the user types is user content.
* Writers MUST preserve body text they did not change. Round trips MUST be byte-stable when nothing changed.
* The body is untrusted input. Apps render it through their sanitizing markdown path and never execute it (section 14).

Preserving unknown content: a reader that finds front matter keys it does not know MUST write them back unchanged, in place. This is what lets a newer iPhone app add a field without the older desktop app deleting it.

## 7. Transcript sidecar

`<id>.transcript.json` holds the segments. It is written when a recording is finalized and changes rarely (a corrected line, a moved line). Speaker renames do not touch it, because segments refer to speaker slots, not names.

```json
{
  "quire": 1,
  "note_id": "01JC90A2T8N4Y6B3D7F1GHKQ5S",
  "segments": [
    { "id": "seg-8d1e", "start": 12.4, "end": 15.9, "speaker": 1, "text": "Let's start with timelines.", "confidence": 0.93 },
    { "id": "seg-8d1f", "start": 16.2, "end": 21.0, "speaker": 2, "text": "Two weeks, maybe three.", "confidence": 0.9 }
  ]
}
```

* `start` and `end` are seconds from the start of the recording. `speaker` is a slot number from the note's `speakers` list, or `null`.
* `id` is unique within the note. Existing desktop segment IDs are kept as they are.
* Segments are sorted by `start`. Unknown keys on segments and on the root are preserved.
* The sidecar and the note arrive separately over iCloud. A note is complete only when the sidecar exists and its SHA-256 equals `transcript.sha256`. Until then readers show the note with a "transcript still syncing" state and retry later.

## 8. People

`_people/<id>.md` is one markdown file per person.

```markdown
---
quire: 1
id: "01JC7M9X2R5T8V1B4D6F3HJKQ0"
name: "Alex Rivera"
aliases:
  - "Alex R"
created: "2026-10-27T18:00:00.000Z"
updated: "2026-10-31T09:12:00.000Z"
updated_by: "dev-7f3a91c2"
rev: 2
merged_into: null
---

Free markdown notes about this person.
```

* Merging person B into person A: the app rewrites references in notes from B to A, writes `merged_into: "<A id>"` into B's file, and keeps B's file for 30 days so a device with stale references can resolve them. After that any device may move B's file to `_trash`.
* A reader that meets a `person` ID with no file shows the stored `name` snapshot and does not fail.
* Voiceprints are never part of a person file (section 14).

## 9. Folders

A folder is a real directory under the vault root. Inside it, `_folder.json` carries the identity:

```json
{ "quire": 1, "id": "01JC5K3N7P9R2T4V6X8Z0BDFGH", "name": "Work Meetings",
  "created": "2026-10-28T12:00:00.000Z", "updated": "2026-10-28T12:00:00.000Z", "updated_by": "dev-7f3a91c2" }
```

Directory naming
* The directory name is the display `name` after cleaning: NFC form, characters `< > : " / \ | ? *` and control characters removed, trailing dots and spaces removed, a leading underscore removed, names that Windows reserves (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, with or without an extension) get a trailing ` folder`, at most 80 characters.
* `_folder.json` keeps the real, uncleaned `name`. The directory name is only for humans browsing files.
* If a cleaned name collides with a directory that has a different folder ID, the newer folder gets ` (2)`, ` (3)` and so on.
* Folder names are unique ignoring case on the desktop today. Readers MUST NOT assume that: two folders with the same name from two devices are kept apart by ID and by the suffix rule.

Renaming and deleting a folder
* Rename: rename the directory and update `_folder.json` (`name`, `updated`). A note's file does not change.
* Delete: only an empty directory is removed. Notes inside are moved to the vault root first (the desktop rule is that deleting a folder never deletes its notes).
* If a device sees a directory with no `_folder.json`, it treats it as a folder named after the directory and writes a new `_folder.json` with a fresh ID.

Nesting: the format allows folders to be created inside folders up to 3 levels. Version 1 writers create depth 1 only. Version 1 readers MUST list nested folders they find and MUST NOT flatten or move them.

## 10. Audio and retention metadata

Audio sync is off by default. When it is off, audio files stay in each device's own storage and never enter the vault. The `audio` map in the note still describes the retention state, so every device enforces the same deadline on whatever copy it has.

```yaml
audio:
  retention: "d7"                       # off | h24 | d7 | d14 | d30 | keep
  expires: "2026-11-08T15:22:11.000Z"   # null when retention is keep
  state: "present"                      # none | present | deleted
  deleted_at: null                      # set when state becomes deleted
  file: null                            # vault path such as "_audio/<id>.m4a" when audio sync is on
```

* `retention` is the policy chosen at recording time. `expires` is `ended` plus the period, in UTC. The clock starts when the recording ends.
* `state: none` means audio was never written (transcript-only mode). `present` means a copy may exist on some device. `deleted` means the deadline passed or the user deleted the audio.
* With audio sync on, the audio file lives at `_audio/<id>.<ext>` and `file` holds that relative path. A reader MUST reject any `file` value that is absolute, contains `..`, or points outside `_audio/`.
* Extending: a user can change a note's retention to a later date, or to `keep`, before `expires`. After `state` is `deleted`, the audio is gone and extending changes nothing.
* Each device deletes its own copy when `now >= expires` or `state` is `deleted`, at launch, on a schedule while running, and when it receives a changed note. A device does not need a message from another device, because the deadline is in the metadata.
* Deletion wins over extension: if one device extends while another device has already deleted, the result is `deleted`. This is the one place the format prefers losing data over keeping it, because the user asked for deletion.
* A device that is off or unused cannot delete anything. Its copy stays until the app next runs. The app says so in its settings text.

What deletion does and does not promise
* The app removes the file from the vault folder and from its own storage and verifies that it is gone.
* iCloud keeps deleted files in a recovery area for up to 30 days, and the user can restore them. A device that is offline keeps its copy until it reconnects. Backups, snapshots and SSD wear leveling are outside the app's control.
* So "deleted" in the format means "removed from every place this app can reach", not "unrecoverable". The user interface says that in plain words wherever audio sync or retention is described. This is the main reason audio sync defaults to off.

## 11. Conflicts

Terms: a device keeps, for each file it has synced, the SHA-256 and `rev` it last saw (the base). A conflict exists when both the local copy and the vault copy differ from the base.

Rules
1. If only the vault copy changed, import it. If only the local copy changed, export it.
2. If both changed, pick a winner by `updated` (later wins). If equal, the higher `rev` wins. If still equal, the lexicographically larger `updated_by` wins. The result is deterministic, so every device picks the same winner.
3. The loser is written in full to `_conflicts/<id>.<updated_by>.<updated compact>.md` (and a `.transcript.json` beside it if that file also differed). Nothing is lost silently. The app shows a "Conflicts" count in Settings and a banner, and the user can open the file or restore it as a new note.
4. The winner gets `rev` set to the larger rev plus 1.
5. iCloud may create its own copies when two devices write at once, with names such as `<id> 2.md`. A reader that finds a second file with an existing `id` treats it as a conflict participant, applies rule 2, and moves the loser into `_conflicts`. It never deletes it.
6. Clock skew can make the wrong side win. Because the loser is always preserved, that costs a click, not data.
7. Within a note, there is no field-level merge in version 1. The whole file is the unit.

## 12. Deleting notes, folders and people

Notes
* Delete means: move `<id>.md` and `<id>.transcript.json` into `_trash/<id>/`, and write `_trash/<id>/_deleted.json` with `{ "id", "deleted_at", "deleted_by", "folder_at_deletion" }`.
* Every device that sees an ID under `_trash` removes that note from its working database and its local audio copy.
* Edit after delete: if a device edits a note and its `updated` is later than the tombstone's `deleted_at`, the note is restored from `_trash` and a conflict record is written. Notes prefer keeping data.
* Purge: after 30 days any device may remove `_trash/<id>/`. The audio file in `_audio` is removed at delete time, not at purge time.

Folders and people
* Folder deletion is covered in section 9. Person deletion uses a tombstone file `_trash/<id>/_deleted.json` as for notes, and notes that referenced the person keep the `name` snapshot with `person: null`.

Audio
* See section 10. Audio deletion is a metadata change plus file removal, with deletion winning over extension.

## 13. Versioning and compatibility

* The vault `version` is a single integer. Version 1 is this document.
* Additive changes (new optional keys, new optional files) do not change the version. Old apps ignore and preserve them.
* A change that old apps cannot safely write alongside raises `version` and `min_reader`. An app with a format level below `min_reader` opens the vault read-only and shows "Update the app to edit this vault". It MUST NOT write.
* An app with a format level above the vault `version` may offer an upgrade. Upgrading is one way, takes a full copy of the vault into `_conflicts/backup-<date>/` first (or refuses if there is no room), and rewrites `quire-format.json` last.
* Every note and person file carries `quire: <level>` so a single stray newer file can be identified.

## 14. Trust, privacy and what never syncs

The vault is untrusted input. Another device, a sync glitch or a malicious file dropped into the folder can all put bytes there.

Readers MUST
* Validate IDs against the ULID regex and require the ID to match the file name.
* Reject paths that are absolute, contain `..`, contain a drive letter, or resolve (after following no links) outside the vault. Readers do not follow symbolic links or Windows reparse points other than the cloud-placeholder kind.
* Enforce the size limits in section 4 and reject YAML anchors, aliases and tags.
* Render markdown through the app's existing sanitizing renderer. Raw HTML, scripts and `javascript:` links are never executed.
* Treat unparseable or half-written files as "not ready, try again later". Never delete, overwrite or "repair" a file the reader could not parse.

Never synced
* Voiceprints and speaker embeddings. They are biometric-like, and different apps use different embedding models, so they are not comparable across devices anyway. They stay in each app's local storage.
* API keys, model files, settings, the retention sweep log, crash-recovery data.
* Audio, unless the user turns audio sync on.

Data that does sync lands on Apple's servers once iCloud uploads it. Without Advanced Data Protection, Apple holds the encryption keys; with it, iCloud Drive is end-to-end encrypted. The app shows this on first enabling sync and does not decide for the user.

## 15. Windows and iCloud constraints

Known from Apple documentation and sources checked during design (to be re-verified on the real devices in Stage 4)
* iCloud for Windows shows iCloud Drive as a folder. Files can be kept on the device ("Always keep on this device") or exist as online-only placeholders that download when opened. A file must be downloaded before another program can read it.
* Files deleted from iCloud Drive stay recoverable for up to 30 days.
* Apple does not publish how iCloud Drive resolves two simultaneous edits of the same file. The format therefore assumes conflicting copies can appear (section 11, rule 5) and that delivery is eventual and unordered across files.
* Windows file names are case-insensitive, cannot use `< > : " / \ | ? *`, reserve names such as `CON` and `NUL`, cannot end in a dot or space, and paths are limited to 260 characters unless long paths are enabled.

Design consequences
* Stable ASCII file names built from IDs (section 3), with titles kept in front matter.
* The desktop keeps the vault path short: it warns when the vault root path is longer than 150 characters. The longest relative path the format produces is under 140 characters.
* Writes are atomic: write the new bytes to `_tmp/<name>.<random>.part` on the same volume, flush, then rename over the target. A reader that sees a `.part` file or any file in `_tmp` ignores it.
* Write order for a new or changed recording: sidecar first, then the note. The note's `transcript.sha256` is the commit point.
* Hydration: before reading a file, the desktop detects an online-only placeholder (the Windows recall-on-data-access attribute), asks for it to download by opening it with a timeout, and shows "waiting for iCloud" instead of treating the file as missing. The setting text recommends "Always keep on this device" for the vault folder.
* Change detection uses a file watcher plus a full rescan at launch, on window focus, and every 5 minutes while running. Watchers are unreliable on cloud folders, so the rescan is the source of truth.
* An iOS app container only appears in iCloud for Windows if the iOS app declares it as a public document container. This is an iOS-side decision (section 17). The format does not depend on it.

Still unverified (must be tested on the real PC and iPhone before the desktop ships sync)
* Whether dot-prefixed names are skipped (the format avoids them anyway).
* The placeholder attribute iCloud for Windows actually sets, and download latency for a 5 MB sidecar.
* What iCloud for Windows does with two simultaneous writes, and how it names conflict copies.
* Whether renaming a directory with 200 files preserves history cleanly or is delivered as delete plus add.

## 16. Migrating the desktop's existing notes

The desktop today stores everything in SQLite (see the spec, section 1). Migration is one way into the vault, safe to repeat, and never deletes or changes existing rows.

1. The user turns on sync and picks a folder. The app writes `quire-format.json` only after the user confirms.
2. A new nullable column `sync_id` (ULID) is added to meetings, people and folders. It is filled the first time a row is exported, using the row's `created_at` for the time part, and never changes after that. The old `meeting-<uuid>` ID is kept as is and written into the note as an unknown-to-iOS key `legacy_id` for traceability.
3. For each meeting: write the sidecar, then the note (front matter from the meetings, speakers and summary tables, body from the summary markdown), and record `(sync_id, sha256, rev, path)` in a local `sync_state` table.
4. People become `_people` files, folders become directories with `_folder.json`.
5. Verification: after the export the app re-reads the vault into memory and compares counts and content hashes with the database. A mismatch is reported and nothing is marked as synced.
6. Re-running the export finds every row already in `sync_state` and only writes what changed.
7. Importing (phase two) goes through the same repository functions the user interface uses, so rules such as name cleaning, invisible-character stripping and unique-name checks apply to anything arriving from the vault.

Where the truth lives: in version 1 the desktop keeps SQLite as its working store, and the vault is the shared exchange. SQLite rows are authoritative on the desktop until a vault change is imported through section 11. The alternative, making the files the only store and SQLite a rebuildable index, is cleaner but rewrites most of the persistence layer. The spec recommends the first option, in phases (export only, then two-way), and asks for a decision at the gate.

## 17. What the iOS app needs to do (informative)

This section is not a requirement on the desktop. It lists what the separate iOS project must decide.
* Reach the vault. Either declare a public iCloud Drive container (it appears in the Files app and, to be verified, in iCloud for Windows) or let the user pick the folder once with the document picker and keep a security-scoped bookmark. The second does not depend on container visibility.
* Coordinate file access with `NSFileCoordinator`, ask for downloads of online-only files, and observe changes with `NSMetadataQuery`.
* Implement sections 3 to 14 exactly, including preserving unknown keys and the deterministic conflict winner.
* Run the retention sweep at launch and in a background task, because iOS does not promise timers (from intent.md).

## 18. Conformance fixtures

A shared set of fixture vaults will live in the desktop repository under `docs/vault-fixtures/` and be copied into the iOS project. Each fixture is a folder tree plus an `expected.json`. Both apps must produce the same results.
* Round trip: a vault with every field type reads and rewrites byte-identical.
* Unknown keys survive a rewrite.
* Conflict: both sides changed, the winner is deterministic, the loser lands in `_conflicts`.
* iCloud-style duplicate (`<id> 2.md`) is resolved without data loss.
* Delete, edit-after-delete restore, audio delete wins over extend.
* Hostile files: path traversal in `audio.file`, YAML alias bomb, oversized file, ID that does not match the file name, truncated sidecar, invisible and bidirectional characters in names.
* Version gating: `min_reader` above the app's level opens read-only.
* Windows names: reserved names, trailing dots, long names, Unicode folder names, two folders whose cleaned names collide.
