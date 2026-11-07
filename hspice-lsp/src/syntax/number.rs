//! Decoding of HSPICE numeric literals with scale factors and unit letters.

/// A recognised scale-factor suffix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor {
    /// Canonical lowercase spelling: `t g meg x k mil m u n p f a r`.
    pub suffix: &'static str,
    /// Factor the mantissa is multiplied with.
    pub multiplier: f64,
}

/// Result of decoding a numeric literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNumber {
    /// Numeric value including exponent and scale factor.
    pub value: f64,
    /// Scale factor that was applied, if any.
    pub scale: Option<ScaleFactor>,
    /// True if an `e`/`d` exponent was present.
    pub has_exponent: bool,
    /// Ignored trailing letters as written, may be empty.
    pub unit: String,
}

const MEG: ScaleFactor = ScaleFactor {
    suffix: "meg",
    multiplier: 1e6,
};
const MIL: ScaleFactor = ScaleFactor {
    suffix: "mil",
    multiplier: 25.4e-6,
};
const INFIX_R: ScaleFactor = ScaleFactor {
    suffix: "r",
    multiplier: 1.0,
};
const SINGLE_LETTER_SCALES: [ScaleFactor; 10] = [
    ScaleFactor {
        suffix: "t",
        multiplier: 1e12,
    },
    ScaleFactor {
        suffix: "g",
        multiplier: 1e9,
    },
    ScaleFactor {
        suffix: "x",
        multiplier: 1e6,
    },
    ScaleFactor {
        suffix: "k",
        multiplier: 1e3,
    },
    ScaleFactor {
        suffix: "m",
        multiplier: 1e-3,
    },
    ScaleFactor {
        suffix: "u",
        multiplier: 1e-6,
    },
    ScaleFactor {
        suffix: "n",
        multiplier: 1e-9,
    },
    ScaleFactor {
        suffix: "p",
        multiplier: 1e-12,
    },
    ScaleFactor {
        suffix: "f",
        multiplier: 1e-15,
    },
    ScaleFactor {
        suffix: "a",
        multiplier: 1e-18,
    },
];

/// Engineering prefixes used by `format_engineering`, largest first.
const ENGINEERING_PREFIXES: [(f64, &str); 11] = [
    (1e12, "t"),
    (1e9, "g"),
    (1e6, "meg"),
    (1e3, "k"),
    (1.0, ""),
    (1e-3, "m"),
    (1e-6, "u"),
    (1e-9, "n"),
    (1e-12, "p"),
    (1e-15, "f"),
    (1e-18, "a"),
];

/// Splits the longest prefix of ASCII digits from `text`.
///
/// # Arguments
/// * `text` - text to split
///
/// # Returns
/// The digits and the remaining text.
fn split_digits(text: &str) -> (&str, &str) {
    text.split_at(
        text.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len()),
    )
}

/// Splits `\d+(\.\d*)?|\.\d+` from the start of `text`.
///
/// # Arguments
/// * `text` - text starting at the mantissa
///
/// # Returns
/// The mantissa text and the remaining text, or `None` if there is no mantissa.
fn split_mantissa(text: &str) -> Option<(&str, &str)> {
    let (integer, after_integer) = split_digits(text);
    let (fraction, rest) = after_integer
        .strip_prefix('.')
        .map(split_digits)
        .map_or(("", after_integer), |(digits, rest)| (digits, rest));
    let has_dot = after_integer.len() != rest.len() || after_integer.starts_with('.');
    let consumed = integer.len() + usize::from(has_dot) + fraction.len();
    (!integer.is_empty() || !fraction.is_empty()).then(|| text.split_at(consumed))
}

