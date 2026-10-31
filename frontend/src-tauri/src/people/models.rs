use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A named person in the global directory.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    /// Free-text notes about the person (role, organization, contact, ...)
    pub notes: Option<String>,
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

/// One meeting a person took part in.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct PersonMeeting {
    pub meeting_id: String,
    pub title: String,
    pub created_at: String,
    pub folder_id: Option<String>,
    /// How many transcript lines they spoke in this meeting
    pub line_count: i64,
    /// Where their first line starts, in seconds from the recording start
    pub first_line_start: Option<f64>,
}

/// One thing a person said, with where it was said.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct PersonLine {
    pub transcript_id: String,
    pub meeting_id: String,
    pub meeting_title: String,
    pub meeting_created_at: String,
    pub text: String,
    pub audio_start_time: Option<f64>,
}

/// A person with every meeting they appear in, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonDetail {
    pub person: Person,
    pub meetings: Vec<PersonMeeting>,
}

/// A meeting found because a speaker's name matched a search.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct PersonMeetingHit {
    pub meeting_id: String,
    pub title: String,
    pub person_name: String,
}
