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
//! - Performs OAuth2 password grant against Tado auth, manages refresh automatically.

use crate::models::tado::*;
use chrono::NaiveDate;
use log::{debug, info};
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::time::{Duration, Instant};

const BASE_URL: &str = "https://my.tado.com/api/v2";
const OAUTH_TOKEN_URL: &str = "https://auth.tado.com/oauth/token";
const OAUTH_CLIENT_ID: &str = "af44f89e-ae86-4ebe-905f-6bf759cf6473";
const OAUTH_CLIENT_SECRET: &str = "WzedWFdqrCqWD45EGCwgwXfdxtsAQGR4BfDsGrxwBcGG4tFebgg1gv3fGcFqGb4W";
const OAUTH_SCOPE: &str = "home.user";
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
struct OAuthToken {
    access_token: String,
    expires_at: Instant,
    refresh_token: Option<String>,
}

#[derive(Debug)]
struct OAuthState {
    token: Option<OAuthToken>,
    username: String,
    password: String,
}

pub struct TadoClient {
    agent: ureq::Agent,
    oauth: RefCell<OAuthState>,
}

impl TadoClient {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self, TadoClientError> {
        let agent = ureq::agent();

        let mut state = OAuthState {
            token: None,
            username: username.into(),
            password: password.into(),
        };

        // Fetch initial token
        let token = Self::oauth_password_grant(&agent, &state)?;
        state.token = Some(token);
        info!("Tado OAuth: initial token acquired");

        Ok(TadoClient {
            agent,
            oauth: RefCell::new(state),
        })
    }

    fn url(path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", BASE_URL, path)
        } else {
            format!("{}/{}", BASE_URL, path)
        }
    }

    fn oauth_password_grant(agent: &ureq::Agent, state: &OAuthState) -> Result<OAuthToken, TadoClientError> {
        debug!("Tado OAuth: password grant start");
        let resp = agent
            .post(OAUTH_TOKEN_URL)
            .header("Accept", "application/json")
            .config()
            .http_status_as_error(false)
            .build()
            .send_form([
                ("client_id", OAUTH_CLIENT_ID),
                ("client_secret", OAUTH_CLIENT_SECRET),
                ("grant_type", "password"),
                ("scope", OAUTH_SCOPE),
                ("username", state.username.as_str()),
                ("password", state.password.as_str()),
            ]);
        Self::parse_token_response(resp)
    }

    fn oauth_refresh_grant(
        agent: &ureq::Agent,
        _state: &OAuthState,
        refresh: &str,
    ) -> Result<OAuthToken, TadoClientError> {
        let _ = refresh; // never log refresh token
        info!("Tado OAuth: refreshing access token");
        let resp = agent
            .post(OAUTH_TOKEN_URL)
            .header("Accept", "application/json")
            .config()
            .http_status_as_error(false)
            .build()
            .send_form([
                ("client_id", OAUTH_CLIENT_ID),
                ("client_secret", OAUTH_CLIENT_SECRET),
                ("grant_type", "refresh_token"),
                ("scope", OAUTH_SCOPE),
                ("refresh_token", refresh),
            ]);
        Self::parse_token_response(resp)
    }

    fn parse_token_response(resp: Result<HttpResponse, ureq::Error>) -> Result<OAuthToken, TadoClientError> {
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
                    } = read_json_body::<R>(&mut r)?;
                    let expires_at = Instant::now() + Duration::from_secs(expires_in);
                    let tok = OAuthToken {
                        access_token,
                        expires_at,
                        refresh_token,
                    };
                    debug!("Tado OAuth: token parsed; expires_in_secs ~{}", expires_in);
                    Ok(tok)
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
            let new_tok = match &s.token.as_ref().and_then(|t| t.refresh_token.clone()) {
                Some(r) => {
                    info!("Tado OAuth: access token expired; using refresh grant");
                    Self::oauth_refresh_grant(&self.agent, &s, r)
                }
                None => {
                    info!("Tado OAuth: access token missing/expired; using password grant");
                    Self::oauth_password_grant(&self.agent, &s)
                }
            }?;
            s.token = Some(new_tok);
        }
        Ok(s.token.as_ref().unwrap().access_token.clone())
    }

    fn call_get(&self, url: &str, query: &[(&str, String)], bearer: &str) -> Result<HttpResponse, ureq::Error> {
        let mut req = self.agent.get(url).header("Accept", "application/json");
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
            let refreshed = match &s.token.as_ref().and_then(|t| t.refresh_token.clone()) {
                Some(r) => Self::oauth_refresh_grant(&self.agent, &s, r),
                None => Self::oauth_password_grant(&self.agent, &s),
            }?;
            s.token = Some(refreshed);
        }
        let token2 = self.get_bearer()?;
        match self.call_get(url, query, &token2) {
            Ok(mut res2) if res2.status().is_success() => read_json_body::<T>(&mut res2),
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
        debug!("GET {} ({} query params)", path, query.len());
        match self.call_get(&url, query, &token) {
            Ok(res) if res.status().as_u16() == 401 => self.retry_after_refresh::<T>(&url, query),
            Ok(mut res) if res.status().is_success() => read_json_body::<T>(&mut res),
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

fn read_json_body<T: DeserializeOwned>(res: &mut HttpResponse) -> Result<T, TadoClientError> {
    let reader = res.body_mut().with_config().limit(JSON_BODY_MAX).reader();
    serde_json::from_reader(reader).map_err(TadoClientError::Json)
}

fn read_body_text(res: &mut HttpResponse) -> String {
    res.body_mut()
        .read_to_string()
        .unwrap_or_else(|_| String::from("<no body>"))
}
