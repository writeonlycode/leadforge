//! # leadforge
//!
//! High-performance async lead generation engine built in Rust.
//!
//! `leadforge` provides a streaming, concurrent pipeline for fetching,
//! filtering, and returning structured job data from external sources
//! such as Hacker News.
//!
//! The crate is designed around three core ideas:
//!
//! - **Streaming pipelines** → process data as it arrives
//! - **Bounded concurrency** → maximize throughput without overload
//! - **Resilience** → retries, timeouts, and error classification
//!
//! ---
//!
//! ## Architecture Overview
//!
//! The execution model follows a layered pipeline:
//!
//! ```text
//! Command
//! ↓
//! run()
//! ↓
//! source module (e.g. hn)
//! ↓
//! async stream (concurrent requests)
//! ↓
//! retry + timeout handling
//! ↓
//! filtering
//! ↓
//! Vec<Output>
//! ```
//!
//! Each data source (e.g. Hacker News) implements its own ingestion logic,
//! while sharing the same high-level execution pattern.
//!
//! ---
//!
//! ## Example
//!
//! ```no_run
//! use leadforge::{run, LeadForgeCommand};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let command = LeadForgeCommand::HackerNews {
//!         limit: 10,
//!         max_concurrency: 50,
//!         max_retries: 3,
//!         timeout: Duration::from_secs(10),
//!         keyword: Some("rust".to_string()),
//!     };
//!
//!     let results = run(command).await.unwrap();
//!
//!     for job in results {
//!         println!("{:?}", job);
//!     }
//! }
//! ```
//!
//! ---
//!
//! ## Design Goals
//!
//! - **Efficiency**: avoid unnecessary allocations and blocking
//! - **Scalability**: handle large ID ranges and high request volume
//! - **Composability**: expose a clean API usable from CLI or other programs
//! - **Robustness**: gracefully handle unreliable networks and APIs
//!
//! ---
//!
//! ## Modules
//!
//! - `hn` → Hacker News ingestion pipeline
//!
//! Additional sources can be added by following the same pattern.
//!
//! ---
//!
//! ## CLI Usage
//!
//! While `leadforge` is designed as a reusable library, it is primarily
//! consumed through its command-line interface.
//!
//! ### Basic Example
//!
//! ```bash
//! leadforge hacker-news --limit 10
//! ```
//!
//! ### With Filtering
//!
//! ```bash
//! leadforge hacker-news --keyword rust --limit 5
//! ```
//!
//! ### Controlling Concurrency and Retries
//!
//! ```bash
//! leadforge hn \
//!   --limit 20 \
//!   --max-concurrency 50 \
//!   --max-retries 5 \
//!   --timeout 5
//! ```
//!
//! ---
//!
//! ## Output Format
//!
//! Results are emitted as **JSON to stdout**, making them easy to
//! integrate with other tools.
//!
//! Example:
//!
//! ```json
//! [{"id":123,"title":"Rust Developer","url":"https://..."}]
//! ```
//!
//! ---
//!
//! ## Shell Integration
//!
//! Because output is written to stdout, `leadforge` composes naturally
//! with standard Unix tools:
//!
//! ```bash
//! leadforge hn --keyword rust | jq
//! ```
//!
//! ```bash
//! leadforge hn --limit 50 > jobs.json
//! ```
//!
//! This makes it suitable for:
//!
//! - automation scripts
//! - data pipelines
//! - lead generation workflows
//!
//! ---
//!
//! ## Library vs CLI
//!
//! The CLI is a thin wrapper around the core [`run`] function. If you need more control, you can
//! use the library directly.

use std::time::Duration;
use thiserror::Error;

mod hn;

use crate::hn::HackerNewsJob;

/// Errors that can occur during lead fetching and processing.
///
/// This type represents failures in external communication and
/// pipeline execution.
///
/// Currently, only network-related errors are exposed, but this
/// enum is designed to grow as new sources and failure modes are added.
#[derive(Debug, Error)]
pub enum LeadForgeError {
    /// Wrapper around HTTP/network errors from `reqwest`.
    ///
    /// This includes:
    /// - connection failures
    /// - timeouts
    /// - invalid responses
    /// - HTTP status errors
    #[error(transparent)]
    NetworkError(#[from] reqwest::Error),
}

impl LeadForgeError {
    /// Returns `true` if the error is considered retryable.
    ///
    /// Retryable errors typically include:
    ///
    /// - timeouts
    /// - connection failures
    /// - request-level errors
    /// - server-side errors (5xx)
    /// - rate limiting (429)
    ///
    /// This method is used internally to drive retry logic
    /// with exponential backoff.
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

/// High-level command describing what data to fetch.
///
/// This enum decouples the *intent* of the operation from its execution,
/// allowing the same API to be used by:
///
/// - CLI interfaces
/// - background jobs
/// - web services
///
/// Each variant represents a different data source.
pub enum LeadForgeCommand {
    /// Fetch job postings from Hacker News.
    ///
    /// Parameters:
    ///
    /// - `limit` → maximum number of results to return
    /// - `max_concurrency` → number of concurrent HTTP requests
    /// - `max_retries` → retry attempts per request
    /// - `timeout` → per-request timeout duration
    /// - `keyword` → optional case-insensitive filter applied to titles
    HackerNews {
        limit: usize,
        max_concurrency: usize,
        max_retries: usize,
        timeout: Duration,
        keyword: Option<String>,
    },
}

/// Executes a [`LeadForgeCommand`] and returns collected results.
///
/// This is the main entry point of the library.
///
/// # Behavior
///
/// - Dispatches to the appropriate data source
/// - Executes an async streaming pipeline
/// - Applies concurrency limits and retry logic
/// - Filters results if requested
/// - Collects results into a `Vec`
///
/// # Errors
///
/// Returns [`LeadForgeError`] if:
///
/// - network requests fail
/// - retries are exhausted
/// - the upstream API is unavailable
///
/// # Example
///
/// ```no_run
/// use leadforge::{run, LeadForgeCommand};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let results = run(LeadForgeCommand::HackerNews {
///     limit: 5,
///     max_concurrency: 10,
///     max_retries: 2,
///     timeout: Duration::from_secs(5),
///     keyword: None,
/// }).await?;
///
/// println!("Fetched {} jobs", results.len());
/// # Ok(())
/// # }
/// ```
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
