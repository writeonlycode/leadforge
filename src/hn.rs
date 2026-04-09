//! # Hacker News Source
//!
//! This module implements the **Hacker News ingestion pipeline** for `leadforge`.
//!
//! It fetches job postings from the official Hacker News API and processes them
//! using a **streaming, concurrent pipeline with retry logic and filtering**.
//!
//! ---
//!
//! ## Pipeline Overview
//!
//! The core execution flow in [`fetch_jobs`] looks like this:
//!
//! ```text
//! fetch max item id  ─┐
//!                     ├─→ build ID stream
//! fetch job stories ──┘
//!                         ↓
//!                   stream::iter(ids)
//!                         ↓
//!               async fetch (concurrent)
//!                         ↓
//!               retry + backoff logic
//!                         ↓
//!                 filter_map (Job only)
//!                         ↓
//!                 keyword filtering
//!                         ↓
//!                     take(limit)
//!                         ↓
//!                     collect()
//! ```
//!
//! ---
//!
//! ## Data Strategy
//!
//! The Hacker News API provides:
//!
//! - `/maxitem` → latest item ID
//! - `/jobstories` → curated list of job IDs
//!
//! To maximize coverage, this module:
//!
//! 1. Uses `jobstories` as a high-signal dataset
//! 2. Falls back to scanning recent IDs from `maxitem`
//! 3. Merges both into a single descending stream
//!
//! This hybrid approach improves recall while maintaining efficiency.
//!
//! ---
//!
//! ## Concurrency Model
//!
//! Concurrency is controlled via:
//!
//! ```text
//! stream::iter(ids)
//!     .map(fetch)
//!     .buffer_unordered(N)
//! ```
//!
//! Where `N = max_concurrency`.
//!
//! This ensures:
//!
//! - high throughput
//! - bounded resource usage
//! - no thread explosion
//!
//! ---
//!
//! ## Retry Strategy
//!
//! Each request is retried with **exponential backoff + jitter**:
//!
//! ```text
//! delay = (base * 2^retries + jitter).min(max_delay)
//! ```
//!
//! Retry is only attempted for **transient errors**, such as:
//!
//! - timeouts
//! - connection failures
//! - 5xx server errors
//! - rate limiting (429)
//!
//! ---
//!
//! ## Filtering
//!
//! Filtering is applied *inside the stream*, not after collection.
//!
//! This means:
//!
//! - fewer allocations
//! - less work overall
//! - early discard of irrelevant data
//!
//! ---
//!
//! ## Design Goals
//!
//! - **Efficiency** → streaming + concurrency
//! - **Resilience** → retries + timeouts
//! - **Scalability** → large ID ranges
//! - **Composability** → reusable async functions

use futures::{
    StreamExt,
    future::{self, join},
    stream,
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::LeadForgeError;

/// Wrapper for the `/jobstories` endpoint response.
///
/// Contains a list of Hacker News item IDs representing job postings.
#[derive(Debug, Serialize, Deserialize)]
pub struct HackerNewsJobStories(Vec<u64>);

/// Wrapper for the `/maxitem` endpoint response.
///
/// Represents the latest item ID in the Hacker News dataset.
#[derive(Debug, Serialize, Deserialize)]
pub struct HackerNewsMaxItemId(u64);

const HN_JOBSTORIES_URL: &str = "https://hacker-news.firebaseio.com/v0/jobstories.json";
const HN_MAXITEM_URL: &str = "https://hacker-news.firebaseio.com/v0/maxitem.json";

/// Represents a generic Hacker News item.
///
/// The API returns different item types (`job`, `story`, `comment`, etc.).
/// We only care about `job`, and ignore everything else.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HackerNewsItem {
    /// A job posting.
    Job(HackerNewsJob),

    /// Any non-job item.
    #[serde(other)]
    Other,
}

/// Represents a Hacker News job posting.
///
/// Fields are optional because the API does not guarantee
/// that all fields are present for every item.
#[derive(Debug, Serialize, Deserialize)]
pub struct HackerNewsJob {
    pub id: u64,
    pub by: Option<String>,
    pub score: Option<u64>,
    pub time: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub text: Option<String>,
}

