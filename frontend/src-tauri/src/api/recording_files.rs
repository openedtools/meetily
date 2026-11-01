//! Removing a meeting's recording from disk when the meeting is deleted.
//!
//! Deleting a meeting used to remove only its database rows, leaving the audio and the full
//! transcript behind in the recordings folder. This removes them too, but deliberately narrowly:
//! only files the app itself creates, and only from a folder that carries the app's own marker
//! files, so a wrong or hostile folder path can never delete anything else.

use std::path::Path;

/// Files the app creates inside a meeting's recording folder.
const APP_FILES: &[&str] = &[
    "audio.mp4", "audio.m4a", "audio.wav", "audio.mp3", "audio.flac", "audio.ogg", "audio.mkv", "audio.webm",
    "audio.wma", "recording.mp4", "transcripts.json", "metadata.json",
];
/// At least one of these must be present for a folder to count as one of ours.
const MARKER_FILES: &[&str] = &["metadata.json", "transcripts.json"];
/// Hidden working files and folders the app leaves while recording or importing.
const CHECKPOINT_DIR: &str = ".checkpoints";
/// Prefixes of the temporary files the app leaves next to a recording. The earlier product used a
/// different one, and its leftovers are still cleaned up.
const TEMP_PREFIXES: [&str; 2] = [crate::brand_generated::TEMP_PREFIX, crate::brand_generated::LEGACY_TEMP_PREFIX];

/// Delete the app's recording files in `folder`, then the folder itself if that leaves it empty.
/// Returns how many files and folders were removed; `Ok(0)` if there was nothing to remove or the
/// folder is not recognisably one of the app's.
pub(crate) fn delete_recording_files(folder: &Path) -> std::io::Result<usize> {
    // Never follow a symlinked folder, and never act on anything that is not a plain folder
    let meta = match std::fs::symlink_metadata(folder) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Ok(0);
    }
    if !MARKER_FILES.iter().any(|name| folder.join(name).is_file()) {
        return Ok(0);
    }

    let mut removed = 0;
    for name in APP_FILES {
        let path = folder.join(name);
        // symlink_metadata: a symlink named like ours is removed itself, never followed
        if std::fs::symlink_metadata(&path).map_or(false, |m| m.is_file() || m.file_type().is_symlink()) {
            std::fs::remove_file(&path)?;
            removed += 1;
        }
    }
    let checkpoints = folder.join(CHECKPOINT_DIR);
    if std::fs::symlink_metadata(&checkpoints).map_or(false, |m| m.is_dir() && !m.file_type().is_symlink()) {
        std::fs::remove_dir_all(&checkpoints)?;
        removed += 1;
    }
    for entry in std::fs::read_dir(folder)?.flatten() {
        let name = entry.file_name();
        if TEMP_PREFIXES.iter().any(|p| name.to_string_lossy().starts_with(p)) && entry.path().is_file() {
            std::fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    // Remove the folder only if nothing else is in it (never recursive)
    if std::fs::remove_dir(folder).is_ok() {
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").unwrap();
    }

    #[test]
    fn removes_the_recording_and_then_the_empty_folder() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Meeting_2026");
        std::fs::create_dir_all(folder.join(".checkpoints")).unwrap();
        touch(&folder.join(".checkpoints"), "chunk_0.mp4");
        let temp_name = format!("{}abc.wav", crate::brand_generated::TEMP_PREFIX);
        for name in ["audio.mp4", "transcripts.json", "metadata.json", temp_name.as_str()] {
            touch(&folder, name);
        }
        let removed = delete_recording_files(&folder).unwrap();
        assert_eq!(removed, 6, "3 files + checkpoints dir + temp file + the folder");
        assert!(!folder.exists());
    }

    #[test]
    fn leaves_anything_the_app_did_not_create_and_keeps_the_folder_then() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Meeting");
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["audio.mp4", "metadata.json", "my-own-notes.txt", "photo.jpg"] {
            touch(&folder, name);
        }
        delete_recording_files(&folder).unwrap();
        assert!(!folder.join("audio.mp4").exists() && !folder.join("metadata.json").exists());
        assert!(folder.join("my-own-notes.txt").exists() && folder.join("photo.jpg").exists(), "user files survive");
        assert!(folder.exists(), "a folder that is not empty is not removed");
    }

    #[test]
    fn a_folder_without_the_apps_marker_files_is_left_completely_alone() {
        // e.g. a database row pointing at the user's own Music folder
        let root = tempfile::tempdir().unwrap();
        let music = root.path().join("Music");
        std::fs::create_dir_all(&music).unwrap();
        touch(&music, "audio.mp3");
        touch(&music, "recording.mp4");
        assert_eq!(delete_recording_files(&music).unwrap(), 0);
        assert!(music.join("audio.mp3").exists() && music.join("recording.mp4").exists());
    }

    #[test]
    fn missing_folders_files_and_odd_paths_are_not_errors() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(delete_recording_files(&root.path().join("nope")).unwrap(), 0);
        let file = root.path().join("a-file");
        touch(root.path(), "a-file");
        assert_eq!(delete_recording_files(&file).unwrap(), 0, "a file is not a recording folder");
        assert!(file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_never_followed() {
        let root = tempfile::tempdir().unwrap();
        let precious = root.path().join("precious");
        std::fs::create_dir_all(&precious).unwrap();
        touch(&precious, "metadata.json");
        touch(&precious, "audio.mp4");
        // a "meeting folder" that is really a link to somewhere else
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&precious, &link).unwrap();
        assert_eq!(delete_recording_files(&link).unwrap(), 0);
        assert!(precious.join("audio.mp4").exists());

        // a real folder whose audio.mp4 is a link: the link goes, the target stays
        let folder = root.path().join("Meeting");
        std::fs::create_dir_all(&folder).unwrap();
        touch(&folder, "metadata.json");
        let target = root.path().join("outside.dat");
        std::fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, folder.join("audio.mp4")).unwrap();
        delete_recording_files(&folder).unwrap();
        assert!(target.exists(), "the file the link pointed to is untouched");
    }
}
