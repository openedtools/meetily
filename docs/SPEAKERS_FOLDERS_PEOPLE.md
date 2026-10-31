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
