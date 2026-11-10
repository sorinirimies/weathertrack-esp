//! HTTP access, behind a trait so the state machine and the pages are tested without a network.

use std::time::Duration;

/// Fetching and posting; every error is a short human-readable string.
pub trait Net: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
    /// `POST` a URL-encoded form.
    fn post_form(&self, url: &str, body: &str) -> Result<Vec<u8>, String>;
}

/// The real thing: `ureq` with timeouts (a dead device must not hang the web app).
pub struct UreqNet {
    agent: ureq::Agent,
}

impl std::fmt::Debug for UreqNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UreqNet")
    }
}

impl UreqNet {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent("weathertrack-web")
            .build()
            .into();
        Self { agent }
    }
}

impl Default for UreqNet {
    fn default() -> Self {
        Self::new()
    }
}

impl Net for UreqNet {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut resp = self.agent.get(url).call().map_err(|e| e.to_string())?;
        resp.body_mut().read_to_vec().map_err(|e| e.to_string())
    }

    fn post_form(&self, url: &str, body: &str) -> Result<Vec<u8>, String> {
        let mut resp = self
            .agent
            .post(url)
            .content_type("application/x-www-form-urlencoded")
            .send(body)
            .map_err(|e| e.to_string())?;
        resp.body_mut().read_to_vec().map_err(|e| e.to_string())
    }
}
