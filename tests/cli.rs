use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ad-aggregator"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn writes_both_reports() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("ads.csv");
    let output = dir.path().join("out");
    fs::write(
        &input,
        "campaign_id,date,impressions,clicks,spend,conversions\n\
         CMP001,2025-01-01,1000,50,100.00,10\n\
         CMP002,2025-01-01,1000,20,30.00,0\n\
         CMP003,2025-01-01,1000,30,15.00,3\n\
         CMP001,2025-01-02,oops,1,1.00,1\n",
    )
    .unwrap();

    let result = run(&[
        "--input",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(result.status.success(), "{stderr}");
    assert!(
        stderr.contains("warning: skipped line 5: invalid impressions `oops`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Processed 4 rows (1 skipped) across 3 campaigns"),
        "{stderr}"
    );

    let header =
        "campaign_id,total_impressions,total_clicks,total_spend,total_conversions,CTR,CPA\n";
    assert_eq!(
        fs::read_to_string(output.join("top10_ctr.csv")).unwrap(),
        format!(
            "{header}CMP001,1000,50,100.00,10,0.0500,10.00\n\
             CMP003,1000,30,15.00,3,0.0300,5.00\n\
             CMP002,1000,20,30.00,0,0.0200,\n"
        )
    );
    assert_eq!(
        fs::read_to_string(output.join("top10_cpa.csv")).unwrap(),
        format!(
            "{header}CMP003,1000,30,15.00,3,0.0300,5.00\n\
             CMP001,1000,50,100.00,10,0.0500,10.00\n"
        )
    );
}

#[test]
fn fails_with_clear_message_when_input_is_missing() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing.csv");

    let result = run(&[
        "--input",
        missing.to_str().unwrap(),
        "--output",
        dir.path().to_str().unwrap(),
    ]);

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("cannot open input file"), "{stderr}");
}

#[test]
fn requires_input_argument() {
    let result = run(&[]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--input"));
}
