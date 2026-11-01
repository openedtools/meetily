//! One-time adoption of the earlier product's data.
//!
//! The app's identifier names its data folder. A new identifier therefore looks like a first
//! launch: no meetings, onboarding again, and every model downloaded again. This module copies
//! what the person already has into the new folder, once, with their agreement.
//!
//! Guarantees: the old folder is never written to (it keeps working for the earlier app), an
//! interrupted copy leaves nothing half-made in the new folder, nothing already in the new folder
//! is overwritten, and a copy whose database comes from a newer version than this build
//! understands is refused. Model files are copied, not linked, so the two apps never share a file
//! that either of them might rewrite. Recordings are not copied: both apps keep pointing at the
//! same audio files, and the offer says so.
//!
//! It runs before the Tauri builder, so no app state exists yet and no database is open.

use crate::brand_generated as brand;
use crate::fs_retry;
use anyhow::{bail, Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const DB_FILE: &str = "meeting_minutes.sqlite";
pub const ADOPTED_MARKER: &str = ".adopted";
pub const DECLINED_MARKER: &str = ".declined";
/// Top-level JSON stores that are not carried over (usage statistics are gone from the app).
const SKIPPED_STORES: &[&str] = &["analytics.json"];
/// A copy lock older than this is assumed to belong to a process that died.
const STALE_LOCK: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Offer to copy the earlier data.
    Ask,
    /// Nothing to do, with the reason (for the log).
    NotNeeded(&'static str),
}

/// Where the earlier product kept data outside its data folder, and what this build understands.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub templates_dir: Option<PathBuf>,
    pub notifications_file: Option<PathBuf>,
    /// Newest database migration this build knows. A copied database that is ahead of it is refused.
    pub newest_migration: Option<i64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub model_files: usize,
}

/// What the person chose in the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    /// Closed with Esc or the window button: not a decision, so ask again next time.
    Dismissed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Adopted(Report),
    Declined,
    /// The offer was dismissed; nothing was written and the person will be asked again.
    Postponed,
    /// Another process finished the copy while the offer was open.
    AlreadyDone,
    /// Another process is copying right now.
    Busy,
    Failed(String),
}

/// Should the person be offered the earlier data?
pub fn decide(old_dir: &Path, new_dir: &Path) -> Decision {
    if old_dir == new_dir {
        return Decision::NotNeeded("the same folder");
    }
    if !old_dir.join(DB_FILE).is_file() {
        return Decision::NotNeeded("no data from the earlier version");
    }
    if new_dir.join(DB_FILE).exists() {
        return Decision::NotNeeded("the new folder already has its own data");
    }
    if new_dir.join(ADOPTED_MARKER).exists() {
        return Decision::NotNeeded("already adopted");
    }
    if new_dir.join(DECLINED_MARKER).exists() {
        return Decision::NotNeeded("declined earlier");
    }
    Decision::Ask
}

/// Remember that the person said no, so they are not asked again.
pub fn decline(new_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(new_dir)?;
    fs::write(new_dir.join(DECLINED_MARKER), b"The person chose not to bring over data from the earlier version.\n")
}

