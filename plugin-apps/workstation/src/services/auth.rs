//! 工位登录与鉴权服务。
//! 负责本地模拟 token、SES station-login，以及请求头 bearer token 到 station_id 的解析。

use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::config::DEFAULT_CONNECT_STATION_ID;
use crate::models::{LoginRequest, bearer_token};

use super::AppState;

impl AppState {
    pub(crate) async fn login(&self, request: &LoginRequest) -> Result<String, String> {
        if self.config.ses_auth_base_url.is_some() {
            return self.ses_station_login(request).await;
        }

        let token = Uuid::new_v4().to_string();
        let mut state = self.inner.write().await;
        state.tokens.insert(token.clone(), request.station_id.clone());
        Ok(token)
    }

    async fn ses_station_login(&self, request: &LoginRequest) -> Result<String, String> {
        let auth_base_url = self
            .config
            .ses_auth_base_url
            .as_ref()
            .ok_or_else(|| "SES auth base URL is not configured".to_string())?;
        let username = request
            .username
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "username is required".to_string())?;
        let password = request
            .password
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "password is required".to_string())?;
        let payload = json!({
            "stationId": request.station_id,
            "platformId": request.platform_id,
            "login": username,
            "password": password
        });
        let response = self
            .client
            .post(format!("{auth_base_url}/station-login"))
            .json(&payload)
            .send()
            .await
            .map_err(|error| format!("failed to call SES station login: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("failed to read SES station login response: {error}"))?;
        if !status.is_success() {
            return Err(ses_error_message(&body).unwrap_or_else(|| format!("SES station login failed: {status}")));
        }
        let payload: SesAuthPayload = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse SES station login response: {error}"))?;
        {
            let mut state = self.inner.write().await;
            state
                .tokens
                .insert(payload.access_token.clone(), request.station_id.clone());
        }
        Ok(payload.access_token)
    }

    pub(crate) async fn authenticated_station_id(&self, headers: &HeaderMap) -> Result<String, String> {
        if self.config.ses_auth_base_url.is_some() {
            let token = bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
            return self.ses_authenticated_station_id(&token).await;
        }

        let state = self.inner.read().await;
        if let Some(token) = bearer_token(headers) {
            if let Some(station_id) = state.tokens.get(&token).cloned() {
                return Ok(station_id);
            }
        }

        let mut connected_workers = state
            .worker_streams
            .keys()
            .filter(|station_id| station_id.as_str() != DEFAULT_CONNECT_STATION_ID);
        let fallback_station_id = connected_workers.next().cloned();
        if fallback_station_id.is_some() && connected_workers.next().is_none() {
            let station_id = fallback_station_id.expect("fallback worker id should exist");
            warn!(station_id = %station_id, "模拟登录验证");
            return Ok(station_id);
        }

        if bearer_token(headers).is_some() {
            Err("invalid bearer token".to_string())
        } else {
            Err("missing bearer token".to_string())
        }
    }

    async fn ses_authenticated_station_id(&self, token: &str) -> Result<String, String> {
        let auth_base_url = self
            .config
            .ses_auth_base_url
            .as_ref()
            .ok_or_else(|| "SES auth base URL is not configured".to_string())?;
        let response = self
            .client
            .post(format!("{auth_base_url}/station-authorize"))
            .bearer_auth(token)
            .json(&json!({ "requiredPermission": "workstation.operate" }))
            .send()
            .await
            .map_err(|error| format!("failed to call SES station authorization: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("failed to read SES station authorization response: {error}"))?;
        if !status.is_success() {
            return Err(
                ses_error_message(&body).unwrap_or_else(|| format!("SES station authorization failed: {status}"))
            );
        }
        let payload: SesStationAuthorizeResponse = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse SES station authorization response: {error}"))?;
        {
            let mut state = self.inner.write().await;
            state.tokens.insert(token.to_string(), payload.station_id.clone());
        }
        Ok(payload.station_id)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SesAuthPayload {
    access_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SesStationAuthorizeResponse {
    station_id: String,
}

fn ses_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|payload| payload.get("error").and_then(Value::as_str).map(str::to_string))
        .filter(|message| !message.trim().is_empty())
}
