//! Standalone HTTP client for the Tado API (subset of GET endpoints).
//!
//! - Blocking client using `ureq` (no async).
//! - Uses existing models in `crate::models::tado`.
//! - Covers all GET endpoints in `tado-openapi.yml` except:
//!   - Only includes the "get overlay" method from the "zone control" section
//!   - Skips GET endpoints that return a single entity also available via a list endpoint
//!   - Skips all endpoints under invitations
//!
//! Authentication
//! - Uses a browser-derived OAuth2 refresh token and rotates it in-memory.
//! - Mimics browser headers for both token refresh and API requests.

use crate::models::tado::*;
use chrono::NaiveDate;
use log::{debug, error, info, warn};
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const BASE_URL: &str = "https://my.tado.com/api/v2";
// Matches the browser refresh endpoint observed in the app
const OAUTH_TOKEN_URL: &str = "https://login.tado.com/oauth2/token?ngsw-bypass=true";
// Public browser client id used by app.tado.com
const OAUTH_CLIENT_ID: &str = "af44f89e-ae86-4ebe-905f-6bf759cf6473";

const JSON_BODY_MAX: u64 = 10 * 1024 * 1024;
type HttpResponse = http::Response<ureq::Body>;

#[derive(Debug)]
pub enum TadoClientError {
    MissingAuth,
    Transport(String),
    Http { status: u16, message: String },
    Json(serde_json::Error),
    Auth(String),
}

impl core::fmt::Display for TadoClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TadoClientError::MissingAuth => write!(f, "missing bearer token for authenticated endpoint"),
            TadoClientError::Transport(s) => write!(f, "transport error: {}", s),
            TadoClientError::Http { status, message } => write!(f, "http {}: {}", status, message),
            TadoClientError::Json(e) => write!(f, "json error: {}", e),
            TadoClientError::Auth(e) => write!(f, "auth error: {}", e),
        }
    }
}

impl std::error::Error for TadoClientError {}

impl From<serde_json::Error> for TadoClientError {
    fn from(value: serde_json::Error) -> Self {
        TadoClientError::Json(value)
    }
}

#[derive(Debug, Clone)]
struct AccessToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Debug)]
struct OAuthState {
    token: Option<AccessToken>,
    refresh_token: String,
}

pub struct TadoClient {
    agent: ureq::Agent,
    oauth: RefCell<OAuthState>,
    firefox_version: String,
    refresh_token_path: PathBuf,
}

impl TadoClient {
    fn browser_headers(&self) -> Vec<(&'static str, String)> {
        let ver = &self.firefox_version;
        let ua = format!(
            "Mozilla/5.0 (X11; Linux x86_64; rv:{v}) Gecko/20100101 Firefox/{v}",
            v = ver
        );
        vec![
            ("User-Agent", ua),
            ("Accept", "application/json, text/plain, */*".to_string()),
            ("Accept-Language", "en-US,en;q=0.5".to_string()),
            // Only advertise encodings that the client can transparently decode.
            ("Accept-Encoding", "gzip, deflate".to_string()),
            ("Referer", "https://app.tado.com/".to_string()),
            ("Origin", "https://app.tado.com".to_string()),
            ("DNT", "1".to_string()),
            ("Sec-GPC", "1".to_string()),
            ("Sec-Fetch-Dest", "empty".to_string()),
            ("Sec-Fetch-Mode", "cors".to_string()),
            ("Sec-Fetch-Site", "same-site".to_string()),
            ("Connection", "keep-alive".to_string()),
            ("Pragma", "no-cache".to_string()),
            ("Cache-Control", "no-cache".to_string()),
        ]
    }
    pub fn new(
        initial_refresh_token: impl Into<String>,
        firefox_version: impl Into<String>,
        refresh_token_path: impl Into<PathBuf>,
    ) -> Result<Self, TadoClientError> {
        let agent = ureq::agent();

        let client = TadoClient {
            agent,
            oauth: RefCell::new(OAuthState {
                token: None,
                refresh_token: initial_refresh_token.into(),
            }),
            firefox_version: firefox_version.into(),
            refresh_token_path: refresh_token_path.into(),
        };

        // Fetch initial access token using the provided refresh token
        let _ = client.get_bearer()?;
        info!("Tado OAuth: initial access token acquired via refresh grant");

        Ok(client)
    }

