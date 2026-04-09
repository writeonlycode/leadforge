//! # CLI Configuration
//!
//! This module defines the **command-line interface (CLI)** for `leadforge`
//! using [`clap`].
//!
//! It is responsible for:
//!
//! - parsing user input
//! - validating arguments
//! - providing help/usage information
//! - converting CLI commands into domain-level commands
//!
//! ---
//!
//! ## Design Overview
//!
//! The CLI layer is intentionally kept **separate from the core logic**.
//!
//! ```text
//! CLI (clap)
//! ↓
//! Config / Command
//! ↓
//! Conversion (From<Command>)
//! ↓
//! LeadForgeCommand (domain)
//! ↓
//! run()
//! ```
//!
//! This separation ensures:
//!
//! - the core logic is reusable (e.g. in APIs or background jobs)
//! - the CLI remains a thin interface layer
//! - input validation happens at the boundary
//!
//! ---
//!
//! ## Commands
//!
//! Currently supported:
//!
//! - `hacker-news` (alias: `hn`) → fetch job postings from Hacker News
//!
//! Example:
//!
//! ```bash
//! leadforge hacker-news --limit 10 --keyword rust
//! ```
//!
//! ---
//!
//! ## Argument Strategy
//!
//! The CLI defines **defaults and constraints**, so the rest of the system
//! can assume valid input.
//!
//! Examples:
//!
//! - `limit` defaults to a safe value
//! - `max_concurrency` is bounded to avoid overload
//! - `timeout` is parsed into a strongly-typed `Duration`
//!
//! This avoids the need for validation deeper in the pipeline.
//!
//! ---
//!
//! ## Extensibility
//!
//! New data sources can be added by:
//!
//! 1. Adding a new variant to [`Command`]
//! 2. Mapping it in `From<Command>`
//! 3. Implementing the corresponding logic in the core crate
//!
//! This keeps the CLI scalable as the project grows.

use std::time::Duration;

use clap::{Parser, Subcommand};
use leadforge::LeadForgeCommand;

/// Top-level CLI configuration.
///
/// This struct is the entry point for parsing command-line arguments.
/// It delegates to subcommands defined in [`Command`].
#[derive(Debug, Parser)]
#[clap(about)]
pub struct Config {
    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported CLI commands.
///
/// Each variant corresponds to a different data source or operation.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Fetch job postings from Hacker News.
    ///
    /// This command retrieves job listings using a concurrent
    /// streaming pipeline with retry logic and optional filtering.
    #[clap(
        alias = "hn",
        about = "Fetch Hacker News job postings with concurrent requests and keyword filtering"
    )]
    HackerNews {
        /// Maximum number of job postings to return.
        ///
        /// This acts as an upper bound; fewer results may be returned
        /// if not enough matching jobs are found.
        #[clap(long, default_value_t = 10, help = "Number of job postings to return")]
        limit: usize,

        /// Maximum number of concurrent HTTP requests.
        ///
        /// Higher values increase throughput but may:
        /// - increase network usage
        /// - trigger API rate limits
        #[clap(long, default_value_t = 100, help = "Number of concurrent connections")]
        max_concurrency: usize,

        /// Number of retry attempts per request.
        ///
        /// Retries are only performed for transient failures such as:
        /// - timeouts
        /// - connection errors
        /// - server errors (5xx)
        #[clap(
            long,
            default_value_t = 3,
            help = "Number of retry attempts per request on transient failures (e.g. timeouts, 5xx errors)"
        )]
        max_retries: usize,

        /// Request timeout in seconds (per HTTP request).
        ///
        /// This does not apply to the entire pipeline, only to individual requests.
        #[clap(
            long,
            default_value = "10",
            value_parser = parse_timeout,
            help = "Request timeout in seconds (applies per HTTP request)"
        )]
        timeout: Duration,

        /// Filter results by keyword in the job title (case-insensitive).
        ///
        /// Only jobs whose titles contain the keyword will be returned.
        #[clap(
            long,
            help = "Filter results by keyword in the job title (case-insensitive)"
        )]
        keyword: Option<String>,
    },
}

/// Parses a timeout value (in seconds) into a [`Duration`].
///
/// # Errors
///
/// Returns an error if the input cannot be parsed as an integer.
///
/// # Example
///
/// ```
/// let duration = super::parse_timeout("10").unwrap();
/// assert_eq!(duration.as_secs(), 10);
/// ```
fn parse_timeout(input: &str) -> Result<Duration, anyhow::Error> {
    let timeout = input.parse()?;
    Ok(Duration::from_secs(timeout))
}

/// Converts a CLI [`Command`] into a domain-level [`LeadForgeCommand`].
///
/// This separates:
///
/// - **user input representation** (CLI)
/// - **execution logic** (core library)
///
/// This conversion layer ensures that the core system remains
/// independent from CLI concerns.
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
