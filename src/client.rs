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

use chrono::NaiveDate;
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::time::{Duration, Instant};

use crate::models::tado::*;

const BASE_URL: &str = "https://my.tado.com/api/v2";
const OAUTH_TOKEN_URL: &str = "https://auth.tado.com/oauth/token";
const OAUTH_CLIENT_ID: &str = "af44f89e-ae86-4ebe-905f-6bf759cf6473";
const OAUTH_CLIENT_SECRET: &str = "WzedWFdqrCqWD45EGCwgwXfdxtsAQGR4BfDsGrxwBcGG4tFebgg1gv3fGcFqGb4W";
const OAUTH_SCOPE: &str = "home.user";

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
        let agent = ureq::AgentBuilder::new().build();

        let mut state = OAuthState {
            token: None,
            username: username.into(),
            password: password.into(),
        };

        // Fetch initial token
        let token = Self::oauth_password_grant(&agent, &state)?;
        state.token = Some(token);

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
        let resp = agent
            .post(OAUTH_TOKEN_URL)
            .set("Accept", "application/json")
            .send_form(&[
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
        let resp = agent
            .post(OAUTH_TOKEN_URL)
            .set("Accept", "application/json")
            .send_form(&[
                ("client_id", OAUTH_CLIENT_ID),
                ("client_secret", OAUTH_CLIENT_SECRET),
                ("grant_type", "refresh_token"),
                ("scope", OAUTH_SCOPE),
                ("refresh_token", refresh),
            ]);
        Self::parse_token_response(resp)
    }

    fn parse_token_response(resp: Result<ureq::Response, ureq::Error>) -> Result<OAuthToken, TadoClientError> {
        #[derive(serde::Deserialize)]
        struct R {
            access_token: String,
            expires_in: u64,
            #[serde(default)]
            refresh_token: Option<String>,
        }
        match resp {
            Ok(r) => {
                let R {
                    access_token,
                    expires_in,
                    refresh_token,
                } = serde_json::from_reader(r.into_reader()).map_err(TadoClientError::Json)?;
                let expires_at = Instant::now() + Duration::from_secs(expires_in);
                Ok(OAuthToken {
                    access_token,
                    expires_at,
                    refresh_token,
                })
            }
            Err(ureq::Error::Transport(t)) => Err(TadoClientError::Transport(t.to_string())),
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp.into_string().unwrap_or_else(|_| String::from("<no body>"));
                Err(TadoClientError::Auth(format!("http {}: {}", status, body)))
            }
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
                Some(r) => Self::oauth_refresh_grant(&self.agent, &s, r),
                None => Self::oauth_password_grant(&self.agent, &s),
            }?;
            s.token = Some(new_tok);
        }
        Ok(s.token.as_ref().unwrap().access_token.clone())
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T, TadoClientError> {
        // Build request
        let url = Self::url(path);
        let mut req = self.agent.get(&url).set("Accept", "application/json");
        for (k, v) in query {
            req = req.query(k, v);
        }

        // Add auth header
        let token = self.get_bearer()?;
        req = req.set("Authorization", &format!("Bearer {}", token));

        // Call, retry once on 401 after forcing refresh
        match req.call() {
            Ok(res) => serde_json::from_reader(res.into_reader()).map_err(TadoClientError::Json),
            Err(ureq::Error::Status(401, _)) => {
                // force refresh and retry
                {
                    let mut s = self.oauth.borrow_mut();
                    let refreshed = match &s.token.as_ref().and_then(|t| t.refresh_token.clone()) {
                        Some(r) => Self::oauth_refresh_grant(&self.agent, &s, r),
                        None => Self::oauth_password_grant(&self.agent, &s),
                    }?;
                    s.token = Some(refreshed);
                }
                let token2 = self.get_bearer()?;
                let mut req2 = self.agent.get(&url).set("Accept", "application/json");
                for (k, v) in query {
                    req2 = req2.query(k, v);
                }
                req2 = req2.set("Authorization", &format!("Bearer {}", token2));
                match req2.call() {
                    Ok(res2) => serde_json::from_reader(res2.into_reader()).map_err(TadoClientError::Json),
                    Err(ureq::Error::Transport(t)) => Err(TadoClientError::Transport(t.to_string())),
                    Err(ureq::Error::Status(status, res)) => {
                        let body = res.into_string().unwrap_or_else(|_| String::from("<no body>"));
                        Err(TadoClientError::Http { status, message: body })
                    }
                }
            }
            Err(ureq::Error::Transport(t)) => Err(TadoClientError::Transport(t.to_string())),
            Err(ureq::Error::Status(status, res)) => {
                let body = res.into_string().unwrap_or_else(|_| String::from("<no body>"));
                Err(TadoClientError::Http { status, message: body })
            }
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
