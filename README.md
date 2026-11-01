# Ad Performance Aggregator

A Rust CLI that streams a large advertising CSV (~1 GB, 26.8 M rows), aggregates
metrics per `campaign_id`, and writes the top 10 campaigns by highest CTR and by
lowest CPA.

**1 GB processed in ~3.2 s with ~3 MB peak memory** (Apple M1 Pro, see [Benchmark](#benchmark)).

## Setup

Requires Rust 1.85+ (2024 edition). Install via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Download `ad_data.csv.zip` from the
[challenge repository](https://github.com/flinters-vietnam/recruitment/tree/main/fv-sec-001-software-engineer-challenge)
and unzip it into the project root. The dataset is not committed because it is
larger than GitHub's file size limit.

## Run

```bash
cargo run --release -- --input ad_data.csv --output results/
```

Or build once and run the binary directly:

```bash
cargo build --release
./target/release/ad-aggregator --input ad_data.csv --output results/
```

| Option | Default | Description |
|---|---|---|
| `-i, --input <PATH>` | required | Input CSV file |
| `-o, --output <DIR>` | `results` | Directory for `top10_ctr.csv` and `top10_cpa.csv` (created if missing) |

Progress information and warnings go to stderr:

```
Processed 26843544 rows (0 skipped) across 50 campaigns in 3.15s
Results written to results/
```

### Docker

```bash
docker build -t ad-aggregator .
docker run --rm -v "$PWD:/data" ad-aggregator --input ad_data.csv --output results
```

### Tests

```bash
cargo test
```

20 tests cover decimal parsing and rounding, ranking and tie-breaking, CSV
output, malformed rows, header handling, and end-to-end CLI behaviour
(including a missing input file).

## Output

The generated files are committed in [`results/`](results/):
[`top10_ctr.csv`](results/top10_ctr.csv) and [`top10_cpa.csv`](results/top10_cpa.csv).

Columns: `campaign_id, total_impressions, total_clicks, total_spend, total_conversions, CTR, CPA`.
`total_spend` and `CPA` have 2 decimals and `CTR` has 4, matching the examples in
the challenge. A campaign with no conversions has an empty `CPA` field (`null`).

The results were cross-checked against an independent Python implementation
using `Decimal`/`Fraction` arithmetic; both files matched byte for byte.

## Libraries

| Crate | Purpose |
|---|---|
| [`csv`](https://crates.io/crates/csv) | Fast, RFC 4180-compliant CSV reading and writing |
| [`clap`](https://crates.io/crates/clap) | Command-line argument parsing |
| [`anyhow`](https://crates.io/crates/anyhow) | Error propagation with context messages |
| [`tempfile`](https://crates.io/crates/tempfile) (dev only) | Temporary directories in CLI tests |

## Benchmark

Release build (`lto = true`, `codegen-units = 1`), 5 consecutive runs on an
Apple M1 Pro (32 GB RAM, macOS 26.5) with the 1,043,304,870-byte dataset. Full
log: [`benchmarks/benchmark.log`](benchmarks/benchmark.log). Reproduce with
`scripts/benchmark.sh ad_data.csv 5`.

| Metric | Result |
|---|---|
| Processing time | 3.10 – 3.25 s (≈ 320 MB/s, 8.5 M rows/s) |
| Peak memory (max RSS) | ~3.0 MB |

Memory stays constant regardless of input size: only one record buffer, a 1 MB
read buffer and one totals entry per campaign are held in memory.

## Design decisions

- **Streaming, single pass.** Records are read one at a time into a reused
  `ByteRecord`, so the file is never loaded into memory. Fields are parsed
  directly from bytes, skipping UTF-8 validation of numeric columns. Campaign
  lookups use the borrowed id, so a `String` is allocated only the first time
  each campaign is seen.
- **Exact money arithmetic.** `spend` is parsed into integer micro-dollars
  (`u64`) instead of `f64`. Summing 26.8 M floats would accumulate rounding
  error in `total_spend` and CPA; integers are exact and just as fast.
- **Exact ranking.** CTR and CPA are compared by cross-multiplying in `u128`
  (`a.clicks × b.impressions` vs `b.clicks × a.impressions`) instead of
  comparing rounded floats. This matters for this dataset: all campaigns in
  both top-10 lists have a CTR of `0.0275` or `0.0274` at 4 decimals, so ranking
  on rounded values would produce an essentially arbitrary order.
- **Deterministic ties.** Equal CTR or CPA is broken by `campaign_id`
  ascending, so output never depends on hash-map iteration order.
- **Half-up rounding with integers.** Displayed values are rounded half away
  from zero using integer arithmetic. Formatting an `f64` would round ties to
  even (`format!("{:.2}", 0.125)` gives `0.12`) and suffer from binary
  representation (`1.005` is stored as `1.00499…`, so it prints `1.00`).
- **Columns resolved by header name.** Column order in the input does not
  matter; a missing required column is a fatal error with a clear message.
- **Single-threaded by choice.** At ~320 MB/s the job finishes in about 3 s.
  Parallel chunked parsing could cut this further but would add significant
  complexity (splitting on record boundaries, quoted newlines, merging
  partial results) for a modest gain, so the simpler design was kept.

### Error handling

| Situation | Behaviour |
|---|---|
| Input file missing or unreadable | Exit with code 1 and a message naming the file |
| Empty file or header missing a required column | Exit with code 1 and a descriptive message |
| Malformed row (missing/empty field, non-numeric or negative number, invalid decimal, non-UTF-8 campaign id) | Row skipped and counted; the first 10 are printed as warnings with their line numbers |
| Total overflowing `u64` | Row skipped and reported (practically unreachable) |
| Campaign with 0 impressions | Excluded from the CTR ranking; CTR left empty |
| Campaign with 0 conversions | Excluded from the CPA ranking; CPA left empty |
| Fewer than 10 eligible campaigns | All eligible campaigns are written |

Surrounding whitespace, a UTF-8 byte order mark, CRLF line endings and quoted
fields are all handled. The `date` column is not used by any metric, so it is
not validated. Rows are checked for well-formedness only, not business rules:
a row with more clicks than impressions is still counted, because silently
dropping it would change the totals and the input is the source of truth.

## Project structure

```
src/
  main.rs        CLI entry point: arguments, file I/O, summary output
  aggregate.rs   Streaming CSV parsing and per-campaign totals
  report.rs      CTR/CPA calculation, ranking, CSV writing
  decimal.rs     Fixed-point parsing and exact ratio formatting
tests/
  aggregate.rs   Aggregation and malformed-input tests
  cli.rs         End-to-end tests of the binary
results/         Output for the 1 GB dataset
benchmarks/      Benchmark log
scripts/         Benchmark script
```

## AI assistance

This solution was built with Claude Code. The prompts are in
[`PROMPTS.md`](PROMPTS.md).