/// Copy the earlier data into `new_dir`. See the module notes for the guarantees.
pub async fn adopt_async(old_dir: &Path, new_dir: &Path, extras: &Extras) -> Result<Report> {
    let mut staging = staging_dir(new_dir, None);
    if staging.exists() {
        // Left by an interrupted run: it was never part of the new folder, so it is safe to drop
        if let Err(e) = fs::remove_dir_all(&staging) {
            log::warn!("Could not clear {} ({e}); using another working folder", staging.display());
            staging = staging_dir(new_dir, Some(std::process::id()));
            let _ = fs::remove_dir_all(&staging);
        }
    }
    fs::create_dir_all(&staging).context("could not create a working folder for the copy")?;

    let result = build_in_staging(old_dir, &staging, extras).await.and_then(|report| {
        install(&staging, new_dir)?;
        Ok(report)
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

/// Blocking wrapper for callers that have no async runtime yet (it runs before the app starts).
pub fn adopt(old_dir: &Path, new_dir: &Path, extras: &Extras) -> Result<Report> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("could not start a runtime for the copy")?
        .block_on(adopt_async(old_dir, new_dir, extras))
}

fn staging_dir(new_dir: &Path, unique: Option<u32>) -> PathBuf {
    let name = new_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "data".to_string());
    match unique {
        None => new_dir.with_file_name(format!("{name}.adopting")),
        Some(id) => new_dir.with_file_name(format!("{name}.adopting-{id}")),
    }
}

async fn build_in_staging(old_dir: &Path, staging: &Path, extras: &Extras) -> Result<Report> {
    let mut report = Report::default();

    let staged_db = staging.join(DB_FILE);
    snapshot_database(&old_dir.join(DB_FILE), &staged_db).await?;
    report.files += 1;
    check_database_is_not_newer(&staged_db, extras.newest_migration).await?;

    // Settings and progress markers (onboarding done, recordings folder, and so on)
    for entry in fs::read_dir(old_dir).context("could not read the earlier data folder")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_file() && name.ends_with(".json") && !SKIPPED_STORES.contains(&name.as_str()) {
            fs::copy(entry.path(), staging.join(&name)).with_context(|| format!("could not copy {name}"))?;
            report.files += 1;
        }
    }

    // Downloaded models
    let models = old_dir.join("models");
    if models.is_dir() {
        copy_tree(&models, &staging.join("models"), &mut report.model_files)?;
    }

    // Things the earlier product kept outside its data folder
    if let Some(templates) = extras.templates_dir.as_ref().filter(|p| p.is_dir()) {
        copy_tree(templates, &staging.join("templates"), &mut report.files)?;
    }
    if let Some(file) = extras.notifications_file.as_ref().filter(|p| p.is_file()) {
        fs::copy(file, staging.join("notifications.json")).context("could not copy notification settings")?;
        report.files += 1;
    }

    fs::write(staging.join(ADOPTED_MARKER), format!("Copied from {}\n", old_dir.display()))?;
    Ok(report)
}

/// A consistent snapshot of the earlier database. It is read through a read-only connection, so
/// its file is not changed and the earlier app can keep running.
async fn snapshot_database(from: &Path, to: &Path) -> Result<()> {
    let options = SqliteConnectOptions::new().filename(from).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("could not open the earlier database")?;
    let result = sqlx::query("VACUUM INTO ?1").bind(to.to_string_lossy().to_string()).execute(&pool).await;
    pool.close().await;
    result.context("could not copy the earlier database")?;
    Ok(())
}

/// Refuse a database saved by a newer version than this build understands: it would fail to open
/// on every launch. The copy is checked before it is put in place.
async fn check_database_is_not_newer(db: &Path, newest_known: Option<i64>) -> Result<()> {
    let Some(newest) = newest_known else {
        return Ok(());
    };
    let options = SqliteConnectOptions::new().filename(db).read_only(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
    let applied = crate::database::backup::applied_version(&pool).await;
    pool.close().await;
    if let Some(applied) = applied.context("could not read the copied database")? {
        if applied > newest {
            bail!(
                "the data was last saved by a newer version of {} than this version of {} understands. Update {} first",
                brand::LEGACY_NAME,
                brand::NAME,
                brand::NAME
            );
        }
    }
    Ok(())
}

/// Move the finished copy into place. An existing new folder keeps everything it already has.
fn install(staging: &Path, new_dir: &Path) -> Result<()> {
    if !new_dir.exists() {
        fs_retry::rename(staging, new_dir).context("could not put the copied data in place")?;
        return Ok(());
    }
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let dest = new_dir.join(entry.file_name());
        if dest.exists() {
            log::info!("Keeping the existing {}", dest.display());
            continue;
        }
        fs_retry::rename(&entry.path(), &dest).with_context(|| format!("could not move {} into place", dest.display()))?;
    }
    fs::remove_dir_all(staging)?;
    Ok(())
}

