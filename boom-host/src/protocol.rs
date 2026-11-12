//! Request and response types exchanged with the browser extension.
//!
//! Each native messaging call is stateless: the extension sends one
//! [`Request`] and reads one [`Response`]. The host computes what to do
//! from the current state of the config and the git repo.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;

/// One management or sync operation requested by the extension.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "action", content = "params", rename_all = "snake_case")]
pub enum Action {
    /// Health check. Returns the host version.
    Ping,
    /// Returns the current config.
    ConfigShow,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Request {
    /// Caller-chosen id, echoed back in the response.
    pub id: String,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    fn success(id: String, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn failure(id: String, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

/// Runs one action and builds the response. Errors become `ok: false` responses.
pub fn handle(request: Request, config: &Config) -> Response {
    let id = request.id;
    match request.action {
        Action::Ping => Response::success(id, json!({ "version": env!("CARGO_PKG_VERSION") })),
        Action::ConfigShow => match serde_json::to_value(config) {
            Ok(value) => Response::success(id, value),
            Err(err) => Response::failure(id, err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ping_request() {
        let request: Request = serde_json::from_str(r#"{"id":"1","action":"ping"}"#).unwrap();
        assert_eq!(request.id, "1");
        assert_eq!(request.action, Action::Ping);
    }

    #[test]
    fn ping_returns_version() {
        let request = Request {
            id: "7".into(),
            action: Action::Ping,
        };
        let response = handle(request, &Config::default());
        assert!(response.ok);
        assert_eq!(response.id, "7");
        assert_eq!(
            response.result.unwrap()["version"],
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn unknown_action_fails_to_parse() {
        let parsed = serde_json::from_str::<Request>(r#"{"id":"1","action":"nope"}"#);
        assert!(parsed.is_err());
    }
}
