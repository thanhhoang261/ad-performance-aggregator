//! Exact fixed-point helpers.
//!
//! Spend values are summed as integer micro-dollars instead of `f64` so that
//! totals over tens of millions of rows carry no floating-point drift, and
//! ratios are rounded with integer arithmetic so the printed digits are exact.

/// Number of spend units per dollar (micro-dollars).
pub const SPEND_SCALE: u64 = 1_000_000;

const MAX_FRACTION_DIGITS: u32 = 6;

/// Parses a non-negative decimal such as `45.5` into micro-units.
///
/// Returns `None` for empty input, signs, exponents, more than six fractional
/// digits, or values that overflow `u64`.
pub fn parse_micros(text: &[u8]) -> Option<u64> {
    let (whole, fraction) = match text.iter().position(|&b| b == b'.') {
        Some(dot) => (&text[..dot], &text[dot + 1..]),
        None => (text, &[][..]),
    };
    if (whole.is_empty() && fraction.is_empty()) || fraction.len() > MAX_FRACTION_DIGITS as usize {
        return None;
    }

    let whole_value = parse_digits(whole)?;
    let fraction_value = parse_digits(fraction)?;
    let fraction_scale = 10u64.pow(MAX_FRACTION_DIGITS - fraction.len() as u32);

    whole_value
        .checked_mul(SPEND_SCALE)?
        .checked_add(fraction_value * fraction_scale)
}

/// Parses a non-negative integer. An empty slice parses as zero so that
/// `parse_micros` can accept `.5` and `5.`; callers needing a value reject
/// empty fields before calling.
pub fn parse_digits(text: &[u8]) -> Option<u64> {
    text.iter().try_fold(0u64, |acc, &b| {
        if !b.is_ascii_digit() {
            return None;
        }
        acc.checked_mul(10)?.checked_add(u64::from(b - b'0'))
    })
}

/// Formats `numerator / denominator` with `decimals` fractional digits,
/// rounding half away from zero. `denominator` must be non-zero.
pub fn format_ratio(numerator: u128, denominator: u128, decimals: u32) -> String {
    let scale = 10u128.pow(decimals);
    let scaled = (numerator * scale * 2 + denominator) / (denominator * 2);
    let whole = scaled / scale;
    if decimals == 0 {
        return whole.to_string();
    }
    let fraction = scaled % scale;
    format!("{whole}.{fraction:0width$}", width = decimals as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimals_with_varying_precision() {
        assert_eq!(parse_micros(b"45.50"), Some(45_500_000));
        assert_eq!(parse_micros(b"964.6"), Some(964_600_000));
        assert_eq!(parse_micros(b"12"), Some(12_000_000));
        assert_eq!(parse_micros(b"0.000001"), Some(1));
        assert_eq!(parse_micros(b".5"), Some(500_000));
        assert_eq!(parse_micros(b"5."), Some(5_000_000));
    }

    #[test]
    fn rejects_invalid_decimals() {
        for input in [
            &b""[..],
            b".",
            b"-1.0",
            b"1e3",
            b"1.2.3",
            b"abc",
            b"1.0000001",
            b" 1",
        ] {
            assert_eq!(
                parse_micros(input),
                None,
                "input {:?}",
                String::from_utf8_lossy(input)
            );
        }
        assert_eq!(parse_micros(b"99999999999999999999"), None);
    }

    #[test]
    fn formats_ratios_with_half_up_rounding() {
        assert_eq!(format_ratio(1, 3, 4), "0.3333");
        assert_eq!(format_ratio(2, 3, 4), "0.6667");
        assert_eq!(format_ratio(1, 8, 2), "0.13");
        assert_eq!(format_ratio(12_500_500_000, 1_000_000, 2), "12500.50");
        assert_eq!(format_ratio(7, 2, 0), "4");
    }
}