    fn url(path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", BASE_URL, path)
        } else {
            format!("{}/{}", BASE_URL, path)
        }
    }

    fn oauth_refresh_grant(&self, refresh: &str) -> Result<(AccessToken, Option<String>), TadoClientError> {
        let _ = refresh; // never log refresh token
        info!("Tado OAuth: refreshing access token (browser flow)");
        let mut req = self.agent.post(OAUTH_TOKEN_URL);
        for (k, v) in self.browser_headers() {
            req = req.header(k, &v);
        }
        let resp = req
            .header("Content-Type", "application/x-www-form-urlencoded")
            .config()
            .http_status_as_error(false)
            .build()
            .send_form([
                ("client_id", OAUTH_CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
            ]);
        Self::parse_token_response(resp)
    }

    fn persist_refresh_token(&self, token: &str) {
        // Best-effort write; never log the token value.
        if let Some(parent) = self.refresh_token_path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!(
                        "Tado OAuth: failed to create directory {} for refresh token persistence: {}",
                        parent.display(),
                        e
                    );
                    return;
                }
            }
        }

        if let Err(e) = std::fs::write(&self.refresh_token_path, token) {
            warn!(
                "Tado OAuth: failed to persist rotated refresh token to {}: {}",
                self.refresh_token_path.display(),
                e
            );
        } else {
            info!(
                "Tado OAuth: rotated refresh token persisted to {}",
                self.refresh_token_path.display()
            );
        }
    }

    fn parse_token_response(
        resp: Result<HttpResponse, ureq::Error>,
    ) -> Result<(AccessToken, Option<String>), TadoClientError> {
        #[derive(serde::Deserialize)]
        struct R {
            access_token: String,
            expires_in: u64,
            #[serde(default)]
            refresh_token: Option<String>,
        }
        match resp {
            Ok(mut r) => {
                if r.status().is_success() {
                    let R {
                        access_token,
                        expires_in,
                        refresh_token,
                    } = read_json_body::<R>(&mut r, OAUTH_TOKEN_URL)?;
                    let expires_at = Instant::now() + Duration::from_secs(expires_in);
                    let tok = AccessToken {
                        access_token,
                        expires_at,
                    };
                    debug!("Tado OAuth: token parsed; expires_in_secs ~{}", expires_in);
                    Ok((tok, refresh_token))
                } else {
                    let status = r.status();
                    let body = read_body_text(&mut r);
                    Err(TadoClientError::Auth(format!("http {}: {}", status, body)))
                }
            }
            Err(e) => Err(TadoClientError::Transport(e.to_string())),
        }
    }

    fn get_bearer(&self) -> Result<String, TadoClientError> {
        let mut s = self.oauth.borrow_mut();
        let needs_refresh = match &s.token {
            None => true,
            Some(t) => Instant::now() + Duration::from_secs(30) >= t.expires_at,
        };
        if needs_refresh {
            info!("Tado OAuth: access token missing/expired; using refresh grant");
            let (new_access, new_refresh) = self.oauth_refresh_grant(&s.refresh_token)?;
            if let Some(r) = new_refresh {
                s.refresh_token = r;
                // Persist the rotated refresh token for future runs.
                self.persist_refresh_token(&s.refresh_token);
            }
            s.token = Some(new_access);
        }
        Ok(s.token.as_ref().unwrap().access_token.clone())
    }

    fn call_get(&self, url: &str, query: &[(&str, String)], bearer: &str) -> Result<HttpResponse, ureq::Error> {
        let mut req = self.agent.get(url);
        for (k, v) in self.browser_headers() {
            req = req.header(k, &v);
        }
        for (k, v) in query {
            req = req.query(k, v);
        }
        req = req.header("Authorization", &format!("Bearer {}", bearer));
        req.config().http_status_as_error(false).build().call()
    }

    fn retry_after_refresh<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<T, TadoClientError> {
        {
            let mut s = self.oauth.borrow_mut();
            let (new_access, new_refresh) = self.oauth_refresh_grant(&s.refresh_token)?;
            if let Some(r) = new_refresh {
                s.refresh_token = r;
                // Persist the rotated refresh token for future runs.
                self.persist_refresh_token(&s.refresh_token);
            }
            s.token = Some(new_access);
        }
        let token2 = self.get_bearer()?;
        // Log the retried request at info level so non-auth calls are visible
        info!("Tado API GET {}{} [after refresh]", url, format_query_params(query));
        match self.call_get(url, query, &token2) {
            Ok(mut res2) if res2.status().is_success() => read_json_body::<T>(&mut res2, url),
            Ok(mut res2) => {
                let status = res2.status().as_u16();
                let msg = read_body_text(&mut res2);
                Err(TadoClientError::Http { status, message: msg })
            }
            Err(e) => Err(TadoClientError::Transport(e.to_string())),
        }
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T, TadoClientError> {
        let url = Self::url(path);
        let token = self.get_bearer()?;
        // Log every non-auth endpoint call at info level
        info!("Tado API GET {}{}", path, format_query_params(query));
        match self.call_get(&url, query, &token) {
            Ok(res) if res.status().as_u16() == 401 => self.retry_after_refresh::<T>(&url, query),
            Ok(mut res) if res.status().is_success() => read_json_body::<T>(&mut res, path),
            Ok(mut res) => {
                let status = res.status().as_u16();
                let msg = read_body_text(&mut res);
                Err(TadoClientError::Http { status, message: msg })
            }
            Err(e) => Err(TadoClientError::Transport(e.to_string())),
        }
    }

    pub fn get_me(&self) -> Result<User, TadoClientError> {
        self.get_json("/me", &[])
    }

    pub fn get_users(&self, home_id: HomeId) -> Result<Vec<User>, TadoClientError> {
        self.get_json(&format!("/homes/{}/users", home_id.0), &[])
    }

    pub fn get_home(&self, home_id: HomeId) -> Result<Home, TadoClientError> {
        self.get_json(&format!("/homes/{}", home_id.0), &[])
    }

    pub fn get_air_comfort(&self, home_id: HomeId) -> Result<AirComfort, TadoClientError> {
        self.get_json(&format!("/homes/{}/airComfort", home_id.0), &[])
    }

    pub fn get_heating_circuits(&self, home_id: HomeId) -> Result<Vec<HeatingCircuit>, TadoClientError> {
        self.get_json(&format!("/homes/{}/heatingCircuits", home_id.0), &[])
    }

    pub fn get_heating_system(&self, home_id: HomeId) -> Result<HeatingSystem, TadoClientError> {
        self.get_json(&format!("/homes/{}/heatingSystem", home_id.0), &[])
    }

    pub fn get_incident_detection(&self, home_id: HomeId) -> Result<IncidentDetection, TadoClientError> {
        self.get_json(&format!("/homes/{}/incidentDetection", home_id.0), &[])
    }

    pub fn get_flow_temperature_optimization(
        &self,
        home_id: HomeId,
    ) -> Result<FlowTemperatureOptimization, TadoClientError> {
        self.get_json(&format!("/homes/{}/flowTemperatureOptimization", home_id.0), &[])
    }

    pub fn get_weather(&self, home_id: HomeId) -> Result<Weather, TadoClientError> {
        self.get_json(&format!("/homes/{}/weather", home_id.0), &[])
    }

    pub fn get_home_state(&self, home_id: HomeId) -> Result<HomeState, TadoClientError> {
        self.get_json(&format!("/homes/{}/state", home_id.0), &[])
    }

    pub fn get_zones(&self, home_id: HomeId) -> Result<Vec<Zone>, TadoClientError> {
        self.get_json(&format!("/homes/{}/zones", home_id.0), &[])
    }

    pub fn get_zone_capabilities(&self, home_id: HomeId, zone_id: ZoneId) -> Result<ZoneCapabilities, TadoClientError> {
        self.get_json(&format!("/homes/{}/zones/{}/capabilities", home_id.0, zone_id.0), &[])
    }

    pub fn get_zone_state(&self, home_id: HomeId, zone_id: ZoneId) -> Result<ZoneState, TadoClientError> {
        self.get_json(&format!("/homes/{}/zones/{}/state", home_id.0, zone_id.0), &[])
    }

    pub fn get_zone_control(&self, home_id: HomeId, zone_id: ZoneId) -> Result<ZoneControl, TadoClientError> {
        self.get_json(&format!("/homes/{}/zones/{}/control", home_id.0, zone_id.0), &[])
    }

    pub fn get_zone_overlay(&self, home_id: HomeId, zone_id: ZoneId) -> Result<ZoneOverlay, TadoClientError> {
        self.get_json(&format!("/homes/{}/zones/{}/overlay", home_id.0, zone_id.0), &[])
    }

    pub fn get_devices(&self, home_id: HomeId) -> Result<Vec<Device>, TadoClientError> {
        self.get_json(&format!("/homes/{}/devices", home_id.0), &[])
    }

    pub fn get_device_list(&self, home_id: HomeId) -> Result<DeviceList, TadoClientError> {
        self.get_json(&format!("/homes/{}/deviceList", home_id.0), &[])
    }

    pub fn get_installations(&self, home_id: HomeId) -> Result<Vec<Installation>, TadoClientError> {
        self.get_json(&format!("/homes/{}/installations", home_id.0), &[])
    }

    pub fn get_temperature_offset(&self, device_id: DeviceId) -> Result<Temperature, TadoClientError> {
        self.get_json(&format!("/devices/{}/temperatureOffset", device_id.0), &[])
    }

    pub fn get_zone_day_report(
        &self,
        home_id: HomeId,
        zone_id: ZoneId,
        date: Option<NaiveDate>,
    ) -> Result<DayReport, TadoClientError> {
        let mut q = Vec::new();
        if let Some(d) = date {
            q.push(("date", d.format("%Y-%m-%d").to_string()));
        }
        self.get_json(&format!("/homes/{}/zones/{}/dayReport", home_id.0, zone_id.0), &q)
    }
}

