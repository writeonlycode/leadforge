use std::time::Duration;
use thiserror::Error;

mod hn;

use crate::hn::HackerNewsJob;

#[derive(Debug, Error)]
pub enum LeadForgeError {
    #[error(transparent)]
    NetworkError(#[from] reqwest::Error),
}

impl LeadForgeError {
    pub fn is_retryable(&self) -> bool {
        match self {
            LeadForgeError::NetworkError(error) => {
                error.is_timeout()
                    || error.is_connect()
                    || error.is_request()
                    || error
                        .status()
                        .map(|s| s.is_server_error() || s.as_u16() == 429)
                        .unwrap_or(false)
            }
        }
    }
}

pub enum LeadForgeCommand {
    HackerNews {
        limit: usize,
        max_concurrency: usize,
        max_retries: usize,
        timeout: Duration,
        keyword: Option<String>,
    },
}

pub async fn run(command: LeadForgeCommand) -> Result<Vec<HackerNewsJob>, LeadForgeError> {
    match command {
        LeadForgeCommand::HackerNews {
            limit,
            max_concurrency,
            max_retries,
            timeout,
            keyword,
        } => hn::fetch_jobs(limit, max_concurrency, max_retries, timeout, keyword).await,
    }
}
