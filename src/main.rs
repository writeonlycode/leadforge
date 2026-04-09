use anyhow::{Context, Result};
use clap::Parser;
use leadforge::run;
use std::io::{self, Write};

mod config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::Config::parse();
    let command = config.command;

    let results = run(command.into())
        .await
        .context("failed to execute command")?;

    let json = serde_json::to_string(&results).context("failed to serialize the results")?;

    let mut stdout = io::stdout();
    stdout.write_all(json.as_bytes())?;

    Ok(())
}
