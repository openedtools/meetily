//! One-time adoption of the earlier product's data.
//!
//! The app's identifier names its data folder. A new identifier therefore looks like a first
//! launch: no meetings, onboarding again, and every model downloaded again. This module copies
//! what the person already has into the new folder, once, with their agreement.
//!
//! Guarantees: the old folder is never changed (it keeps working for the earlier app), an
//! interrupted copy leaves nothing half-made in the new folder, and nothing already in the new
//! folder is overwritten. Model files are hard-linked where the disk allows, so several gigabytes
//! are not duplicated.
//!
//! It runs before the Tauri builder, so no app state exists yet and no database is open.

use crate::brand_generated as brand;
use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DB_FILE: &str = "meeting_minutes.sqlite";
pub const ADOPTED_MARKER: &str = ".adopted";
pub const DECLINED_MARKER: &str = ".declined";
/// Top-level JSON stores that are not carried over (usage statistics are gone from the app).
const SKIPPED_STORES: &[&str] = &["analytics.json"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Offer to copy the earlier data.
    Ask,
    /// Nothing to do, with the reason (for the log).
    NotNeeded(&'static str),
}

/// Where the earlier product kept data that does not live in its data folder.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub templates_dir: Option<PathBuf>,
    pub notifications_file: Option<PathBuf>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub models_linked: usize,
    pub models_copied: usize,
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
    let staging = staging_dir(new_dir);
    if staging.exists() {
        // Left by an interrupted run: it was never part of the new folder, so it is safe to drop
        fs::remove_dir_all(&staging).context("could not clear an earlier unfinished copy")?;
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

fn staging_dir(new_dir: &Path) -> PathBuf {
    let name = new_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "data".to_string());
    new_dir.with_file_name(format!("{name}.adopting"))
}

async fn build_in_staging(old_dir: &Path, staging: &Path, extras: &Extras) -> Result<Report> {
    let mut report = Report::default();

    snapshot_database(&old_dir.join(DB_FILE), &staging.join(DB_FILE)).await?;
    report.files += 1;

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
        place_tree(&models, &staging.join("models"), &mut report)?;
    }

    // Things the earlier product kept outside its data folder
    if let Some(templates) = extras.templates_dir.as_ref().filter(|p| p.is_dir()) {
        copy_tree(templates, &staging.join("templates"), &mut report)?;
    }
    if let Some(file) = extras.notifications_file.as_ref().filter(|p| p.is_file()) {
        fs::copy(file, staging.join("notifications.json")).context("could not copy notification settings")?;
        report.files += 1;
    }

    fs::write(
        staging.join(ADOPTED_MARKER),
        format!("Copied from {}\n", old_dir.display()),
    )?;
    Ok(report)
}

/// A consistent snapshot of the earlier database. It is read through a read-only connection, so
/// the earlier app can still be running and its file is not changed.
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

/// Move the finished copy into place. An existing new folder keeps everything it already has.
fn install(staging: &Path, new_dir: &Path) -> Result<()> {
    if !new_dir.exists() {
        fs::rename(staging, new_dir).context("could not put the copied data in place")?;
        return Ok(());
    }
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let dest = new_dir.join(entry.file_name());
        if dest.exists() {
            log::info!("Keeping the existing {}", dest.display());
            continue;
        }
        fs::rename(entry.path(), &dest).with_context(|| format!("could not move {} into place", dest.display()))?;
    }
    fs::remove_dir_all(staging)?;
    Ok(())
}

/// Hard-link (or copy) a folder of immutable files such as downloaded models. Symbolic links are
/// skipped so a link cannot pull in something from outside the data folder.
fn place_tree(src: &Path, dst: &Path, report: &mut Report) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_symlink() {
            log::warn!("Skipping a link inside the earlier data: {}", entry.path().display());
        } else if kind.is_dir() {
            place_tree(&entry.path(), &to, report)?;
        } else if kind.is_file() {
            let linked = place_file(&entry.path(), &to, &|a, b| fs::hard_link(a, b))
                .with_context(|| format!("could not copy {}", entry.path().display()))?;
            if linked {
                report.models_linked += 1;
            } else {
                report.models_copied += 1;
            }
        }
    }
    Ok(())
}

/// Plain recursive copy for small folders such as templates. Symbolic links are skipped.
fn copy_tree(src: &Path, dst: &Path, report: &mut Report) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &to, report)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &to)?;
            report.files += 1;
        }
    }
    Ok(())
}

/// Wording of the one-time offer (title, body).
pub fn offer_text() -> (String, String) {
    (
        "Bring your data over?".to_string(),
        format!(
            "Found data from {legacy} on this computer.\n\n{name} can copy your meetings, people, folders, settings and downloaded models into its own folder, so you can carry on where you left off.\n\nYour original data is not changed, and {legacy} keeps working. A recording that was open but not saved cannot be carried over.\n\nCopy it now?",
            legacy = brand::LEGACY_NAME,
            name = brand::NAME,
        ),
    )
}

