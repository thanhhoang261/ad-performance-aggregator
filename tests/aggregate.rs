use ad_aggregator::aggregate::{CampaignTotals, aggregate};

const SAMPLE: &str = "\
campaign_id,date,impressions,clicks,spend,conversions
CMP001,2025-01-01,12000,300,45.50,12
CMP002,2025-01-01,8000,120,28.00,4
CMP001,2025-01-02,14000,340,48.20,15
CMP003,2025-01-01,5000,60,15.00,3
CMP002,2025-01-02,8500,150,31.00,5
";

fn totals(impressions: u64, clicks: u64, spend_micros: u64, conversions: u64) -> CampaignTotals {
    CampaignTotals {
        impressions,
        clicks,
        spend_micros,
        conversions,
    }
}

#[test]
fn sums_rows_per_campaign() {
    let result = aggregate(SAMPLE.as_bytes()).unwrap();

    assert_eq!(result.rows_processed, 5);
    assert_eq!(result.rows_skipped, 0);
    assert_eq!(result.campaigns.len(), 3);
    assert_eq!(
        result.campaigns["CMP001"],
        totals(26000, 640, 93_700_000, 27)
    );
    assert_eq!(
        result.campaigns["CMP002"],
        totals(16500, 270, 59_000_000, 9)
    );
    assert_eq!(result.campaigns["CMP003"], totals(5000, 60, 15_000_000, 3));
}

#[test]
fn skips_malformed_rows_and_reports_them() {
    let input = "\
campaign_id,date,impressions,clicks,spend,conversions
CMP001,2025-01-01,100,10,1.00,1
CMP001,2025-01-02,abc,10,1.00,1
CMP001,2025-01-03,100,-5,1.00,1
CMP001,2025-01-04,100,10,1.00
,2025-01-05,100,10,1.00,1
CMP001,2025-01-06,100,10,1.2.3,1
CMP001,2025-01-07,100,10,2.50,0
";
    let result = aggregate(input.as_bytes()).unwrap();

    assert_eq!(result.rows_processed, 7);
    assert_eq!(result.rows_skipped, 5);
    assert_eq!(result.campaigns["CMP001"], totals(200, 20, 3_500_000, 1));

    let lines: Vec<u64> = result.skipped_examples.iter().map(|s| s.line).collect();
    assert_eq!(lines, [3, 4, 5, 6, 7]);
    assert_eq!(
        result.skipped_examples[0].reason,
        "invalid impressions `abc`"
    );
    assert_eq!(
        result.skipped_examples[2].reason,
        "missing conversions (row has 5 fields)"
    );
    assert_eq!(result.skipped_examples[3].reason, "empty campaign_id");
}

#[test]
fn resolves_columns_by_header_name() {
    let input = "\
conversions,spend,clicks,impressions,date,campaign_id
2,10.00,5,100,2025-01-01,CMP001
";
    let result = aggregate(input.as_bytes()).unwrap();
    assert_eq!(result.campaigns["CMP001"], totals(100, 5, 10_000_000, 2));
}

#[test]
fn tolerates_surrounding_whitespace_and_quotes() {
    let input = "campaign_id, date, impressions, clicks, spend, conversions\n\"CMP001\", 2025-01-01, 100 , 5, 10.5 , 2\n";
    let result = aggregate(input.as_bytes()).unwrap();
    assert_eq!(result.campaigns["CMP001"], totals(100, 5, 10_500_000, 2));
}

#[test]
fn handles_byte_order_mark_and_crlf_line_endings() {
    let input = "\u{feff}campaign_id,date,impressions,clicks,spend,conversions\r\nCMP001,2025-01-01,100,5,10.00,2\r\n";
    let result = aggregate(input.as_bytes()).unwrap();
    assert_eq!(result.campaigns["CMP001"], totals(100, 5, 10_000_000, 2));
}

#[test]
fn counts_quoted_multiline_field_as_one_record() {
    let input = "campaign_id,date,impressions,clicks,spend,conversions\n\"CMP\n001\",2025-01-01,100,5,10.00,2\n";
    let result = aggregate(input.as_bytes()).unwrap();
    assert_eq!(result.rows_processed, 1);
    assert_eq!(result.campaigns["CMP\n001"], totals(100, 5, 10_000_000, 2));
}

#[test]
fn rejects_header_missing_required_column() {
    let error = aggregate("campaign_id,date,impressions,clicks,spend\n".as_bytes()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing required column `conversions`"),
        "{error}"
    );
}

#[test]
fn rejects_empty_input() {
    let error = aggregate("".as_bytes()).unwrap_err();
    assert!(error.to_string().contains("input is empty"), "{error}");
}

#[test]
fn accepts_header_only_input() {
    let result =
        aggregate("campaign_id,date,impressions,clicks,spend,conversions\n".as_bytes()).unwrap();
    assert_eq!(result.rows_processed, 0);
    assert!(result.campaigns.is_empty());
}
