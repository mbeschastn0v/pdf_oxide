//! pdf_oxide extraction-quality benchmark.
//!
//! Computes TF1 (token F1) and SF1 (block-weighted structural F1 with
//! LIS order penalty) against a directory of ground-truth markdown files.
//! See `PLAN.md` for scoring formulas and sequencing.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod engine;
mod report;
mod score;
mod sf1;

#[derive(Parser)]
#[command(name = "benchmark-harness", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run an engine against a corpus and emit a JSON report.
    Run(RunArgs),
    /// Compare two JSON reports; exit non-zero on meaningful regression.
    Diff(DiffArgs),
}

#[derive(Parser)]
struct RunArgs {
    /// Engine to benchmark.
    #[arg(long, value_enum)]
    engine: engine::EngineKind,

    /// Directory containing PDFs to extract.
    #[arg(long)]
    corpus: PathBuf,

    /// Directory of ground-truth markdown files, matched by stem.
    #[arg(long)]
    ground_truth: PathBuf,

    /// Output JSON report path.
    #[arg(long)]
    output: PathBuf,

    /// Seconds before an individual extraction is aborted (0 = no limit).
    #[arg(long, default_value_t = 60)]
    timeout_secs: u64,
}

#[derive(Parser)]
struct DiffArgs {
    base: PathBuf,
    head: PathBuf,

    /// Fail if mean TF1 drops by more than this (percentage points).
    #[arg(long, default_value_t = 0.5)]
    mean_tf1_drop_pp: f64,

    /// Fail if any fixture's TF1 drops by more than this (pp).
    #[arg(long, default_value_t = 5.0)]
    per_fixture_tf1_drop_pp: f64,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run(args) => report::run(args),
        Cmd::Diff(args) => report::diff(args),
    }
}
