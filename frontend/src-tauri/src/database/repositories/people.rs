use crate::people::models::{MeetingSpeaker, Person, PersonSummary};
use chrono::Utc;
use sqlx::{Error as SqlxError, SqliteConnection, SqlitePool};
use uuid::Uuid;

const MAX_NAME_LEN: usize = 120;

#[derive(Debug, thiserror::Error)]
pub enum PeopleError {
    /// A problem with what the caller asked for; the message is safe to show in the UI.
    #[error("{0}")]
    Invalid(String),
    #[error("Database error: {0}")]
    Db(#[from] SqlxError),
}

fn invalid<T>(msg: impl Into<String>) -> Result<T, PeopleError> {
    Err(PeopleError::Invalid(msg.into()))
}

/// Collapse runs of whitespace and reject empty or oversized names.
fn normalize_name(name: &str) -> Result<String, PeopleError> {
    let cleaned = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return invalid("Name cannot be empty");
    }
    if cleaned.chars().count() > MAX_NAME_LEN {
        return invalid(format!("Name is too long (max {} characters)", MAX_NAME_LEN));
    }
    Ok(cleaned)
}

/// Speaker keys are "S1", "S2", ... with no leading zeros, so one speaker has one spelling.
fn speaker_number(key: &str) -> Option<u32> {
    let n: u32 = key.strip_prefix('S')?.parse().ok()?;
    (n > 0 && format!("S{}", n) == key).then_some(n)
}

fn check_speaker_key(key: &str) -> Result<(), PeopleError> {
    match speaker_number(key) {
        Some(_) => Ok(()),
        None => invalid(format!("Invalid speaker key: {}", key)),
    }
}

/// Escape LIKE wildcards so a typed "50%" or "a_b" matches literally.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub struct PeopleRepository;

impl PeopleRepository {
    // ---------------------------------------------------------------- people

    /// All people with meeting counts, optionally filtered by a name fragment.
    pub async fn list_people(
        pool: &SqlitePool,
        query: Option<&str>,
    ) -> Result<Vec<PersonSummary>, PeopleError> {
        let pattern = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(|q| format!("%{}%", escape_like(q)));

        let people = sqlx::query_as::<_, PersonSummary>(
            r#"SELECT p.id, p.name,
                      COUNT(DISTINCT ms.meeting_id) AS meeting_count,
                      MAX(m.created_at) AS last_meeting_at
               FROM people p
               LEFT JOIN meeting_speakers ms ON ms.person_id = p.id
               LEFT JOIN meetings m ON m.id = ms.meeting_id
               WHERE ?1 IS NULL OR p.name LIKE ?1 ESCAPE '\'
               GROUP BY p.id, p.name
               ORDER BY p.name COLLATE NOCASE"#,
        )
        .bind(pattern)
        .fetch_all(pool)
        .await?;
        Ok(people)
    }

    async fn find_person_by_name(
        conn: &mut SqliteConnection,
        name: &str,
    ) -> Result<Option<Person>, SqlxError> {
        sqlx::query_as::<_, Person>("SELECT * FROM people WHERE name = ? COLLATE NOCASE")
            .bind(name)
            .fetch_optional(conn)
            .await
    }

