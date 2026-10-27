use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A named person in the global directory.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A person plus how often they appear, for the directory list and the speaker dropdown.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PersonSummary {
    pub id: String,
    pub name: String,
    pub meeting_count: i64,
    pub last_meeting_at: Option<String>,
}

/// A speaker slot in one meeting. `speaker_key` is "S1", "S2", ...; the UI shows
/// the linked person's name, or "Speaker N" when no one is linked yet.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct MeetingSpeaker {
    pub speaker_key: String,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub segment_count: i64,
}

/// Result of linking a speaker to a person. `person` is set when a person was
/// looked up or created, so the UI can add them to its dropdown without refetching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignSpeakerResult {
    pub speakers: Vec<MeetingSpeaker>,
    pub person: Option<Person>,
}
