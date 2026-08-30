use anyhow::Result;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// One durable, ordered user submission for a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedUserSubmissionRecord {
    pub id: String,
    pub thread_id: ThreadId,
    pub payload: String,
    pub state: QueuedUserSubmissionState,
}

/// Durable dispatch state for one queued user submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueuedUserSubmissionState {
    Pending,
    Claimed { turn_id: String },
}

impl QueuedUserSubmissionRecord {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        let claimed_turn_id = row.try_get::<Option<String>, _>("claimed_turn_id")?;
        Ok(Self {
            id: row.try_get("id")?,
            thread_id: ThreadId::try_from(row.try_get::<String, _>("thread_id")?)?,
            payload: row.try_get("payload_json")?,
            state: claimed_turn_id.map_or(QueuedUserSubmissionState::Pending, |turn_id| {
                QueuedUserSubmissionState::Claimed { turn_id }
            }),
        })
    }
}