/// Copy a folder, counting the files. Symbolic links are skipped so a link cannot pull in
/// something from outside the data folder.
fn copy_tree(src: &Path, dst: &Path, count: &mut usize) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_symlink() {
            log::warn!("Skipping a link inside the earlier data: {}", entry.path().display());
        } else if kind.is_dir() {
            copy_tree(&entry.path(), &to, count)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &to).with_context(|| format!("could not copy {}", entry.path().display()))?;
            *count += 1;
        }
    }
    Ok(())
}

/// Total size of the regular files under `dir` (links are not followed).
fn tree_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            match entry.file_type() {
                Ok(k) if k.is_dir() => total += tree_size(&entry.path()),
                Ok(k) if k.is_file() => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

pub fn human_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{} KB", (bytes / 1024).max(1))
    }
}

/// Wording of the one-time offer (title, body). `size` is the earlier data folder's size.
pub fn offer_text(size: u64) -> (String, String) {
    (
        "Bring your data over?".to_string(),
        format!(
            "Found data from {legacy} on this computer (about {size}).\n\n\
             {name} can copy your meetings, people, folders, settings and downloaded models into its own folder, so you can carry on where you left off. For the safest copy, close {legacy} first.\n\n\
             Your original data is not changed, and {legacy} keeps working. Recordings are not copied: they stay where they are, so both apps use the same audio files, and deleting a meeting in one app removes its audio for the other. A recording that was open but not saved, and display preferences such as the language choice, cannot be carried over.\n\n\
             Copy it now?",
            legacy = brand::LEGACY_NAME,
            name = brand::NAME,
            size = human_size(size),
        ),
    )
}

