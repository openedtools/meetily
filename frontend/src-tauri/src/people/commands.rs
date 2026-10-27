use super::models::{AssignSpeakerResult, MeetingSpeaker, Person, PersonSummary};
use crate::database::repositories::people::{PeopleError, PeopleRepository};
use crate::state::AppState;
use log::{error as log_error, info as log_info};

fn ui_error(context: &str, e: PeopleError) -> String {
    if matches!(e, PeopleError::Db(_)) {
        log_error!("{}: {}", context, e);
    }
    e.to_string()
}

/// People directory, optionally filtered by a name fragment. Feeds the speaker dropdown.
#[tauri::command]
pub async fn api_list_people(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<PersonSummary>, String> {
    PeopleRepository::list_people(state.db_manager.pool(), query.as_deref())
        .await
        .map_err(|e| ui_error("api_list_people", e))
}

/// Create a person, or return the existing one with the same name (case-insensitive).
#[tauri::command]
pub async fn api_create_person(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Person, String> {
    PeopleRepository::get_or_create_person(state.db_manager.pool(), &name)
        .await
        .map_err(|e| ui_error("api_create_person", e))
}

#[tauri::command]
pub async fn api_rename_person(
    state: tauri::State<'_, AppState>,
    person_id: String,
    name: String,
) -> Result<Person, String> {
    PeopleRepository::rename_person(state.db_manager.pool(), &person_id, &name)
        .await
        .map_err(|e| ui_error("api_rename_person", e))
}

#[tauri::command]
pub async fn api_delete_person(
    state: tauri::State<'_, AppState>,
    person_id: String,
) -> Result<bool, String> {
    log_info!("api_delete_person called for {}", person_id);
    PeopleRepository::delete_person(state.db_manager.pool(), &person_id)
        .await
        .map_err(|e| ui_error("api_delete_person", e))
}

#[tauri::command]
pub async fn api_get_meeting_speakers(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<MeetingSpeaker>, String> {
    PeopleRepository::get_meeting_speakers(state.db_manager.pool(), &meeting_id)
        .await
        .map_err(|e| ui_error("api_get_meeting_speakers", e))
}

/// Add the next speaker slot ("Speaker 3") to a meeting.
#[tauri::command]
pub async fn api_add_meeting_speaker(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<MeetingSpeaker>, String> {
    PeopleRepository::add_speaker(state.db_manager.pool(), &meeting_id)
        .await
        .map_err(|e| ui_error("api_add_meeting_speaker", e))
}

/// Name a speaker. Pass `new_person_name` to create-or-reuse a person by name, or
/// `person_id` to pick someone from the dropdown. Passing neither clears the name.
/// All lines from that speaker follow, because they only store the slot key.
#[tauri::command]
pub async fn api_assign_speaker_person(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    speaker_key: String,
    person_id: Option<String>,
    new_person_name: Option<String>,
) -> Result<AssignSpeakerResult, String> {
    let pool = state.db_manager.pool();

    let typed_name = new_person_name.filter(|n| !n.trim().is_empty());
    let (person, person_id) = match typed_name {
        Some(name) => {
            let person = PeopleRepository::get_or_create_person(pool, &name)
                .await
                .map_err(|e| ui_error("api_assign_speaker_person", e))?;
            let id = person.id.clone();
            (Some(person), Some(id))
        }
        None => (None, person_id),
    };

    let speakers =
        PeopleRepository::set_speaker_person(pool, &meeting_id, &speaker_key, person_id.as_deref())
            .await
            .map_err(|e| ui_error("api_assign_speaker_person", e))?;

    Ok(AssignSpeakerResult { speakers, person })
}

/// Move one transcript line to another speaker (or `None` to unlabel it).
#[tauri::command]
pub async fn api_set_transcript_speaker(
    state: tauri::State<'_, AppState>,
    transcript_id: String,
    speaker_key: Option<String>,
) -> Result<Vec<MeetingSpeaker>, String> {
    PeopleRepository::set_transcript_speaker(
        state.db_manager.pool(),
        &transcript_id,
        speaker_key.as_deref(),
    )
    .await
    .map_err(|e| ui_error("api_set_transcript_speaker", e))
}

#[tauri::command]
pub async fn api_merge_meeting_speakers(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    from_key: String,
    into_key: String,
) -> Result<Vec<MeetingSpeaker>, String> {
    PeopleRepository::merge_speakers(state.db_manager.pool(), &meeting_id, &from_key, &into_key)
        .await
        .map_err(|e| ui_error("api_merge_meeting_speakers", e))
}

#[tauri::command]
pub async fn api_delete_meeting_speaker(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    speaker_key: String,
) -> Result<Vec<MeetingSpeaker>, String> {
    PeopleRepository::delete_speaker(state.db_manager.pool(), &meeting_id, &speaker_key)
        .await
        .map_err(|e| ui_error("api_delete_meeting_speaker", e))
}
