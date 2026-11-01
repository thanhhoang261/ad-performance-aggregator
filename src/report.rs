//! Campaign metrics, rankings and CSV output.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Write;

use anyhow::Result;

use crate::aggregate::CampaignTotals;
use crate::decimal::{SPEND_SCALE, format_ratio};

const HEADER: [&str; 7] = [
    "campaign_id",
    "total_impressions",
    "total_clicks",
    "total_spend",
    "total_conversions",
    "CTR",
    "CPA",
];

/// A campaign with its aggregated totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campaign {
    pub id: String,
    pub totals: CampaignTotals,
}

impl Campaign {
    /// CTR rounded to 4 decimals, or `None` when there were no impressions.
    pub fn ctr(&self) -> Option<String> {
        (self.totals.impressions > 0)
            .then(|| format_ratio(self.totals.clicks.into(), self.totals.impressions.into(), 4))
    }

    /// CPA rounded to 2 decimals, or `None` when there were no conversions.
    pub fn cpa(&self) -> Option<String> {
        (self.totals.conversions > 0).then(|| {
            let denominator = u128::from(self.totals.conversions) * u128::from(SPEND_SCALE);
            format_ratio(self.totals.spend_micros.into(), denominator, 2)
        })
    }

    fn total_spend(&self) -> String {
        format_ratio(self.totals.spend_micros.into(), SPEND_SCALE.into(), 2)
    }
}

pub fn into_campaigns(totals: HashMap<String, CampaignTotals>) -> Vec<Campaign> {
    totals
        .into_iter()
        .map(|(id, totals)| Campaign { id, totals })
        .collect()
}

/// Campaigns with the highest CTR. Campaigns without impressions have no CTR
/// and are excluded. Ties are broken by campaign id so output is deterministic.
pub fn top_by_ctr(campaigns: &[Campaign], limit: usize) -> Vec<&Campaign> {
    top_by(
        campaigns,
        limit,
        |c| c.totals.impressions > 0,
        |a, b| {
            // a.clicks / a.impressions vs b.clicks / b.impressions, cross-multiplied
            // to compare the exact ratios rather than rounded floats.
            compare_ratios(
                b.totals.clicks,
                b.totals.impressions,
                a.totals.clicks,
                a.totals.impressions,
            )
        },
    )
}

/// Campaigns with the lowest CPA. Campaigns without conversions have no CPA
/// and are excluded. Ties are broken by campaign id so output is deterministic.
pub fn top_by_cpa(campaigns: &[Campaign], limit: usize) -> Vec<&Campaign> {
    top_by(
        campaigns,
        limit,
        |c| c.totals.conversions > 0,
        |a, b| {
            compare_ratios(
                a.totals.spend_micros,
                a.totals.conversions,
                b.totals.spend_micros,
                b.totals.conversions,
            )
        },
    )
}

fn top_by(
    campaigns: &[Campaign],
    limit: usize,
    eligible: impl Fn(&Campaign) -> bool,
    order: impl Fn(&Campaign, &Campaign) -> Ordering,
) -> Vec<&Campaign> {
    let mut ranked: Vec<&Campaign> = campaigns.iter().filter(|c| eligible(c)).collect();
    ranked.sort_by(|a, b| order(a, b).then_with(|| a.id.cmp(&b.id)));
    ranked.truncate(limit);
    ranked
}

/// Compares `a_num / a_den` with `b_num / b_den` exactly. Denominators must be
/// non-zero.
fn compare_ratios(a_num: u64, a_den: u64, b_num: u64, b_den: u64) -> Ordering {
    (u128::from(a_num) * u128::from(b_den)).cmp(&(u128::from(b_num) * u128::from(a_den)))
}

/// Writes campaigns as CSV. A missing CTR or CPA is written as an empty field.
pub fn write_csv<W: Write>(output: W, campaigns: &[&Campaign]) -> Result<()> {
    let mut writer = csv::Writer::from_writer(output);
    writer.write_record(HEADER)?;
    for campaign in campaigns {
        let totals = &campaign.totals;
        writer.write_record([
            campaign.id.clone(),
            totals.impressions.to_string(),
            totals.clicks.to_string(),
            campaign.total_spend(),
            totals.conversions.to_string(),
            campaign.ctr().unwrap_or_default(),
            campaign.cpa().unwrap_or_default(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign(
        id: &str,
        impressions: u64,
        clicks: u64,
        spend_dollars: u64,
        conversions: u64,
    ) -> Campaign {
        Campaign {
            id: id.to_owned(),
            totals: CampaignTotals {
                impressions,
                clicks,
                spend_micros: spend_dollars * SPEND_SCALE,
                conversions,
            },
        }
    }

    fn ids(campaigns: &[&Campaign]) -> Vec<String> {
        campaigns.iter().map(|c| c.id.clone()).collect()
    }

    #[test]
    fn ranks_by_highest_ctr_and_skips_zero_impressions() {
        let campaigns = vec![
            campaign("LOW", 1000, 10, 1, 1),
            campaign("HIGH", 1000, 50, 1, 1),
            campaign("NONE", 0, 0, 0, 0),
            campaign("MID", 1000, 30, 1, 1),
        ];
        assert_eq!(ids(&top_by_ctr(&campaigns, 10)), ["HIGH", "MID", "LOW"]);
        assert_eq!(ids(&top_by_ctr(&campaigns, 2)), ["HIGH", "MID"]);
    }

    #[test]
    fn ranks_by_lowest_cpa_and_skips_zero_conversions() {
        let campaigns = vec![
            campaign("PRICEY", 1, 1, 100, 2),
            campaign("CHEAP", 1, 1, 10, 2),
            campaign("NOCONV", 1, 1, 1, 0),
        ];
        assert_eq!(ids(&top_by_cpa(&campaigns, 10)), ["CHEAP", "PRICEY"]);
    }

    #[test]
    fn breaks_ties_by_campaign_id() {
        let campaigns = vec![campaign("B", 100, 5, 10, 1), campaign("A", 200, 10, 20, 2)];
        assert_eq!(ids(&top_by_ctr(&campaigns, 10)), ["A", "B"]);
        assert_eq!(ids(&top_by_cpa(&campaigns, 10)), ["A", "B"]);
    }

    #[test]
    fn compares_exact_ratios_that_round_equal() {
        // 0.33333 vs 0.33334: identical at 4 decimals but must still rank correctly.
        let campaigns = vec![
            campaign("A", 100_000, 33_333, 1, 1),
            campaign("B", 100_000, 33_334, 1, 1),
        ];
        assert_eq!(ids(&top_by_ctr(&campaigns, 10)), ["B", "A"]);
    }

    #[test]
    fn writes_csv_with_empty_cpa_when_no_conversions() {
        let campaigns = [
            campaign("CMP001", 12000, 300, 45, 12),
            campaign("CMP002", 8000, 120, 28, 0),
        ];
        let mut output = Vec::new();
        write_csv(&mut output, &campaigns.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "campaign_id,total_impressions,total_clicks,total_spend,total_conversions,CTR,CPA\n\
             CMP001,12000,300,45.00,12,0.0250,3.75\n\
             CMP002,8000,120,28.00,0,0.0150,\n"
        );
    }
}