/// A marker file that keeps two processes from copying at once. Removed when dropped.
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// `Ok(None)` means another process holds a fresh lock.
    pub fn acquire(new_dir: &Path) -> io::Result<Option<Lock>> {
        let name = new_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "data".to_string());
        let path = new_dir.with_file_name(format!("{name}.adopting.lock"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        for _ in 0..2 {
            match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Some(Lock { path })),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let age = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok());
                    match age {
                        Some(age) if age < STALE_LOCK => return Ok(None),
                        _ => {
                            log::warn!("Removing a stale copy lock: {}", path.display());
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }

    pub fn path_for(new_dir: &Path) -> PathBuf {
        let name = new_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "data".to_string());
        new_dir.with_file_name(format!("{name}.adopting.lock"))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The whole offer, with the question injected so it can be tested. Assumes `decide` said `Ask`.
pub fn run_offer(old: &Path, new: &Path, extras: &Extras, ask: &dyn Fn(&str, &str) -> Answer) -> Outcome {
    let (title, body) = offer_text(tree_size(old));
    match ask(&title, &body) {
        Answer::No => {
            if let Err(e) = decline(new) {
                log::warn!("Could not remember the choice not to bring data over: {e}");
            }
            Outcome::Declined
        }
        Answer::Dismissed => Outcome::Postponed,
        Answer::Yes => {
            let lock = match Lock::acquire(new) {
                Ok(Some(lock)) => lock,
                Ok(None) => return Outcome::Busy,
                Err(e) => return Outcome::Failed(format!("could not take the copy lock: {e}")),
            };
            if decide(old, new) != Decision::Ask {
                return Outcome::AlreadyDone; // another process finished while the offer was open
            }
            let result = adopt(old, new, extras);
            drop(lock);
            match result {
                Ok(report) => Outcome::Adopted(report),
                Err(e) => Outcome::Failed(format!("{e:#}")),
            }
        }
    }
}

/// Offer the earlier data once, before the app starts. A problem here is reported, and the person
/// chooses whether to open the app anyway.
pub fn maybe_adopt() {
    let Some(data_dir) = dirs::data_dir() else {
        return;
    };
    let old = data_dir.join(brand::LEGACY_IDENTIFIER);
    let new = data_dir.join(brand::BUNDLE_ID);
    if let Decision::NotNeeded(why) = decide(&old, &new) {
        log::info!("No data to bring over: {why}");
        return;
    }
    let extras = Extras {
        templates_dir: Some(data_dir.join(brand::LEGACY_TEMPLATES_DIR).join("templates")),
        notifications_file: dirs::config_dir().map(|c| c.join(brand::LEGACY_CONFIG_DIR).join("notifications.json")),
        newest_migration: sqlx::migrate!("./migrations").migrations.last().map(|m| m.version),
    };
    match run_offer(&old, &new, &extras, &confirm) {
        Outcome::Adopted(report) => log::info!("Brought data over from {}: {report:?}", old.display()),
        Outcome::Declined => log::info!("The person chose not to bring data over"),
        Outcome::Postponed => log::info!("The offer was dismissed; it will be shown again next time"),
        Outcome::AlreadyDone => log::info!("The data was brought over by another window"),
        Outcome::Busy => {
            show_error(
                &format!("{} is already setting up", brand::NAME),
                &format!(
                    "Another window is bringing your data over right now. Wait for it to finish, then open {} again.\n\nIf this message keeps appearing, delete {}.",
                    brand::NAME,
                    Lock::path_for(&new).display()
                ),
            );
            std::process::exit(0);
        }
        Outcome::Failed(why) => {
            log::error!("Could not bring data over from {}: {why}", old.display());
            let body = format!(
                "{name} could not copy the data from {legacy}. Your original data is untouched.\n\nDetails: {why}\n\nOpen {name} with no data now? Choose No to close it and try again later. If you open it empty and want to copy later, close it, delete the folder {folder}, and open it again.",
                name = brand::NAME,
                legacy = brand::LEGACY_NAME,
                folder = new.display(),
            );
            if confirm("Your data was not copied", &body) != Answer::Yes {
                std::process::exit(1);
            }
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn confirm(title: &str, body: &str) -> Answer {
    match rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
    {
        rfd::MessageDialogResult::Yes => Answer::Yes,
        rfd::MessageDialogResult::No => Answer::No,
        _ => Answer::Dismissed,
    }
}

/// Tell the person about a problem in a window, because a release build has no console.
#[cfg(any(windows, target_os = "macos"))]
pub fn show_error(title: &str, body: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

// Other platforms have no dialog toolkit linked in: bring the data over without asking.
#[cfg(not(any(windows, target_os = "macos")))]
fn confirm(_title: &str, _body: &str) -> Answer {
    Answer::Yes
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn show_error(title: &str, body: &str) {
    log::error!("{title}: {body}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Row, SqlitePool};

    async fn open(path: &Path) -> SqlitePool {
        let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
        SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
    }

    /// An earlier install with data, left open as if the old app were still running.
    async fn old_install(dir: &Path) -> SqlitePool {
        fs::create_dir_all(dir.join("models/parakeet")).unwrap();
        fs::write(dir.join("models/parakeet/model.onnx"), vec![7u8; 4096]).unwrap();
        fs::write(dir.join("models/ggml-small.bin"), vec![9u8; 2048]).unwrap();
        fs::write(dir.join("onboarding-status.json"), r#"{"completed":true}"#).unwrap();
        fs::write(dir.join("recording_preferences.json"), r#"{"preferences":{"save_folder":"D:\\recs"}}"#).unwrap();
        fs::write(dir.join("preferences.json"), r#"{"theme":"x"}"#).unwrap();
        fs::write(dir.join("analytics.json"), r#"{"user":"x"}"#).unwrap();
        fs::write(dir.join("recording-123.wav"), b"stray").unwrap();
        fs::create_dir_all(dir.join("backups")).unwrap();
        fs::write(dir.join("backups/old.bak"), b"bak").unwrap();

        let pool = open(&dir.join(DB_FILE)).await;
        sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await.unwrap();
        sqlx::query("PRAGMA wal_autocheckpoint = 0").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT)").execute(&pool).await.unwrap();
        for (id, title) in [("m1", "Weekly sync"), ("m2", "Planning")] {
            sqlx::query("INSERT INTO meetings VALUES (?, ?)").bind(id).bind(title).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn titles(db: &Path) -> Vec<String> {
        let options = SqliteConnectOptions::new().filename(db).read_only(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
        let rows = sqlx::query("SELECT title FROM meetings ORDER BY id").fetch_all(&pool).await.unwrap();
        rows.iter().map(|r| r.get::<String, _>("title")).collect()
    }

    /// Every regular file under `dir` with its bytes, except SQLite's transient `-shm` file.
    fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for e in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, out);
                } else if !p.to_string_lossy().ends_with("-shm") {
                    out.push((p.strip_prefix(base).unwrap().to_string_lossy().to_string(), fs::read(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn dirs() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("com.old.app");
        let new = root.path().join("com.new.app");
        (root, old, new)
    }

    #[test]
    fn the_offer_is_honest_about_what_is_and_is_not_copied() {
        let (title, body) = offer_text(3 * 1024 * 1024 * 1024);
        assert!(body.contains(brand::LEGACY_NAME) && body.contains(brand::NAME));
        assert!(body.contains("(about 3.0 GB)"));
        assert!(body.contains("close") && body.contains("first"), "advises closing the old app");
        assert!(body.contains("not changed"));
        assert!(body.contains("Recordings are not copied"));
        assert!(body.contains("deleting a meeting in one app removes its audio for the other"));
        assert!(body.contains("not saved") && body.contains("language"));
        assert!(!format!("{title}{body}").contains(char::from_u32(0x2014).unwrap()));
        assert!(title.len() < 60);
    }

    #[test]
    fn sizes_are_shown_in_plain_units() {
        assert_eq!(human_size(0), "1 KB");
        assert_eq!(human_size(900 * 1024), "900 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5 MB");
        assert_eq!(human_size(2 * 1024 * 1024 * 1024 + 512 * 1024 * 1024), "2.5 GB");
    }

    #[tokio::test]
    async fn decide_covers_every_condition() {
        let (_root, old, new) = dirs();
        assert_eq!(decide(&old, &new), Decision::NotNeeded("no data from the earlier version"));

        let _pool = old_install(&old).await;
        assert_eq!(decide(&old, &new), Decision::Ask);
        assert_eq!(decide(&old, &old), Decision::NotNeeded("the same folder"));

        fs::create_dir_all(&new).unwrap();
        fs::write(new.join(DB_FILE), b"x").unwrap();
        assert_eq!(decide(&old, &new), Decision::NotNeeded("the new folder already has its own data"));
        fs::remove_file(new.join(DB_FILE)).unwrap();

        fs::write(new.join(ADOPTED_MARKER), b"").unwrap();
        assert_eq!(decide(&old, &new), Decision::NotNeeded("already adopted"));
        fs::remove_file(new.join(ADOPTED_MARKER)).unwrap();

        decline(&new).unwrap();
        assert_eq!(decide(&old, &new), Decision::NotNeeded("declined earlier"));
    }

    #[tokio::test]
    async fn decline_creates_the_folder_and_is_remembered() {
        let (_root, old, new) = dirs();
        let _pool = old_install(&old).await;
        assert!(!new.exists());
        decline(&new).unwrap();
        assert!(new.join(DECLINED_MARKER).is_file());
        assert!(!new.join(DB_FILE).exists(), "declining copies nothing");
    }

    #[tokio::test]
    async fn the_database_is_copied_even_while_the_old_app_has_it_open() {
        let (_root, old, new) = dirs();
        let pool = old_install(&old).await; // rows are still in the write-ahead log
        let report = adopt_async(&old, &new, &Extras::default()).await.unwrap();
        assert_eq!(titles(&new.join(DB_FILE)).await, ["Weekly sync", "Planning"]);
        assert!(report.files >= 1);
        drop(pool);
    }

    #[tokio::test]
    async fn the_old_folder_is_left_exactly_as_it_was() {
        let (_root, old, new) = dirs();
        let pool = old_install(&old).await;
        let before = snapshot(&old);
        adopt_async(&old, &new, &Extras::default()).await.unwrap();
        let after = snapshot(&old);
        assert_eq!(before, after);
        // And the old app can still use its database
        sqlx::query("INSERT INTO meetings VALUES ('m3', 'Later')").execute(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn settings_are_carried_over_but_statistics_strays_and_backups_are_not() {
        let (_root, old, new) = dirs();
        let _pool = old_install(&old).await;
        adopt_async(&old, &new, &Extras::default()).await.unwrap();
        for kept in ["onboarding-status.json", "recording_preferences.json", "preferences.json", DB_FILE, ADOPTED_MARKER] {
            assert!(new.join(kept).is_file(), "{kept} should be carried over");
        }
        for left in ["analytics.json", "recording-123.wav", "backups"] {
            assert!(!new.join(left).exists(), "{left} should not be carried over");
        }
        assert_eq!(fs::read_to_string(new.join("recording_preferences.json")).unwrap(), r#"{"preferences":{"save_folder":"D:\\recs"}}"#);
    }

    #[tokio::test]
    async fn models_are_copied_so_the_two_apps_never_share_a_file() {
        let (_root, old, new) = dirs();
        let _pool = old_install(&old).await;
        let report = adopt_async(&old, &new, &Extras::default()).await.unwrap();
        assert_eq!(report.model_files, 2);
        assert_eq!(fs::read(new.join("models/parakeet/model.onnx")).unwrap(), vec![7u8; 4096]);
        assert_eq!(fs::read(new.join("models/ggml-small.bin")).unwrap(), vec![9u8; 2048]);

        // Rewriting a model in the new folder (a failed re-download truncates it) leaves the old one alone
        fs::write(new.join("models/ggml-small.bin"), b"").unwrap();
        assert_eq!(fs::read(old.join("models/ggml-small.bin")).unwrap(), vec![9u8; 2048]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = fs::metadata(old.join("models/parakeet/model.onnx")).unwrap().ino();
            let b = fs::metadata(new.join("models/parakeet/model.onnx")).unwrap().ino();
            assert_ne!(a, b, "separate files on disk");
        }
    }

    #[tokio::test]
    async fn a_failure_leaves_nothing_in_the_new_folder() {
        let (root, old, new) = dirs();
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join(DB_FILE), b"this is not a database").unwrap();
        let err = adopt_async(&old, &new, &Extras::default()).await;
        assert!(err.is_err());
        assert!(!new.exists(), "no half-made new folder");
        let leftovers: Vec<_> = fs::read_dir(root.path()).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(leftovers, ["com.old.app"], "no staging folder left behind");
    }

    #[tokio::test]
    async fn a_stale_staging_folder_from_an_interrupted_run_is_replaced() {
        let (root, old, new) = dirs();
        let _pool = old_install(&old).await;
        let stale = root.path().join("com.new.app.adopting");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("junk"), b"half").unwrap();
        adopt_async(&old, &new, &Extras::default()).await.unwrap();
        assert!(!stale.exists());
        assert!(!new.join("junk").exists());
        assert_eq!(titles(&new.join(DB_FILE)).await.len(), 2);
    }

    #[tokio::test]
    async fn what_the_new_folder_already_has_is_never_overwritten() {
        let (_root, old, new) = dirs();
        let _pool = old_install(&old).await;
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("recording_preferences.json"), r#"{"mine":true}"#).unwrap();
        adopt_async(&old, &new, &Extras::default()).await.unwrap();
        assert_eq!(fs::read_to_string(new.join("recording_preferences.json")).unwrap(), r#"{"mine":true}"#);
        assert!(new.join(DB_FILE).is_file(), "the missing parts are still added");
        assert!(new.join("onboarding-status.json").is_file());
    }

    #[tokio::test]
    async fn templates_and_notification_settings_from_their_old_homes_are_brought_in() {
        let (root, old, new) = dirs();
        let _pool = old_install(&old).await;
        let templates = root.path().join("old-templates-home/templates");
        fs::create_dir_all(&templates).unwrap();
        fs::write(templates.join("standup.json"), r#"{"name":"standup"}"#).unwrap();
        let notifications = root.path().join("old-config-home/notifications.json");
        fs::create_dir_all(notifications.parent().unwrap()).unwrap();
        fs::write(&notifications, r#"{"enabled":false}"#).unwrap();

        let extras = Extras { templates_dir: Some(templates), notifications_file: Some(notifications), newest_migration: None };
        adopt_async(&old, &new, &extras).await.unwrap();
        assert_eq!(fs::read_to_string(new.join("templates/standup.json")).unwrap(), r#"{"name":"standup"}"#);
        assert_eq!(fs::read_to_string(new.join("notifications.json")).unwrap(), r#"{"enabled":false}"#);
    }

    #[tokio::test]
    async fn symbolic_links_inside_the_old_folder_are_not_followed() {
        #[cfg(unix)]
        {
            let (root, old, new) = dirs();
            let _pool = old_install(&old).await;
            let secret = root.path().join("secret.txt");
            fs::write(&secret, b"private").unwrap();
            std::os::unix::fs::symlink(&secret, old.join("models/escape.bin")).unwrap();
            adopt_async(&old, &new, &Extras::default()).await.unwrap();
            assert!(!new.join("models/escape.bin").exists(), "a link out of the folder is skipped");
        }
    }

    #[tokio::test]
    async fn an_adopted_database_opens_under_the_real_migrations_without_complaint() {
        use crate::database::manager::DatabaseManager;
        let (_root, old, new) = dirs();
        fs::create_dir_all(&old).unwrap();

        // The earlier app's database: fully migrated, with a meeting in it
        let earlier = DatabaseManager::new(old.join(DB_FILE).to_str().unwrap(), "none.db").await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('m1', 'Weekly sync', 'x', 'x')")
            .execute(earlier.pool())
            .await
            .unwrap();

        adopt_async(&old, &new, &Extras::default()).await.unwrap();

        // Opening the copy runs the migration check: matching checksums mean it is accepted as is,
        // and nothing needs backing up because nothing is pending
        let adopted = DatabaseManager::new(new.join(DB_FILE).to_str().unwrap(), "none.db").await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings").fetch_one(adopted.pool()).await.unwrap();
        assert_eq!(n, 1);
        assert!(!crate::database::backup::backups_dir(&new.join(DB_FILE)).exists());
    }

    #[tokio::test]
    async fn a_database_from_a_newer_version_is_refused_and_nothing_is_left_behind() {
        use crate::database::manager::DatabaseManager;
        let (root, old, new) = dirs();
        fs::create_dir_all(&old).unwrap();
        let _earlier = DatabaseManager::new(old.join(DB_FILE).to_str().unwrap(), "none.db").await.unwrap();
        let newest = sqlx::migrate!("./migrations").migrations.last().unwrap().version;

        // This build knows one migration fewer than the data has
        let err = adopt_async(&old, &new, &Extras { newest_migration: Some(newest - 1), ..Extras::default() })
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("newer version"), "{err:#}");
        assert!(!new.exists());
        assert!(!root.path().join("com.new.app.adopting").exists());

        // And it is accepted when the build knows the same migrations
        adopt_async(&old, &new, &Extras { newest_migration: Some(newest), ..Extras::default() }).await.unwrap();
        assert!(new.join(DB_FILE).is_file());
    }

    // The offer as a whole. These are plain tests: adopt() starts its own runtime.
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }

    #[test]
    fn yes_copies_the_data_and_releases_the_lock() {
        let (root, old, new) = dirs();
        let rt = runtime();
        let _pool = rt.block_on(old_install(&old));
        let outcome = run_offer(&old, &new, &Extras::default(), &|_, _| Answer::Yes);
        assert!(matches!(outcome, Outcome::Adopted(_)), "{outcome:?}");
        assert!(new.join(DB_FILE).is_file());
        assert!(!Lock::path_for(&new).exists(), "lock released");
        assert!(root.path().join("com.old.app").exists());
    }

    #[test]
    fn no_is_remembered_and_copies_nothing() {
        let (_root, old, new) = dirs();
        let rt = runtime();
        let _pool = rt.block_on(old_install(&old));
        assert_eq!(run_offer(&old, &new, &Extras::default(), &|_, _| Answer::No), Outcome::Declined);
        assert!(new.join(DECLINED_MARKER).is_file());
        assert!(!new.join(DB_FILE).exists());
        assert_eq!(decide(&old, &new), Decision::NotNeeded("declined earlier"));
    }

    #[test]
    fn dismissing_the_offer_is_not_a_decision() {
        let (_root, old, new) = dirs();
        let rt = runtime();
        let _pool = rt.block_on(old_install(&old));
        assert_eq!(run_offer(&old, &new, &Extras::default(), &|_, _| Answer::Dismissed), Outcome::Postponed);
        assert!(!new.exists(), "nothing written, so the person is asked again next time");
        assert_eq!(decide(&old, &new), Decision::Ask);
    }

    #[test]
    fn the_offer_shows_the_size_of_the_earlier_data() {
        let (_root, old, new) = dirs();
        let rt = runtime();
        let _pool = rt.block_on(old_install(&old));
        let seen = std::cell::RefCell::new(String::new());
        run_offer(&old, &new, &Extras::default(), &|_, body| {
            *seen.borrow_mut() = body.to_string();
            Answer::Dismissed
        });
        assert!(seen.borrow().contains("(about "), "{}", seen.borrow());
    }

    #[test]
    fn a_second_process_is_turned_away_while_a_copy_is_running() {
        let (_root, old, new) = dirs();
        let rt = runtime();
        let _pool = rt.block_on(old_install(&old));
        let _held = Lock::acquire(&new).unwrap().expect("first process gets the lock");
        assert_eq!(run_offer(&old, &new, &Extras::default(), &|_, _| Answer::Yes), Outcome::Busy);
        assert!(!new.join(DB_FILE).exists());
    }

    #[test]
    fn a_lock_left_by_a_dead_process_does_not_block_forever() {
        let (_root, old, new) = dirs();
        fs::create_dir_all(&old).unwrap();
        let first = Lock::acquire(&new).unwrap().unwrap();
        std::mem::forget(first); // simulate a crash: the file stays behind
        let path = Lock::path_for(&new);
        assert!(path.exists());
        assert!(Lock::acquire(&new).unwrap().is_none(), "a fresh lock is respected");

        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(2 * 60 * 60)).unwrap();
        drop(file);
        assert!(Lock::acquire(&new).unwrap().is_some(), "an old lock is replaced");
    }

    #[test]
    fn a_failed_copy_is_reported_with_its_reason_and_leaves_no_lock() {
        use crate::database::manager::DatabaseManager;
        let (_root, old, new) = dirs();
        let rt = runtime();
        fs::create_dir_all(&old).unwrap();
        let _earlier = rt.block_on(DatabaseManager::new(old.join(DB_FILE).to_str().unwrap(), "none.db")).unwrap();
        let extras = Extras { newest_migration: Some(1), ..Extras::default() };
        match run_offer(&old, &new, &extras, &|_, _| Answer::Yes) {
            Outcome::Failed(why) => assert!(why.contains("newer version"), "{why}"),
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(!Lock::path_for(&new).exists());
        assert!(!new.exists());
    }
}