/// Offer the earlier data once, before the app starts. A problem here is reported and logged
/// but never stops the app from opening.
pub fn maybe_adopt() {
    let Some(data_dir) = dirs::data_dir() else {
        return;
    };
    let old = data_dir.join(brand::LEGACY_IDENTIFIER);
    let new = data_dir.join(brand::BUNDLE_ID);
    match decide(&old, &new) {
        Decision::NotNeeded(why) => log::info!("No data to bring over: {why}"),
        Decision::Ask => {
            let (title, body) = offer_text();
            if !confirm(&title, &body) {
                if let Err(e) = decline(&new) {
                    log::warn!("Could not remember the choice not to bring data over: {e}");
                }
                return;
            }
            let extras = Extras {
                templates_dir: Some(data_dir.join(brand::LEGACY_TEMPLATES_DIR).join("templates")),
                notifications_file: dirs::config_dir()
                    .map(|c| c.join(brand::LEGACY_CONFIG_DIR).join("notifications.json")),
            };
            match adopt(&old, &new, &extras) {
                Ok(report) => log::info!("Brought data over from {}: {report:?}", old.display()),
                Err(e) => {
                    log::error!("Could not bring data over from {}: {e:#}", old.display());
                    tell(
                        "Your data was not copied",
                        &format!(
                            "{} could not copy the data from {}. Your original data is untouched, and {} will start empty. Close the app and open it again to be asked once more.\n\nDetails: {e}",
                            brand::NAME,
                            brand::LEGACY_NAME,
                            brand::NAME
                        ),
                    );
                }
            }
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn confirm(title: &str, body: &str) -> bool {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

#[cfg(any(windows, target_os = "macos"))]
fn tell(title: &str, body: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

// Other platforms have no dialog toolkit linked in: bring the data over without asking.
#[cfg(not(any(windows, target_os = "macos")))]
fn confirm(_title: &str, _body: &str) -> bool {
    true
}

#[cfg(not(any(windows, target_os = "macos")))]
fn tell(_title: &str, body: &str) {
    log::error!("{body}");
}

/// Hard-link `src` to `dst`, or copy it when linking is not possible (another drive, a file
/// system without links). The link function is a parameter so the fallback can be tested.
fn place_file(src: &Path, dst: &Path, link: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<bool> {
    match link(src, dst) {
        Ok(()) => Ok(true),
        Err(e) => {
            log::info!("Could not link {} ({e}); copying instead", src.display());
            fs::copy(src, dst)?;
            Ok(false)
        }
    }
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
    fn the_offer_names_both_products_and_promises_the_original_is_untouched() {
        let (title, body) = offer_text();
        assert!(body.contains(brand::LEGACY_NAME) && body.contains(brand::NAME));
        assert!(body.contains("not changed"));
        assert!(body.contains("not saved cannot be carried over"));
        assert!(!format!("{title}{body}").contains(char::from_u32(0x2014).unwrap()));
        assert!(title.len() < 60);
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
    async fn models_are_linked_not_duplicated_where_possible() {
        let (_root, old, new) = dirs();
        let _pool = old_install(&old).await;
        let report = adopt_async(&old, &new, &Extras::default()).await.unwrap();
        assert_eq!(fs::read(new.join("models/parakeet/model.onnx")).unwrap(), vec![7u8; 4096]);
        assert_eq!(fs::read(new.join("models/ggml-small.bin")).unwrap(), vec![9u8; 2048]);
        assert_eq!(report.models_linked + report.models_copied, 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = fs::metadata(old.join("models/ggml-small.bin")).unwrap().ino();
            let b = fs::metadata(new.join("models/ggml-small.bin")).unwrap().ino();
            assert_eq!(a, b, "same file on disk");
            assert_eq!(report.models_linked, 2);
        }
    }

    #[test]
    fn a_failed_link_falls_back_to_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.bin");
        let dst = dir.path().join("b.bin");
        fs::write(&src, b"payload").unwrap();
        let no_links = |_: &Path, _: &Path| -> io::Result<()> { Err(io::Error::new(io::ErrorKind::Other, "cross-device")) };
        let linked = place_file(&src, &dst, &no_links).unwrap();
        assert!(!linked, "reported as copied");
        assert_eq!(fs::read(&dst).unwrap(), b"payload");

        let dst2 = dir.path().join("c.bin");
        assert!(place_file(&src, &dst2, &|a, b| fs::hard_link(a, b)).unwrap(), "reported as linked");
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

        let extras = Extras { templates_dir: Some(templates), notifications_file: Some(notifications) };
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
}
