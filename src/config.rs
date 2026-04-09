use std::time::Duration;

use clap::{Parser, Subcommand};
use leadforge::LeadForgeCommand;

#[derive(Debug, Parser)]
#[clap(about)]
pub struct Config {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[clap(
        alias = "hn",
        about = "Fetch Hacker News job postings with concurrent requests and keyword filtering"
    )]
    HackerNews {
        #[clap(long, default_value_t = 10, help = "Number of job postings to return")]
        limit: usize,

        #[clap(long, default_value_t = 100, help = "Number of concurrent connections")]
        max_concurrency: usize,

        #[clap(
            long,
            default_value_t = 3,
            help = "Number of retry attempts per request on transient failures (e.g. timeouts, 5xx errors)"
        )]
        max_retries: usize,

        #[clap(
            long,
            default_value = "10",
            value_parser = parse_timeout,
            help = "Request timeout in seconds (applies per HTTP request)"
        )]
        timeout: Duration,

        #[clap(
            long,
            help = "Filter results by keyword in the job title (case-insensitive)"
        )]
        keyword: Option<String>,
    },
}

fn parse_timeout(input: &str) -> Result<Duration, anyhow::Error> {
    let timeout = input.parse()?;
    Ok(Duration::from_secs(timeout))
}

impl From<Command> for LeadForgeCommand {
    fn from(value: Command) -> Self {
        match value {
            Command::HackerNews {
                limit,
                max_concurrency,
                max_retries,
                timeout,
                keyword,
            } => LeadForgeCommand::HackerNews {
                limit,
                max_concurrency,
                max_retries,
                timeout,
                keyword,
            },
        }
    }
}
