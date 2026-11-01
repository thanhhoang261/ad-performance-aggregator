use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ad_aggregator::aggregate::aggregate;
use ad_aggregator::report::{Campaign, into_campaigns, top_by_cpa, top_by_ctr, write_csv};
use anyhow::{Context, Result};
use clap::Parser;

const TOP_N: usize = 10;

/// Aggregates ad performance CSV data per campaign and writes the top 10
/// campaigns by CTR and by CPA.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Path to the input CSV file.
    #[arg(short, long)]
    input: PathBuf,

    /// Directory to write top10_ctr.csv and top10_cpa.csv into.
    #[arg(short, long, default_value = "results")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let started = Instant::now();

    let input = File::open(&args.input)
        .with_context(|| format!("cannot open input file {}", args.input.display()))?;
    let aggregation =
        aggregate(input).with_context(|| format!("failed to process {}", args.input.display()))?;

    for skipped in &aggregation.skipped_examples {
        eprintln!("warning: skipped line {}: {}", skipped.line, skipped.reason);
    }

    let campaigns = into_campaigns(aggregation.campaigns);
    fs::create_dir_all(&args.output)
        .with_context(|| format!("cannot create output directory {}", args.output.display()))?;
    write_report(
        &args.output.join("top10_ctr.csv"),
        &top_by_ctr(&campaigns, TOP_N),
    )?;
    write_report(
        &args.output.join("top10_cpa.csv"),
        &top_by_cpa(&campaigns, TOP_N),
    )?;

    eprintln!(
        "Processed {} rows ({} skipped) across {} campaigns in {:.2?}",
        aggregation.rows_processed,
        aggregation.rows_skipped,
        campaigns.len(),
        started.elapsed()
    );
    eprintln!("Results written to {}", args.output.display());
    Ok(())
}

fn write_report(path: &Path, campaigns: &[&Campaign]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    write_csv(BufWriter::new(file), campaigns)
        .with_context(|| format!("failed to write {}", path.display()))
}
