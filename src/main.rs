//! # leadforge (CLI)
//!
//! `leadforge` is a command-line tool for extracting high-value leads
//! from external data sources using a concurrent, fault-tolerant pipeline.
//!
//! This binary is responsible for:
//!
//! - parsing CLI arguments
//! - invoking the core execution pipeline
//! - serializing results to JSON
//! - writing output to stdout
//!
//! ---
//!
//! ## Execution Flow
//!
//! ```text
//! CLI input (clap)
//! ↓
//! Config (config.rs)
//! ↓
//! LeadForgeCommand (core domain)
//! ↓
//! run()
//! ↓
//! Vec<Leads>
//! ↓
//! JSON output (stdout)
//! ```
//!
//! ---
//!
//! ## Example
//!
//! ```bash
//! leadforge hacker-news --limit 5 --keyword rust
//! ```
//!
//! Output:
//!
//! ```json
//! [{"id":123,"title":"Rust Developer", ...}]
//! ```
//!
//! The output is designed to be:
//!
//! - machine-readable (JSON)
//! - pipe-friendly (stdout)
//!
//! Example usage in pipelines:
//!
//! ```bash
//! leadforge hn --keyword rust | jq
//! ```
//!
//! ---
//!
//! ## Error Handling
//!
//! Errors are handled using [`anyhow`] at the boundary layer.
//!
//! This allows:
//!
//! - rich context (`.context(...)`)
//! - simplified error propagation
//!
//! Internally, the core library uses structured errors
//! (`LeadForgeError`) which are converted here.
//!
//! ---
//!
//! ## Design Philosophy
//!
//! This file intentionally contains **minimal logic**.
//!
//! Responsibilities are strictly limited to:
//!
//! - orchestration
//! - I/O
//!
//! All business logic lives in the library crate (`leadforge`),
//! making it reusable outside the CLI.

use anyhow::{Context, Result};
use clap::Parser;
use leadforge::run;
use std::io::{self, Write};

mod config;

/// Application entry point.
///
/// Parses CLI arguments, executes the selected command,
/// and writes the results as JSON to stdout.
///
/// # Errors
///
/// Returns an error if:
///
/// - CLI parsing fails
/// - the execution pipeline fails
/// - serialization fails
/// - writing to stdout fails
#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI input into structured configuration
    let config = config::Config::parse();
    let command = config.command;

    // Execute the core pipeline
    let results = run(command.into())
        .await
        .context("failed to execute command")?;

    // Serialize results to JSON
    let json = serde_json::to_string(&results).context("failed to serialize the results")?;

    // Write to stdout (pipe-friendly output)
    let mut stdout = io::stdout();
    stdout.write_all(json.as_bytes())?;

    Ok(())
}
