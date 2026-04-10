# LeadForge

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![CLI](https://img.shields.io/badge/type-CLI-blue)
![Crates.io](https://img.shields.io/crates/v/leadforge.svg)

High-performance Rust CLI for **concurrent lead generation from real-world data
sources**.

`leadforge` fetches and processes job listings (currently from **Hacker News**)
using an **async streaming pipeline with bounded concurrency, retry logic, and
filtering** — designed for building real lead generation workflows.

The tool is built for developers, freelancers, and founders who want to
**extract actionable opportunities from live data**.

---

# Features

### Concurrent Data Fetching

Fetches job postings using **bounded concurrency** via async streams.

This allows hundreds of requests to be processed efficiently without
overwhelming the system or the API.

---

### Streaming Pipeline Architecture

Jobs are processed as a stream:

```
job IDs
↓
async fetch (concurrent)
↓
retry + timeout handling
↓
filtering (keyword)
↓
collected results
```

Benefits:

* efficient resource usage
* fast time-to-first-result
* scalable to large datasets

---

### Retry & Resilience

Built-in retry logic with exponential backoff handles:

* network timeouts
* connection failures
* server errors (5xx)
* rate limiting (429)

This makes `leadforge` robust against unreliable APIs.

---

### Keyword Filtering

Filter results directly in the pipeline:

* case-insensitive matching
* applied during streaming (no extra pass)

Example:

```bash
leadforge hacker-news --keyword rust
```

---

### Structured Output (JSON)

Results are returned as structured JSON, making it easy to:

* pipe into other tools
* store results
* integrate with scripts or APIs

---

# Example

Fetch the latest 5 Hacker News job postings:

```bash
leadforge hacker-news --limit 5
```

Example output:

```json
[
  {
    "id": 47251163,
    "by": "yeldarb",
    "score": 1,
    "time": 1772646584,
    "title": "Roboflow (YC S20) Is Hiring a Security Engineer for AI Infra",
    "url": "https://roboflow.com/careers",
    "text": null
  }
]
```

---

# Real-World Usage

`leadforge` can be used as a **lead discovery engine**:

### Find Rust jobs

```bash
leadforge hacker-news --keyword rust --limit 20
```

---

### Save results to a file

```bash
leadforge hacker-news --limit 50 > jobs.json
```

---

### Pipe into other tools

```bash
leadforge hacker-news --keyword remote \
| jq '.[] | .title'
```

---

# CLI Options

| Option              | Description                                      |
| ------------------- | ------------------------------------------------ |
| `--limit <N>`       | Maximum number of job postings to return         |
| `--max-concurrency` | Maximum number of concurrent HTTP requests       |
| `--max-retries`     | Retry attempts per request on transient failures |
| `--timeout`         | Request timeout in seconds (per request)         |
| `--keyword`         | Filter results by keyword in job title           |

Example:

```bash
leadforge hacker-news \
  --limit 20 \
  --max-concurrency 50 \
  --max-retries 3 \
  --timeout 10 \
  --keyword rust
```

---

# Documentation

Generate full Rust documentation locally:

```bash
cargo doc --open
```

Key components:

* `run` — core execution entry point
* `LeadForgeCommand` — command abstraction layer
* `fetch_jobs` — async pipeline for job ingestion
* `LeadForgeError` — structured error handling

---

# Architecture

`leadforge` is structured as a clean layered system:

```
CLI (clap)
↓
Command conversion
↓
Execution (run)
↓
Async pipeline (streams + concurrency)
↓
HTTP client (reqwest)
```

This separation allows:

* easy extension to new data sources
* clean testing
* reusable core logic

---

# Performance

The tool is optimized for:

* high concurrency
* low overhead per request
* minimal blocking

Performance depends on:

* network latency
* API responsiveness
* concurrency settings

---

# Installation

## Build from Source

```bash
git clone https://github.com/writeonlycode/leadforge
cd leadforge
cargo build --release
```

Binary:

```bash
target/release/leadforge
```

---

## Install via Cargo

```bash
cargo install leadforge
```

---

# Why Rust?

Rust allows `leadforge` to combine:

* **high concurrency without threads explosion**
* **memory safety**
* **predictable performance**
* **zero-cost abstractions**

This makes it ideal for **data ingestion pipelines and lead generation tools**.

---

# License

MIT / Apache 2.0
