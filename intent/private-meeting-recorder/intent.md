# Intent: Private on-device meeting recorder for iPhone (working name TBD: Quire or Marginalia)
Author: Ryan (Captain, USAF; IITC instructor and builder). Status: accepted by Ryan on 2026-11-01. Date: 2026-11-01

## Problem
I use a self-built fork of Meetily on my desktop (folders, speaker recognition, local Parakeet transcription). I cannot record or review meetings and calls from my iPhone with the same privacy. Granola's free tier is limited, and its transcription runs in its cloud. I want a native iPhone app that does the same job with nothing leaving the device by default.

## Proposed outcome
A native iPhone app, built in Swift and distributed through TestFlight first, with an optional App Store release later. From the user's side:

* Record a meeting and see a live transcript during it.
* Ask questions mid-meeting ("what did he just say about timelines?") and get answers from the transcript so far.
* Get a summary after the meeting.
* Organize notes into folders that sync with the desktop app through iCloud Drive.
* Control audio retention: keep or discard audio, with auto-delete after a user-chosen period (Off/instant, 24 hours, 7 days, 14 days, 30 days, Keep). Applies to the desktop, Windows, and iOS versions.
* Offer a transcript-only mode: live transcription from the microphone and system audio with no audio file saved. It is the same as setting audio retention to Off, presented as its own clear option. It reduces what is stored. It is not a legal shield (see Constraints).
* Show a short, non-intrusive reminder to tell participants the tool is in use. It can be dismissed, and offers "Never show this again." The reminder also links to plain-language consent guidance and suggests a verbal heads-up or invite language.
* Desktop parity: the desktop app (Windows first, Mac later) gets the same live Q&A against the transcript so far, the same transcript-only mode, and the same retention setting.
* Use a cream and terracotta paper look: minimal, quick, calm.

Model choices (user-selectable, same spirit as desktop)

* Transcription: on-device by default. The user downloads a model from a linked list: Parakeet (via FluidAudio) first, Whisper-family models (open source) as the alternative for languages Parakeet does not cover. Optional API transcription, clearly labeled as leaving the device.
* Summaries and live Q&A: on-device model by default. Alternatives: send the transcript to my desktop (for example a local Ollama model), or a user-supplied API key. Cloud options are off until the user turns them on.
* Wispr Flow is proprietary and cloud-based, so it is not a candidate engine. Whisper models and Parakeet are.

## Affected users and systems

* Me first. Target devices: iPhone 16 Pro Max on iOS 27 (per my recollection of the model), and a Windows desktop as the primary desktop. A Mac desktop build comes later. A Mac with Xcode is used to build the iOS app. Possibly other people later if released on the App Store.
* Desktop Meetily fork and its Windows build: must read and write the same note and folder format.
* Upstream: Meetily is MIT. Keep its copyright and license notice, credit it in About, and do not copy its paid Pro features.
* Dependencies: FluidAudio (Parakeet, diarization, VAD), a Whisper-family library, iCloud Drive.

## Constraints

* Local first: no audio or transcript leaves the device unless the user enables a cloud option.
* Sync format: one markdown file per note plus JSON metadata, folders as real directories, unique IDs to avoid conflicts. Transcripts and notes sync by default. Audio sync is optional.
* Retention cleanup must run at app launch and in a background task, because iOS does not guarantee timers. Deleting audio must propagate to any synced copy.
* Background audio recording capability is required so recording continues when the screen locks.
* Phone call audio cannot be captured by third party apps on iOS. Plan on in-person meetings, speakerphone, and imported recordings.
* Work use: this will touch work meetings. Unclassified information only. The app must never be used for classified or CUI discussions. Whether a personal phone may be used to record work meetings is a local policy question I must confirm with my chain of command or security manager before relying on it.
* The consent reminder is a courtesy prompt, not legal compliance. Recording consent rules vary by state and country, and the app should say so in plain words.
* Transcript-only does not remove consent obligations. Wiretap statutes such as California's can reach learning the contents of a communication, not just recording it, and the Otter.ai class actions testing AI notetakers are unresolved. Texas is a one-party state, but all-party states exist and out-of-state participants can matter. Safest practice: disclose, get a yes from participants, and follow local policy. This is not legal advice.
* No em dashes in app copy or documentation.

## Out of scope (v1)

* Android.
* Capturing phone call audio directly.
* Team accounts, a cloud backend, or any server I run.
* Calendar integrations and CRM integrations.
* App Store submission (TestFlight only for v1).

## Open questions

1. How does the desktop fork store notes and folders today (database or files), and how will it read and write the shared iCloud format? The spec needs the fork's actual code to answer this.
2. Which on-device LLM for summaries and live Q&A, and how large a download is acceptable?
3. Battery and heat during hour-long live transcription: acceptable thresholds?
4. Does the desktop fork need changes so it reads the shared note format, and does the Windows build need the same retention setting?
5. Final name and icon, after an App Store and trademark availability check.
6. Local policy answer on recording work meetings with a personal device.
