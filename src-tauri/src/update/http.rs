use std::time::Duration;

use crate::update::manifest::current_version;

/// One HTTP client shape for every outbound update request: the same user
/// agent and connect timeout, a caller-chosen overall timeout.
pub(crate) fn client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(format!("Klipparkiv/{}", current_version()))
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .build()
}
