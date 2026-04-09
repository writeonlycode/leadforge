use futures::{
    StreamExt,
    future::{self, join},
    stream,
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::LeadForgeError;

#[derive(Debug, Serialize, Deserialize)]
pub struct HackerNewsJobStories(Vec<u64>);

#[derive(Debug, Serialize, Deserialize)]
pub struct HackerNewsMaxItemId(u64);

const HN_JOBSTORIES_URL: &str = "https://hacker-news.firebaseio.com/v0/jobstories.json";
const HN_MAXITEM_URL: &str = "https://hacker-news.firebaseio.com/v0/maxitem.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HackerNewsItem {
    Job(HackerNewsJob),

    #[serde(other)]
    Other,
}

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
