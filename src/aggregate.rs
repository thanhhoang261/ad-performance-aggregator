//! Streaming aggregation of ad records by campaign.

use std::collections::HashMap;
use std::io::Read;

use anyhow::{Context, Result, bail};
use csv::{ByteRecord, ReaderBuilder};

use crate::decimal::{parse_digits, parse_micros};

const READ_BUFFER_BYTES: usize = 1 << 20;
const MAX_REPORTED_SKIPS: usize = 10;

/// Running totals for one campaign. Spend is kept in micro-dollars.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CampaignTotals {
    pub impressions: u64,
    pub clicks: u64,
    pub spend_micros: u64,
    pub conversions: u64,
}

impl CampaignTotals {
    fn checked_add(&self, row: &CampaignTotals) -> Option<Self> {
        Some(Self {
            impressions: self.impressions.checked_add(row.impressions)?,
            clicks: self.clicks.checked_add(row.clicks)?,
            spend_micros: self.spend_micros.checked_add(row.spend_micros)?,
            conversions: self.conversions.checked_add(row.conversions)?,
        })
    }
}

/// A row that could not be used, with the reason it was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRow {
    pub line: u64,
    pub reason: String,
}

/// Result of a full pass over the input.
#[derive(Debug, Default)]
pub struct Aggregation {
    pub campaigns: HashMap<String, CampaignTotals>,
    pub rows_processed: u64,
    pub rows_skipped: u64,
    /// The first few skipped rows, kept for diagnostics.
    pub skipped_examples: Vec<SkippedRow>,
}

/// Positions of the required columns, resolved from the header so that column
/// order in the file does not matter.
struct Columns {
    campaign_id: usize,
    impressions: usize,
    clicks: usize,
    spend: usize,
    conversions: usize,
}

impl Columns {
    fn from_header(header: &ByteRecord) -> Result<Self> {
        let find = |name: &str| {
            header
                .iter()
                .position(|field| field.trim_ascii() == name.as_bytes())
                .with_context(|| format!("missing required column `{name}` in header"))
        };
        Ok(Self {
            campaign_id: find("campaign_id")?,
            impressions: find("impressions")?,
            clicks: find("clicks")?,
            spend: find("spend")?,
            conversions: find("conversions")?,
        })
    }
}

/// Reads CSV records one at a time and sums them per campaign.
///
/// Memory use is bounded by the number of distinct campaigns, not the input
/// size. Malformed rows are skipped and counted; I/O errors and a missing or
/// invalid header abort the run.
pub fn aggregate<R: Read>(input: R) -> Result<Aggregation> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .buffer_capacity(READ_BUFFER_BYTES)
        .from_reader(input);

    let header = reader.byte_headers().context("failed to read CSV header")?;
    if header.is_empty() {
        bail!("input is empty: expected a CSV header");
    }
    let columns = Columns::from_header(header)?;

    let mut aggregation = Aggregation::default();
    let mut record = ByteRecord::new();
    while reader
        .read_byte_record(&mut record)
        .context("failed to read CSV record")?
    {
        aggregation.rows_processed += 1;
        let line = record.position().map_or(0, |position| position.line());
        if let Err(reason) = accumulate(&mut aggregation.campaigns, &record, &columns) {
            aggregation.rows_skipped += 1;
            if aggregation.skipped_examples.len() < MAX_REPORTED_SKIPS {
                aggregation
                    .skipped_examples
                    .push(SkippedRow { line, reason });
            }
        }
    }
    Ok(aggregation)
}

fn accumulate(
    campaigns: &mut HashMap<String, CampaignTotals>,
    record: &ByteRecord,
    columns: &Columns,
) -> Result<(), String> {
    let campaign_id = field(record, columns.campaign_id, "campaign_id")?;
    let row = CampaignTotals {
        impressions: integer_field(record, columns.impressions, "impressions")?,
        clicks: integer_field(record, columns.clicks, "clicks")?,
        spend_micros: parse_micros(field(record, columns.spend, "spend")?)
            .ok_or_else(|| invalid_value(record, columns.spend, "spend"))?,
        conversions: integer_field(record, columns.conversions, "conversions")?,
    };

    let id = std::str::from_utf8(campaign_id)
        .map_err(|_| "campaign_id is not valid UTF-8".to_owned())?;
    // Look up by borrowed key first so the common path allocates nothing.
    if !campaigns.contains_key(id) {
        campaigns.insert(id.to_owned(), CampaignTotals::default());
    }
    let totals = campaigns.get_mut(id).expect("campaign was just inserted");
    *totals = totals
        .checked_add(&row)
        .ok_or_else(|| "campaign totals overflow u64".to_owned())?;
    Ok(())
}

fn field<'r>(record: &'r ByteRecord, index: usize, name: &str) -> Result<&'r [u8], String> {
    match record.get(index).map(<[u8]>::trim_ascii) {
        Some(value) if !value.is_empty() => Ok(value),
        Some(_) => Err(format!("empty {name}")),
        None => Err(format!("missing {name} (row has {} fields)", record.len())),
    }
}

fn integer_field(record: &ByteRecord, index: usize, name: &str) -> Result<u64, String> {
    parse_digits(field(record, index, name)?).ok_or_else(|| invalid_value(record, index, name))
}

fn invalid_value(record: &ByteRecord, index: usize, name: &str) -> String {
    let value = record.get(index).unwrap_or_default();
    format!("invalid {name} `{}`", String::from_utf8_lossy(value))
}