/// Splits an exponent (`e`/`d`, optional sign, digits) from the start of `text`.
///
/// # Arguments
/// * `text` - text following the mantissa
///
/// # Returns
/// The normalized exponent (`-3`, `5`) if present, and the remaining text.
fn split_exponent(text: &str) -> (Option<String>, &str) {
    let Some(after_marker) = text.strip_prefix(['e', 'E', 'd', 'D']) else {
        return (None, text);
    };
    let (sign, after_sign) = match after_marker.strip_prefix(['+', '-']) {
        Some(rest) => (&after_marker[..1], rest),
        None => ("", after_marker),
    };
    let (digits, rest) = split_digits(after_sign);
    if digits.is_empty() {
        return (None, text);
    }
    (Some(format!("{}{digits}", sign.replace('+', ""))), rest)
}

/// Recognises a scale factor at the start of the letters following a number.
///
/// # Arguments
/// * `letters` - text after mantissa and exponent
///
/// # Returns
/// The scale factor and the remaining text; the scale is `None` if the letters are plain unit text.
fn split_scale(letters: &str) -> (Option<ScaleFactor>, &str) {
    let lower = letters.to_ascii_lowercase();
    if lower.starts_with("meg") {
        return (Some(MEG), &letters[3..]);
    }
    if lower.starts_with("mil") {
        return (Some(MIL), &letters[3..]);
    }
    let mut chars = lower.chars();
    let (Some(first), next) = (chars.next(), chars.next()) else {
        return (None, letters);
    };
    if first == 'r' && next.is_some_and(|c| c.is_ascii_digit()) {
        return (Some(INFIX_R), &letters[1..]);
    }
    if first == 'a' && next.is_some_and(|c| c.is_ascii_alphabetic()) {
        return (None, letters);
    }
    SINGLE_LETTER_SCALES
        .iter()
        .find(|scale| scale.suffix.starts_with(first))
        .map_or((None, letters), |scale| (Some(*scale), &letters[1..]))
}

/// Returns true if `text` consists only of ASCII letters.
///
/// # Arguments
/// * `text` - text to check
fn is_unit_text(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii_alphabetic())
}

/// Decodes a numeric literal such as `1.5meg`, `10pF`, `1e-3`, `3k3` or `-2u`.
///
/// # Arguments
/// * `text` - the literal as written, including sign and trailing letters
///
/// # Returns
/// The decoded number, or `None` if `text` is not a valid numeric literal.
pub fn parse_number(text: &str) -> Option<ParsedNumber> {
    let (negative, unsigned) = match text.strip_prefix(['+', '-']) {
        Some(rest) => (text.starts_with('-'), rest),
        None => (false, text),
    };
    let (mantissa, after_mantissa) = split_mantissa(unsigned)?;
    let (exponent, letters) = split_exponent(after_mantissa);
    let (scale, after_scale) = split_scale(letters);
    let multiplier = scale.map_or(1.0, |factor| factor.multiplier);
    let (digits, after_digits) = split_digits(after_scale);
    let (base, unit) = if digits.is_empty() {
        (decimal_value(mantissa, exponent.as_deref())?, after_scale)
    } else {
        let infix_allowed = scale.is_some() && exponent.is_none() && !mantissa.contains('.');
        let value = infix_allowed
            .then(|| decimal_value(&format!("{mantissa}.{digits}"), None))
            .flatten()?;
        (value, after_digits)
    };
    let value = base * multiplier * if negative { -1.0 } else { 1.0 };
    (is_unit_text(unit) && value.is_finite()).then(|| ParsedNumber {
        value,
        scale,
        has_exponent: exponent.is_some(),
        unit: unit.to_string(),
    })
}

/// Converts mantissa and optional exponent text to a float.
///
/// # Arguments
/// * `mantissa` - digits with optional decimal point
/// * `exponent` - normalized exponent text
fn decimal_value(mantissa: &str, exponent: Option<&str>) -> Option<f64> {
    let trimmed = mantissa.strip_suffix('.').unwrap_or(mantissa);
    format!("{trimmed}e{}", exponent.unwrap_or("0"))
        .parse()
        .ok()
}