    async fn find_person_by_id(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> Result<Option<Person>, SqlxError> {
        sqlx::query_as::<_, Person>("SELECT * FROM people WHERE id = ?")
            .bind(id)
            .fetch_optional(conn)
            .await
    }

    /// Return the person with this name (case-insensitive), creating them if new.
    /// This is what makes a name typed once reappear in the dropdown from then on.
    pub async fn get_or_create_person(pool: &SqlitePool, name: &str) -> Result<Person, PeopleError> {
        let name = normalize_name(name)?;
        let mut conn = pool.acquire().await?;

        if let Some(existing) = Self::find_person_by_name(&mut conn, &name).await? {
            return Ok(existing);
        }

        let now = Utc::now().to_rfc3339();
        let person = Person {
            id: format!("person-{}", Uuid::new_v4()),
            name: name.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        let inserted = sqlx::query(
            "INSERT INTO people (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&person.id)
        .bind(&person.name)
        .bind(&person.created_at)
        .bind(&person.updated_at)
        .execute(&mut *conn)
        .await;

        match inserted {
            Ok(_) => Ok(person),
            // Lost a race against the unique name index: hand back whoever won.
            Err(e) => match Self::find_person_by_name(&mut conn, &name).await? {
                Some(winner) => Ok(winner),
                None => Err(e.into()),
            },
        }
    }

    pub async fn rename_person(
        pool: &SqlitePool,
        person_id: &str,
        new_name: &str,
    ) -> Result<Person, PeopleError> {
        let name = normalize_name(new_name)?;
        let mut conn = pool.acquire().await?;

        if let Some(clash) = Self::find_person_by_name(&mut conn, &name).await? {
            if clash.id != person_id {
                return invalid(format!("A person named \"{}\" already exists", clash.name));
            }
        }

        let updated = sqlx::query("UPDATE people SET name = ?, updated_at = ? WHERE id = ?")
            .bind(&name)
            .bind(Utc::now().to_rfc3339())
            .bind(person_id)
            .execute(&mut *conn)
            .await?;
        if updated.rows_affected() == 0 {
            return invalid("Person not found");
        }

        match Self::find_person_by_id(&mut conn, person_id).await? {
            Some(person) => Ok(person),
            None => invalid("Person not found"),
        }
    }

    /// Remove a person. Their speaker slots stay in each meeting, back to "Speaker N".
    pub async fn delete_person(pool: &SqlitePool, person_id: &str) -> Result<bool, PeopleError> {
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE meeting_speakers SET person_id = NULL WHERE person_id = ?")
            .bind(person_id)
            .execute(&mut *tx)
            .await?;
        let deleted = sqlx::query("DELETE FROM people WHERE id = ?")
            .bind(person_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(deleted.rows_affected() > 0)
    }

    // -------------------------------------------------------------- speakers

    async fn fetch_speakers(
        conn: &mut SqliteConnection,
        meeting_id: &str,
    ) -> Result<Vec<MeetingSpeaker>, SqlxError> {
        sqlx::query_as::<_, MeetingSpeaker>(
            r#"SELECT s.speaker_key, s.person_id, p.name AS person_name,
                      (SELECT COUNT(*) FROM transcripts t
                        WHERE t.meeting_id = s.meeting_id AND t.speaker = s.speaker_key) AS segment_count
               FROM meeting_speakers s
               LEFT JOIN people p ON p.id = s.person_id
               WHERE s.meeting_id = ?
               ORDER BY CAST(SUBSTR(s.speaker_key, 2) AS INTEGER)"#,
        )
        .bind(meeting_id)
        .fetch_all(conn)
        .await
    }

    pub async fn get_meeting_speakers(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        let mut conn = pool.acquire().await?;
        Ok(Self::fetch_speakers(&mut conn, meeting_id).await?)
    }

    async fn require_meeting(conn: &mut SqliteConnection, meeting_id: &str) -> Result<(), PeopleError> {
        let found: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(conn)
            .await?;
        match found {
            Some(_) => Ok(()),
            None => invalid("Meeting not found"),
        }
    }

    /// Create the speaker slot if it does not exist yet (no-op otherwise).
    async fn ensure_speaker(
        conn: &mut SqliteConnection,
        meeting_id: &str,
        speaker_key: &str,
    ) -> Result<(), PeopleError> {
        check_speaker_key(speaker_key)?;
        sqlx::query(
            "INSERT OR IGNORE INTO meeting_speakers (id, meeting_id, speaker_key, person_id, created_at)
             VALUES (?, ?, ?, NULL, ?)",
        )
        .bind(format!("speaker-{}", Uuid::new_v4()))
        .bind(meeting_id)
        .bind(speaker_key)
        .bind(Utc::now().to_rfc3339())
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Add the next free speaker slot ("S3" after "S1", "S2") and return the updated list.
    pub async fn add_speaker(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        let mut tx = pool.begin().await?;
        Self::require_meeting(&mut tx, meeting_id).await?;

        let keys: Vec<(String,)> =
            sqlx::query_as("SELECT speaker_key FROM meeting_speakers WHERE meeting_id = ?")
                .bind(meeting_id)
                .fetch_all(&mut *tx)
                .await?;
        let next = keys
            .iter()
            .filter_map(|(k,)| speaker_number(k))
            .max()
            .unwrap_or(0)
            + 1;
        Self::ensure_speaker(&mut tx, meeting_id, &format!("S{}", next)).await?;

        let speakers = Self::fetch_speakers(&mut tx, meeting_id).await?;
        tx.commit().await?;
        Ok(speakers)
    }

    /// Link a speaker slot to a person, or clear the link with `None` (back to "Speaker N").
    /// Every line spoken by this slot picks up the change because lines only store the slot key.
    pub async fn set_speaker_person(
        pool: &SqlitePool,
        meeting_id: &str,
        speaker_key: &str,
        person_id: Option<&str>,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        let mut tx = pool.begin().await?;
        Self::require_meeting(&mut tx, meeting_id).await?;
        check_speaker_key(speaker_key)?;

        if let Some(pid) = person_id {
            if Self::find_person_by_id(&mut tx, pid).await?.is_none() {
                return invalid("Person not found");
            }
        }

        Self::ensure_speaker(&mut tx, meeting_id, speaker_key).await?;
        sqlx::query("UPDATE meeting_speakers SET person_id = ? WHERE meeting_id = ? AND speaker_key = ?")
            .bind(person_id)
            .bind(meeting_id)
            .bind(speaker_key)
            .execute(&mut *tx)
            .await?;

        let speakers = Self::fetch_speakers(&mut tx, meeting_id).await?;
        tx.commit().await?;
        Ok(speakers)
    }

    /// Reassign one transcript line to a speaker slot, or `None` to leave it unlabeled.
    /// Returns the meeting's refreshed speaker list (line counts change).
    pub async fn set_transcript_speaker(
        pool: &SqlitePool,
        transcript_id: &str,
        speaker_key: Option<&str>,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        let mut tx = pool.begin().await?;

        let meeting: Option<(String,)> =
            sqlx::query_as("SELECT meeting_id FROM transcripts WHERE id = ?")
                .bind(transcript_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((meeting_id,)) = meeting else {
            return invalid("Transcript line not found");
        };

        if let Some(key) = speaker_key {
            Self::ensure_speaker(&mut tx, &meeting_id, key).await?;
        }
        sqlx::query("UPDATE transcripts SET speaker = ? WHERE id = ?")
            .bind(speaker_key)
            .bind(transcript_id)
            .execute(&mut *tx)
            .await?;

        let speakers = Self::fetch_speakers(&mut tx, &meeting_id).await?;
        tx.commit().await?;
        Ok(speakers)
    }

    /// Fold `from_key` into `into_key`: all its lines move over and the slot is removed.
    /// Used when automatic detection split one real person into two speakers.
    pub async fn merge_speakers(
        pool: &SqlitePool,
        meeting_id: &str,
        from_key: &str,
        into_key: &str,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        check_speaker_key(from_key)?;
        check_speaker_key(into_key)?;
        if from_key == into_key {
            return invalid("Choose two different speakers to merge");
        }

        let mut tx = pool.begin().await?;
        Self::require_meeting(&mut tx, meeting_id).await?;

        let from_person: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT person_id FROM meeting_speakers WHERE meeting_id = ? AND speaker_key = ?",
        )
        .bind(meeting_id)
        .bind(from_key)
        .fetch_optional(&mut *tx)
        .await?;

        Self::ensure_speaker(&mut tx, meeting_id, into_key).await?;
        sqlx::query("UPDATE transcripts SET speaker = ? WHERE meeting_id = ? AND speaker = ?")
            .bind(into_key)
            .bind(meeting_id)
            .bind(from_key)
            .execute(&mut *tx)
            .await?;

        // Keep the name if only the slot being folded away had one.
        if let Some((Some(person_id),)) = from_person {
            sqlx::query(
                "UPDATE meeting_speakers SET person_id = ?
                 WHERE meeting_id = ? AND speaker_key = ? AND person_id IS NULL",
            )
            .bind(person_id)
            .bind(meeting_id)
            .bind(into_key)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("DELETE FROM meeting_speakers WHERE meeting_id = ? AND speaker_key = ?")
            .bind(meeting_id)
            .bind(from_key)
            .execute(&mut *tx)
            .await?;

        let speakers = Self::fetch_speakers(&mut tx, meeting_id).await?;
        tx.commit().await?;
        Ok(speakers)
    }

    /// Remove a speaker slot; its lines become unlabeled.
    pub async fn delete_speaker(
        pool: &SqlitePool,
        meeting_id: &str,
        speaker_key: &str,
    ) -> Result<Vec<MeetingSpeaker>, PeopleError> {
        check_speaker_key(speaker_key)?;
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE transcripts SET speaker = NULL WHERE meeting_id = ? AND speaker = ?")
            .bind(meeting_id)
            .bind(speaker_key)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM meeting_speakers WHERE meeting_id = ? AND speaker_key = ?")
            .bind(meeting_id)
            .bind(speaker_key)
            .execute(&mut *tx)
            .await?;
        let speakers = Self::fetch_speakers(&mut tx, meeting_id).await?;
        tx.commit().await?;
        Ok(speakers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// In-memory database with the real migration chain applied. A single connection
    /// keeps every query on the same in-memory database.
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn add_meeting(pool: &SqlitePool, id: &str) {
        let now = Utc::now();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(format!("Meeting {}", id))
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn add_line(pool: &SqlitePool, id: &str, meeting_id: &str, start: f64, speaker: Option<&str>) {
        sqlx::query(
            "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time, speaker)
             VALUES (?, ?, 'hello', '10:00:00', ?, ?)",
        )
        .bind(id)
        .bind(meeting_id)
        .bind(start)
        .bind(speaker)
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn speaker_keys_have_one_spelling() {
        assert_eq!(speaker_number("S1"), Some(1));
        assert_eq!(speaker_number("S12"), Some(12));
        for bad in ["S0", "S01", "s1", "S", "1", "S-1", "S1 ", "Speaker 1", ""] {
            assert_eq!(speaker_number(bad), None, "{bad:?} should be rejected");
        }
    }

    #[test]
    fn names_are_cleaned_and_validated() {
        assert_eq!(normalize_name("  Maj   Ana  Lopez ").unwrap(), "Maj Ana Lopez");
        assert!(normalize_name("   ").is_err());
        assert!(normalize_name(&"x".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[tokio::test]
    async fn typing_a_name_once_creates_one_person_reused_case_insensitively() {
        let pool = test_pool().await;
        let first = PeopleRepository::get_or_create_person(&pool, "Maj Smith").await.unwrap();
        let again = PeopleRepository::get_or_create_person(&pool, "  maj   smith ").await.unwrap();
        assert_eq!(first.id, again.id);
        assert_eq!(PeopleRepository::list_people(&pool, None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn renaming_a_speaker_mirrors_to_every_line() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        add_line(&pool, "t1", "m1", 1.0, Some("S1")).await;
        add_line(&pool, "t2", "m1", 5.0, Some("S2")).await;
        add_line(&pool, "t3", "m1", 9.0, Some("S1")).await;
        // Slots exist for the lines above (diarization will create them later; here by hand)
        PeopleRepository::set_transcript_speaker(&pool, "t1", Some("S1")).await.unwrap();
        PeopleRepository::set_transcript_speaker(&pool, "t2", Some("S2")).await.unwrap();

        let person = PeopleRepository::get_or_create_person(&pool, "Col Reyes").await.unwrap();
        let speakers = PeopleRepository::set_speaker_person(&pool, "m1", "S1", Some(&person.id))
            .await
            .unwrap();

        let s1 = speakers.iter().find(|s| s.speaker_key == "S1").unwrap();
        assert_eq!(s1.person_name.as_deref(), Some("Col Reyes"));
        assert_eq!(s1.segment_count, 2, "both S1 lines resolve to the one slot");
        let s2 = speakers.iter().find(|s| s.speaker_key == "S2").unwrap();
        assert_eq!(s2.person_name, None);

        // Renaming the person itself also flows through, still one row to update.
        PeopleRepository::rename_person(&pool, &person.id, "Col Reyes Jr").await.unwrap();
        let speakers = PeopleRepository::get_meeting_speakers(&pool, "m1").await.unwrap();
        assert_eq!(
            speakers.iter().find(|s| s.speaker_key == "S1").unwrap().person_name.as_deref(),
            Some("Col Reyes Jr")
        );
    }

    #[tokio::test]
    async fn rename_rejects_a_name_that_belongs_to_someone_else() {
        let pool = test_pool().await;
        let a = PeopleRepository::get_or_create_person(&pool, "Ana").await.unwrap();
        PeopleRepository::get_or_create_person(&pool, "Ben").await.unwrap();
        assert!(PeopleRepository::rename_person(&pool, &a.id, "ben").await.is_err());
        // Changing only the case of your own name is fine.
        assert_eq!(PeopleRepository::rename_person(&pool, &a.id, "ANA").await.unwrap().name, "ANA");
    }

    #[tokio::test]
    async fn add_speaker_picks_the_next_free_number() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        PeopleRepository::add_speaker(&pool, "m1").await.unwrap();
        let speakers = PeopleRepository::add_speaker(&pool, "m1").await.unwrap();
        let keys: Vec<_> = speakers.iter().map(|s| s.speaker_key.as_str()).collect();
        assert_eq!(keys, ["S1", "S2"]);
        assert!(PeopleRepository::add_speaker(&pool, "nope").await.is_err());
    }

    #[tokio::test]
    async fn merging_moves_lines_and_keeps_the_name() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        add_line(&pool, "t1", "m1", 1.0, Some("S1")).await;
        add_line(&pool, "t2", "m1", 2.0, Some("S2")).await;
        PeopleRepository::set_transcript_speaker(&pool, "t1", Some("S1")).await.unwrap();
        PeopleRepository::set_transcript_speaker(&pool, "t2", Some("S2")).await.unwrap();
        let person = PeopleRepository::get_or_create_person(&pool, "Lt Kim").await.unwrap();
        PeopleRepository::set_speaker_person(&pool, "m1", "S2", Some(&person.id)).await.unwrap();

        let speakers = PeopleRepository::merge_speakers(&pool, "m1", "S2", "S1").await.unwrap();
        assert_eq!(speakers.len(), 1);
        assert_eq!(speakers[0].speaker_key, "S1");
        assert_eq!(speakers[0].segment_count, 2);
        assert_eq!(speakers[0].person_name.as_deref(), Some("Lt Kim"));
        assert!(PeopleRepository::merge_speakers(&pool, "m1", "S1", "S1").await.is_err());
    }

    #[tokio::test]
    async fn deleting_a_person_unlinks_but_keeps_the_speaker_slot() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        let person = PeopleRepository::get_or_create_person(&pool, "Capt Osei").await.unwrap();
        PeopleRepository::set_speaker_person(&pool, "m1", "S1", Some(&person.id)).await.unwrap();

        assert!(PeopleRepository::delete_person(&pool, &person.id).await.unwrap());
        let speakers = PeopleRepository::get_meeting_speakers(&pool, "m1").await.unwrap();
        assert_eq!(speakers.len(), 1);
        assert_eq!(speakers[0].person_id, None);
    }

    #[tokio::test]
    async fn deleting_a_meeting_removes_its_speaker_slots() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        PeopleRepository::add_speaker(&pool, "m1").await.unwrap();
        sqlx::query("DELETE FROM meetings WHERE id = 'm1'").execute(&pool).await.unwrap();
        let left: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meeting_speakers")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left.0, 0);
    }

    #[tokio::test]
    async fn people_list_counts_meetings_and_filters_by_name() {
        let pool = test_pool().await;
        add_meeting(&pool, "m1").await;
        add_meeting(&pool, "m2").await;
        let ana = PeopleRepository::get_or_create_person(&pool, "Ana Silva").await.unwrap();
        PeopleRepository::get_or_create_person(&pool, "Ben_100%").await.unwrap();
        PeopleRepository::set_speaker_person(&pool, "m1", "S1", Some(&ana.id)).await.unwrap();
        PeopleRepository::set_speaker_person(&pool, "m2", "S3", Some(&ana.id)).await.unwrap();

        let all = PeopleRepository::list_people(&pool, None).await.unwrap();
        let ana_row = all.iter().find(|p| p.name == "Ana Silva").unwrap();
        assert_eq!(ana_row.meeting_count, 2);
        assert!(ana_row.last_meeting_at.is_some());

        assert_eq!(PeopleRepository::list_people(&pool, Some("silv")).await.unwrap().len(), 1);
        // Wildcards in the query are literal, not patterns.
        assert_eq!(PeopleRepository::list_people(&pool, Some("_100%")).await.unwrap().len(), 1);
        assert_eq!(PeopleRepository::list_people(&pool, Some("%")).await.unwrap().len(), 1);
    }
}
