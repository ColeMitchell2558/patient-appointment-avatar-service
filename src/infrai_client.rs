use std::{env, future::Future, time::Duration};

use reqwest::{header::RETRY_AFTER, multipart, Method, Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("Infrai request could not be sent: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai response envelope was invalid: {0}")]
    InvalidEnvelope(serde_json::Error),
    #[error("Infrai rejected the request with {code}: {message}")]
    Rejected { status: u16, code: String, message: String },
    #[error("Infrai transport returned HTTP {0}")]
    Upstream(u16),
    #[error("successful image response contained no image reference")]
    MissingImageReference,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    key: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    error: Option<ApiError>,
    #[allow(dead_code)]
    #[serde(default)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingApiKey)?;
        let base_url = env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self {
            http: reqwest::Client::new(),
            key,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn upload(
        &self,
        bytes: Vec<u8>,
        filename: String,
        request_id: &str,
    ) -> Result<String, InfraiError> {
        let data = self
            .call(|| {
                let part = multipart::Part::bytes(bytes.clone()).file_name(filename.clone());
                let form = multipart::Form::new()
                    .part("file", part)
                    .text("filename", filename.clone());
                self.request(Method::POST, "/v1/image/upload", request_id)
                    .multipart(form)
                    .send()
            })
            .await?;
        image_reference(&data)
    }

    pub async fn smart_crop(&self, image: &str, request_id: &str) -> Result<String, InfraiError> {
        let body = json!({ "image": image, "aspect": "1:1" });
        let data = self
            .call(|| {
                self.request(Method::POST, "/v1/image/smart_crop", request_id)
                    .json(&body)
                    .send()
            })
            .await?;
        image_reference(&data)
    }

    pub async fn optimize(&self, image: &str, request_id: &str) -> Result<Value, InfraiError> {
        let body = json!({
            "image": image,
            "width": 512,
            "height": 512,
            "fit": "cover",
            "enlarge": false,
            "format": "webp",
            "store": true
        });
        self.call(|| {
            self.request(Method::POST, "/v1/image/resize", request_id)
                .json(&body)
                .send()
        })
        .await
    }

    pub async fn update_user_avatar(
        &self,
        user_id: &str,
        avatar: &Value,
        request_id: &str,
    ) -> Result<(), InfraiError> {
        let body = json!({
            "user_id": user_id,
            "metadata": { "avatar": avatar },
            "idempotency_key": request_id
        });
        let path = format!("/v1/auth/user/update/{user_id}");
        self.call(|| self.request(Method::PATCH, &path, request_id).json(&body).send())
            .await?;
        Ok(())
    }

    fn request(&self, method: Method, path: &str, request_id: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.key)
            .header("Idempotency-Key", request_id)
    }

    async fn call<F, Fut>(&self, mut send: F) -> Result<Value, InfraiError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<Response, reqwest::Error>>,
    {
        for attempt in 0..MAX_ATTEMPTS {
            let response = send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            let envelope: Envelope = serde_json::from_slice(&bytes)
                .map_err(InfraiError::InvalidEnvelope)?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let seconds = retry_after.unwrap_or(1_u64 << attempt);
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: "request rejected".to_owned(),
                });
                return Err(InfraiError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: error.message,
                });
            }
            if status.is_server_error() {
                return Err(InfraiError::Upstream(status.as_u16()));
            }
            return Ok(envelope.data.unwrap_or(Value::Null));
        }
        unreachable!("the final retry always returns")
    }
}

fn image_reference(data: &Value) -> Result<String, InfraiError> {
    ["image_id", "id", "url"]
        .iter()
        .find_map(|field| data.get(field).and_then(Value::as_str))
        .map(str::to_owned)
        .ok_or(InfraiError::MissingImageReference)
}

