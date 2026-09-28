use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::infrai_client::{InfraiClient, InfraiError};

const MAX_AVATAR_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct AvatarState {
    pub infrai: InfraiClient,
}

#[derive(Debug, Deserialize)]
pub struct AvatarCommand {
    user_id: String,
    appointment_id: String,
    appointment_state: AppointmentState,
    request_id: String,
    filename: String,
    avatar: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AppointmentState {
    Scheduled,
    Completed,
    Cancelled,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationalNotice {
    ProfileUpdated { message: &'static str },
    None,
}

#[derive(Debug, Serialize)]
pub struct AvatarResult {
    user_id: String,
    appointment_id: String,
    avatar: Value,
    notice: OperationalNotice,
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("missing multipart field: {0}")]
    MissingField(&'static str),
    #[error("invalid multipart request: {0}")]
    Multipart(String),
    #[error("avatar must be a non-empty JPEG, PNG, or WebP under 8 MiB")]
    InvalidAvatar,
    #[error("invalid appointment state")]
    InvalidAppointmentState,
    #[error(transparent)]
    Infrai(#[from] InfraiError),
}

pub fn router(state: AvatarState) -> Router {
    Router::new()
        .route("/appointment-avatar", post(process_avatar))
        .layer(DefaultBodyLimit::max(MAX_AVATAR_BYTES + 64 * 1024))
        .with_state(state)
}

async fn process_avatar(
    State(state): State<AvatarState>,
    Json(command): Json<AvatarCommand>,
) -> Result<(StatusCode, Json<AvatarResult>), ServiceError> {
    validate_text(&command.user_id, "user_id")?;
    validate_text(&command.appointment_id, "appointment_id")?;
    validate_text(&command.request_id, "request_id")?;
    validate_avatar(&command.filename, &command.avatar)?;

    let uploaded = state
        .infrai
        .upload(
            command.avatar,
            command.filename,
            &format!("{}:upload", command.request_id),
        )
        .await?;
    let cropped = state
        .infrai
        .smart_crop(&uploaded, &format!("{}:crop", command.request_id))
        .await?;
    let optimized = state
        .infrai
        .optimize(&cropped, &format!("{}:optimize", command.request_id))
        .await?;
    state
        .infrai
        .update_user_avatar(
            &command.user_id,
            &optimized,
            &format!("{}:user", command.request_id),
        )
        .await?;

    let notice = notification_for(command.appointment_state);
    Ok((
        StatusCode::ACCEPTED,
        Json(AvatarResult {
            user_id: command.user_id,
            appointment_id: command.appointment_id,
            avatar: optimized,
            notice,
        }),
    ))
}

pub fn notification_for(state: AppointmentState) -> OperationalNotice {
    match state {
        AppointmentState::Scheduled => OperationalNotice::ProfileUpdated {
            message: "Your profile photo was updated for your upcoming appointment.",
        },
        AppointmentState::Completed | AppointmentState::Cancelled => OperationalNotice::None,
    }
}

fn validate_avatar(filename: &str, bytes: &[u8]) -> Result<(), ServiceError> {
    let lower = filename.to_ascii_lowercase();
    let valid_extension = [".jpg", ".jpeg", ".png", ".webp"]
        .iter()
        .any(|extension| lower.ends_with(extension));
    if !valid_extension || bytes.is_empty() || bytes.len() > MAX_AVATAR_BYTES {
        return Err(ServiceError::InvalidAvatar);
    }
    Ok(())
}

fn validate_text(value: &str, name: &'static str) -> Result<(), ServiceError> {
    if value.trim().is_empty() {
        return Err(ServiceError::MissingField(name));
    }
    Ok(())
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let status = match &self {
            ServiceError::MissingField(_)
            | ServiceError::Multipart(_)
            | ServiceError::InvalidAvatar
            | ServiceError::InvalidAppointmentState => StatusCode::BAD_REQUEST,
            ServiceError::Infrai(InfraiError::Rejected { status, .. }) => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST)
            }
            ServiceError::Infrai(_) => StatusCode::BAD_GATEWAY,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{notification_for, AppointmentState, OperationalNotice};

    #[test]
    fn only_scheduled_appointments_receive_an_operational_notice() {
        assert!(matches!(
            notification_for(AppointmentState::Scheduled),
            OperationalNotice::ProfileUpdated { .. }
        ));
        assert_eq!(
            notification_for(AppointmentState::Cancelled),
            OperationalNotice::None
        );
        assert_eq!(
            notification_for(AppointmentState::Completed),
            OperationalNotice::None
        );
    }
}