/// Formats a float with at most `decimals` fraction digits, trimming trailing zeros.
///
/// # Arguments
/// * `value` - number to format
/// * `decimals` - maximum number of fraction digits
fn trim_decimal(value: f64, decimals: usize) -> String {
    let formatted = format!("{value:.decimals$}");
    let trimmed = if formatted.contains('.') {
        formatted.trim_end_matches('0').trim_end_matches('.')
    } else {
        formatted.as_str()
    };
    match trimmed {
        "-0" | "" => "0".to_string(),
        other => other.to_string(),
    }
}

/// Formats a value in scientific notation like `1.23e+15`.
///
/// # Arguments
/// * `value` - number to format
fn format_scientific(value: f64) -> String {
    let formatted = format!("{value:.2e}");
    let Some((mantissa, exponent)) = formatted.split_once('e') else {
        return formatted;
    };
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    let sign = if exponent.starts_with('-') { "" } else { "+" };
    format!("{mantissa}e{sign}{exponent}")
}

/// Renders a value with an SI suffix (`t g meg k`, none, `m u n p f a`) and up to 4 significant digits.
///
/// # Arguments
/// * `value` - number to render
///
/// # Returns
/// Text such as `2.2k`, `10m` or `1.23e+15` for values outside the suffix range.
pub fn format_engineering(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if !value.is_finite() {
        return value.to_string();
    }
    let rounded: f64 = format!("{value:.3e}").parse().unwrap_or(value);
    ENGINEERING_PREFIXES
        .iter()
        .find(|(factor, _)| rounded.abs() >= *factor)
        .filter(|_| rounded.abs() < 1e15)
        .map_or_else(
            || format_scientific(value),
            |(factor, prefix)| format!("{}{prefix}", trim_decimal(rounded / factor, 3)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() <= expected.abs() * 1e-12
    }

    fn value_of(text: &str) -> f64 {
        parse_number(text).map_or(f64::NAN, |parsed| parsed.value)
    }

    fn assert_value(text: &str, expected: f64) {
        let actual = value_of(text);
        assert!(close(actual, expected), "{text}: {actual} != {expected}");
    }

    #[test]
    fn plain_numbers() {
        assert_value("1", 1.0);
        assert_value("1.5", 1.5);
        assert_value(".5", 0.5);
        assert_value("5.", 5.0);
        assert_value("-2.5", -2.5);
        assert_value("+3", 3.0);
    }

    #[test]
    fn exponents() {
        assert_value("1e3", 1000.0);
        assert_value("1E-3", 1e-3);
        assert_value("1D-3", 1e-3);
        assert_value("2.5e+2", 250.0);
        let parsed = parse_number("1e3");
        assert_eq!(parsed.map(|p| p.has_exponent), Some(true));
        assert_eq!(parse_number("1").map(|p| p.has_exponent), Some(false));
    }

    #[test]
    fn exponent_without_digits_is_unit_text() {
        let parsed = parse_number("1e");
        assert_eq!(parsed.as_ref().map(|p| p.value), Some(1.0));
        assert_eq!(parsed.map(|p| p.unit), Some("e".to_string()));
    }

    #[test]
    fn scale_factors() {
        assert_value("1t", 1e12);
        assert_value("1g", 1e9);
        assert_value("1meg", 1e6);
        assert_value("1MEG", 1e6);
        assert_value("1x", 1e6);
        assert_value("2k", 2e3);
        assert_value("1mil", 25.4e-6);
        assert_value("1m", 1e-3);
        assert_value("1u", 1e-6);
        assert_value("1n", 1e-9);
        assert_value("1p", 1e-12);
        assert_value("1f", 1e-15);
        assert_value("1a", 1e-18);
        assert_value("1.5K", 1500.0);
        assert_value("-1u", -1e-6);
    }

    #[test]
    fn scale_suffix_is_reported() {
        let parsed = parse_number("10MEG");
        assert_eq!(parsed.and_then(|p| p.scale).map(|s| s.suffix), Some("meg"));
        assert_eq!(parse_number("10").and_then(|p| p.scale), None);
    }

    #[test]
    fn units_are_ignored() {
        let parsed = parse_number("10pF");
        assert_eq!(parsed.as_ref().map(|p| p.unit.as_str()), Some("F"));
        assert_value("10pF", 10e-12);
        assert_value("1kV", 1e3);
        assert_value("1.5VOLTS", 1.5);
        assert_value("1w", 1.0);
        let volts = parse_number("1V");
        assert_eq!(volts.as_ref().map(|p| p.value), Some(1.0));
        assert_eq!(volts.and_then(|p| p.scale), None);
    }

    #[test]
    fn milli_versus_mega_and_mil() {
        let siemens = parse_number("1MS");
        assert_eq!(siemens.as_ref().map(|p| p.unit.as_str()), Some("S"));
        assert_value("1MS", 1e-3);
        assert_value("1megohm", 1e6);
        assert_eq!(parse_number("1megohm").map(|p| p.unit), Some("ohm".into()));
        assert_value("10M", 1e-2);
    }

    #[test]
    fn a_followed_by_letters_is_unit() {
        let parsed = parse_number("20amps");
        assert_eq!(parsed.as_ref().map(|p| p.value), Some(20.0));
        assert_eq!(parsed.as_ref().and_then(|p| p.scale), None);
        assert_eq!(parsed.map(|p| p.unit), Some("amps".into()));
    }

    #[test]
    fn exponent_with_scale_is_still_parsed() {
        let parsed = parse_number("1e-6u");
        assert_eq!(parsed.as_ref().map(|p| p.has_exponent), Some(true));
        assert_value("1e-6u", 1e-12);
    }

    #[test]
    fn pcb_infix_forms() {
        assert_value("3k3", 3300.0);
        assert_value("0u1", 1e-7);
        assert_value("5R6", 5.6);
        assert_value("5MEG35", 5.35e6);
        let parsed = parse_number("5R6");
        assert_eq!(parsed.and_then(|p| p.scale).map(|s| s.suffix), Some("r"));
    }

    #[test]
    fn infix_requires_integer_mantissa() {
        assert!(parse_number("1.5k3").is_none());
        assert!(parse_number("5v6").is_none());
    }

    #[test]
    fn invalid_literals() {
        for text in [
            "", "abc", "1.2.3", "1u-", ".", "-", "+", "k", "1 2", "1µ", "--1",
        ] {
            assert!(parse_number(text).is_none(), "{text:?} should be rejected");
        }
    }

    #[test]
    fn bare_r_is_unit_text() {
        let parsed = parse_number("1r");
        assert_eq!(parsed.as_ref().and_then(|p| p.scale), None);
        assert_eq!(parsed.map(|p| p.unit), Some("r".into()));
    }

    #[test]
    fn overflow_is_rejected() {
        assert!(parse_number("1e999").is_none());
    }

    #[test]
    fn engineering_basic() {
        assert_eq!(format_engineering(1e6), "1meg");
        assert_eq!(format_engineering(0.01), "10m");
        assert_eq!(format_engineering(2200.0), "2.2k");
        assert_eq!(format_engineering(0.0), "0");
        assert_eq!(format_engineering(-1.5e-9), "-1.5n");
        assert_eq!(format_engineering(1.0), "1");
        assert_eq!(format_engineering(45e-9), "45n");
        assert_eq!(format_engineering(1e-15), "1f");
        assert_eq!(format_engineering(1e12), "1t");
    }

    #[test]
    fn engineering_rounding() {
        assert_eq!(format_engineering(1.23456e3), "1.235k");
        assert_eq!(format_engineering(999.99), "1k");
        assert_eq!(format_engineering(0.5), "500m");
        assert_eq!(format_engineering(3.3), "3.3");
    }

    #[test]
    fn engineering_extremes() {
        assert_eq!(format_engineering(1.23e15), "1.23e+15");
        assert_eq!(format_engineering(1e-20), "1e-20");
        assert_eq!(format_engineering(-4.56e-19), "-4.56e-19");
        assert_eq!(format_engineering(f64::INFINITY), "inf");
    }
}