/// Fetches job postings from Hacker News using a concurrent streaming pipeline.
///
/// # Parameters
///
/// - `limit` → maximum number of results to return
/// - `max_concurrency` → number of concurrent HTTP requests
/// - `max_retries` → retry attempts per request
/// - `timeout` → per-request timeout
/// - `keyword` → optional case-insensitive filter on job titles
///
/// # Behavior
///
/// - Fetches job IDs from `/jobstories`
/// - Expands coverage using `/maxitem`
/// - Streams IDs in reverse order (newest first)
/// - Fetches items concurrently
/// - Retries transient failures
/// - Filters only job postings
/// - Applies keyword filtering
/// - Stops after `limit` results
///
/// # Errors
///
/// Returns [`LeadForgeError`] if:
///
/// - API requests fail and retries are exhausted
/// - network issues occur
///
/// # Performance
///
/// This function is optimized for:
///
/// - high throughput (via concurrency)
/// - low memory usage (streaming)
/// - early termination (`take(limit)`)
pub async fn fetch_jobs(
    limit: usize,
    max_concurrency: usize,
    max_retries: usize,
    timeout: Duration,
    keyword: Option<String>,
) -> Result<Vec<HackerNewsJob>, LeadForgeError> {
    let client = reqwest::Client::new();

    let (max_item_id, job_stories) =
        join(fetch_max_item_id(&client), fetch_job_stories(&client)).await;

    let HackerNewsMaxItemId(max_item_id) = max_item_id?;
    let HackerNewsJobStories(mut job_stories) = job_stories?;

    job_stories.sort();
    let first = job_stories.first().copied().unwrap_or(max_item_id);
    let range = 0..first;
    let ids = range.chain(job_stories).rev();

    let keyword = keyword.map(|k| k.to_lowercase());

    let result: Vec<HackerNewsJob> = stream::iter(ids)
        .map(|id| fetch_job_with_retry(&client, timeout, max_retries, id))
        .buffer_unordered(max_concurrency)
        .filter_map(|item| async move {
            match item {
                Ok(HackerNewsItem::Job(job)) => Some(job),
                Ok(_) => None,
                Err(error) => {
                    eprintln!("fetch failed: {}", error);
                    None
                }
            }
        })
        .filter(|job| future::ready(filter(job.title.as_deref(), keyword.as_deref())))
        .take(limit)
        .collect()
        .await;

    Ok(result)
}

/// Fetches a single item with retry logic.
///
/// Retries are performed using exponential backoff with jitter,
/// and only for retryable errors.
///
/// # Parameters
///
/// - `client` → shared HTTP client
/// - `timeout` → per-request timeout
/// - `max_retries` → maximum retry attempts
/// - `job_id` → Hacker News item ID
///
/// # Errors
///
/// Returns an error if:
///
/// - retries are exhausted
/// - a non-retryable error occurs
pub async fn fetch_job_with_retry(
    client: &reqwest::Client,
    timeout: Duration,
    max_retries: usize,
    job_id: u64,
) -> Result<HackerNewsItem, LeadForgeError> {
    let mut retries = 0;

    loop {
        match fetch_job(client, timeout, job_id).await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if retries >= max_retries {
                    return Err(error);
                }

                if !error.is_retryable() {
                    return Err(error);
                }

                retries += 1;

                let jitter = rand::rng().random_range(0..100);
                let delay = ((100 * 2u64.pow(retries as u32)) + jitter).min(10_000);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    }
}

/// Fetches the latest item ID from Hacker News.
///
/// Used as a fallback to scan recent items.
pub async fn fetch_max_item_id(
    client: &reqwest::Client,
) -> Result<HackerNewsMaxItemId, LeadForgeError> {
    Ok(client
        .get(HN_MAXITEM_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Fetches curated job story IDs from Hacker News.
pub async fn fetch_job_stories(
    client: &reqwest::Client,
) -> Result<HackerNewsJobStories, LeadForgeError> {
    let ids: HackerNewsJobStories = client
        .get(HN_JOBSTORIES_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(ids)
}

/// Fetches a single Hacker News item.
///
/// Applies a per-request timeout and parses the response into [`HackerNewsItem`].
pub async fn fetch_job(
    client: &reqwest::Client,
    timeout: Duration,
    job_id: u64,
) -> Result<HackerNewsItem, LeadForgeError> {
    let result = client
        .get(format!(
            "https://hacker-news.firebaseio.com/v0/item/{}.json",
            job_id
        ))
        .timeout(timeout)
        .send()
        .await?
        .error_for_status()?
        .json::<HackerNewsItem>()
        .await?;

    Ok(result)
}

/// Filters job titles based on an optional keyword.
///
/// # Behavior
///
/// - If no keyword is provided → always returns `true`
/// - If title is missing → returns `false`
/// - Otherwise → performs case-insensitive substring match
pub fn filter(title: Option<&str>, keyword: Option<&str>) -> bool {
    if keyword.is_none() {
        return true;
    }

    if title.is_none() {
        return false;
    }

    if let (Some(keyword), Some(title)) = (keyword, title) {
        return title.to_lowercase().contains(keyword);
    }

    false
}
