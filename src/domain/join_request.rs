use chrono::{DateTime, Utc};
use std::fmt;

/// Status of a join request through its lifecycle.
///
/// Valid transitions:
/// - `PendingContact` → `QuestionnaireInProgress`
/// - `QuestionnaireInProgress` → `Submitted`
/// - `Submitted` → `Approved` | `Rejected` | `Banned`
/// - Any non-terminal → `Expired` | `Cancelled`
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum JoinRequestStatus {
    PendingContact,
    QuestionnaireInProgress,
    Submitted,
    Approved,
    Rejected,
    Banned,
    Expired,
    Cancelled,
}

impl JoinRequestStatus {
    /// Returns `true` if this status is terminal (no further transitions allowed except expiry/cancel).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Approved | Self::Rejected | Self::Banned | Self::Expired | Self::Cancelled
        )
    }

    /// Validates whether a transition from this status to the target is allowed.
    pub fn can_transition_to(&self, target: &JoinRequestStatus) -> bool {
        match (self, target) {
            (Self::PendingContact, Self::QuestionnaireInProgress) => true,
            (Self::QuestionnaireInProgress, Self::Submitted) => true,
            (Self::Submitted, Self::Approved | Self::Rejected | Self::Banned) => true,
            // Any active (non-terminal) status can transition to expired or cancelled
            (_, Self::Expired | Self::Cancelled) if !self.is_terminal() => true,
            _ => false,
        }
    }
}

impl fmt::Display for JoinRequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingContact => write!(f, "pending_contact"),
            Self::QuestionnaireInProgress => write!(f, "questionnaire_in_progress"),
            Self::Submitted => write!(f, "submitted"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Banned => write!(f, "banned"),
            Self::Expired => write!(f, "expired"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A join request from an applicant to a community.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct JoinRequest {
    pub id: i64,
    pub community_id: i64,
    pub applicant_id: i64,
    pub telegram_user_chat_id: i64,
    pub status: JoinRequestStatus,
    pub telegram_join_request_date: DateTime<Utc>,
    pub submitted_to_moderators_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub moderator_message_chat_id: Option<i64>,
    pub moderator_message_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reminder_sent_at: Option<DateTime<Utc>>,
}
