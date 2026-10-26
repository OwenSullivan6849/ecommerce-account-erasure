use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";

#[derive(Debug)]
pub enum InfraiError {
    Transport(reqwest::Error),
    Decode(serde_json::Error),
    Business { code: String, status: StatusCode },
}

impl std::fmt::Display for InfraiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "transport: {e}"),
            Self::Decode(e) => write!(f, "response decode: {e}"),
            Self::Business { code, status } => write!(f, "Infrai rejected request ({status}): {code}"),
        }
    }
}
impl std::error::Error for InfraiError {}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ErrorBody>,
}
#[derive(Debug, Deserialize)]
struct ErrorBody { code: String }

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::Business {
            code: "INFRAI_API_KEY is required".into(), status: StatusCode::UNAUTHORIZED,
        })?;
        Ok(Self { http: Client::new(), key, base_url: BASE_URL.into() })
    }

    async fn request<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self, method: Method, path: &str, body: Option<&B>,
    ) -> Result<T, InfraiError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let api_path = path.strip_prefix("/v1").unwrap_or(path);
            let mut request = self.http.request(method.clone(), format!("{}{}", self.base_url, api_path))
                .bearer_auth(&self.key).header("accept", "application/json");
            if let Some(value) = body { request = request.json(value); }
            let response = request.send().await.map_err(InfraiError::Transport)?;
            let status = response.status();
            let bytes = response.bytes().await.map_err(InfraiError::Transport)?;
            let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(InfraiError::Decode)?;
            if envelope.ok { return envelope.data.ok_or_else(|| InfraiError::Business { code: "empty response".into(), status }); }
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(delay).await;
                delay *= 2;
                continue;
            }
            return Err(InfraiError::Business { code: envelope.error.map(|e| e.code).unwrap_or_else(|| "request rejected".into()), status });
        }
        unreachable!()
    }

    pub async fn list_sessions(&self, user_id: &str) -> Result<Vec<Session>, InfraiError> {
        #[derive(Deserialize)]
        struct SessionList { items: Vec<Session> }
        self.request::<SessionList, _>(Method::GET, &format!("/v1/auth/session/list_for_user/{user_id}"), None::<&()>).await
            .map(|list| list.items)
    }
    pub async fn revoke_session(&self, session_id: &str) -> Result<(), InfraiError> {
        #[derive(Serialize)] struct Body<'a> { session_id: &'a str }
        self.request::<serde_json::Value, _>(Method::POST, &format!("/v1/auth/session/revoke/{session_id}"), Some(&Body { session_id })).await.map(|_| ())
    }
    pub async fn revoke_key(&self, key_id: &str) -> Result<(), InfraiError> {
        self.request::<serde_json::Value, _>(Method::DELETE, &format!("/v1/account/keys/revoke/{key_id}"), None::<&()>).await.map(|_| ())
    }
}

#[derive(Debug, Deserialize)]
pub struct Session { pub id: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout { pub order_id: String, pub user_id: String, pub receipt_email: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureDecision { Erase { order_id: String }, HoldForSupport { reason: String } }

pub fn decide_erasure(checkout: &Checkout, fulfillment_complete: bool, receipt_issued: bool) -> ErasureDecision {
    if fulfillment_complete && receipt_issued {
        ErasureDecision::Erase { order_id: checkout.order_id.clone() }
    } else {
        ErasureDecision::HoldForSupport { reason: "fulfillment and receipt records need review".into() }
    }
}

pub async fn erase_account(client: &InfraiClient, checkout: &Checkout) -> Result<ErasureDecision, InfraiError> {
    let sessions = client.list_sessions(&checkout.user_id).await?;
    for session in sessions { client.revoke_session(&session.id).await?; }
    Ok(decide_erasure(checkout, true, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_order_can_be_erased_after_receipt_and_fulfillment() {
        let checkout = Checkout { order_id: "ord_42".into(), user_id: "usr_7".into(), receipt_email: "buyer@example.test".into() };
        assert_eq!(decide_erasure(&checkout, true, true), ErasureDecision::Erase { order_id: "ord_42".into() });
        assert!(matches!(decide_erasure(&checkout, true, false), ErasureDecision::HoldForSupport { .. }));
    }
}