fn format_query_params(query: &[(&str, String)]) -> String {
    if query.is_empty() {
        "".to_string()
    } else {
        let joined = query
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{}", joined)
    }
}

fn read_json_body<T: DeserializeOwned>(res: &mut HttpResponse, context: &str) -> Result<T, TadoClientError> {
    // Read the (potentially compressed) body with a hard size limit, then deserialize.
    // On failure, log detailed error with precise JSON path and full body (except for sensitive endpoints).
    use std::io::Read as _;

    let mut reader = res.body_mut().with_config().limit(JSON_BODY_MAX).reader();
    let mut buf = Vec::new();
    if let Err(e) = reader.read_to_end(&mut buf) {
        return Err(TadoClientError::Transport(format!("failed to read body: {}", e)));
    }

    // Use serde_path_to_error to capture the exact path where deserialization fails.
    let mut de = serde_json::Deserializer::from_slice(&buf);
    match serde_path_to_error::deserialize::<_, T>(&mut de) {
        Ok(v) => Ok(v),
        Err(err) => {
            let status = res.status().as_u16();
            let content_type = res
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<unknown>");
            let total_bytes = buf.len();
            let json_path = err.path().to_string();
            let inner = err.into_inner();

            // Avoid logging sensitive token bodies if context indicates OAuth/Token.
            let is_sensitive = {
                let lc = context.to_ascii_lowercase();
                lc.contains("oauth") || lc.contains("token") || lc.contains("login.tado.com")
            };
            if is_sensitive {
                error!(
                    "JSON deserialization failed (context={}, path={}, status={}, content-type={}, bytes={}): {}. Body: <redacted>",
                    context, json_path, status, content_type, total_bytes, inner
                );
            } else {
                let body_str = String::from_utf8_lossy(&buf);
                error!(
                    "JSON deserialization failed (context={}, path={}, status={}, content-type={}, bytes={}): {}. Body: {}",
                    context, json_path, status, content_type, total_bytes, inner, body_str
                );
            }

            Err(TadoClientError::Json(inner))
        }
    }
}

fn read_body_text(res: &mut HttpResponse) -> String {
    res.body_mut()
        .read_to_string()
        .unwrap_or_else(|_| String::from("<no body>"))
}
