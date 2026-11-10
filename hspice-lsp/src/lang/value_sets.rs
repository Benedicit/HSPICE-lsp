//! Documented literal values of `key=value` arguments of `.option`, `.mosra`, `.fft` and `.store`.

use super::options::find_option;

/// Values accepted by one `key=value` argument of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueSet {
    /// Canonical lowercase command name.
    pub command: &'static str,
    /// Lowercase key (canonical option name for `.option`).
    pub key: &'static str,
    /// Accepted values in lowercase.
    pub values: &'static [&'static str],
}

/// Builds one `ValueSet`.
macro_rules! values {
    ($command:literal, $key:literal, [$($value:literal),+]) => {
        ValueSet {
            command: $command,
            key: $key,
            values: &[$($value),+],
        }
    };
}

/// Value sets with more than an on/off choice.
pub static VALUE_SETS: &[ValueSet] = &[
    values!(
        ".option",
        "post",
        ["0", "1", "2", "3", "ascii", "binary", "csdf"]
    ),
    values!(".option", "method", ["gear", "trap", "bdf"]),
    values!(".option", "runlvl", ["0", "1", "2", "3", "4", "5", "6"]),
    values!(".option", "parhier", ["global", "local"]),
    values!(".option", "optparhier", ["global", "local"]),
    values!(".option", "ingold", ["0", "1", "2"]),
    values!(".option", "measform", ["0", "1", "2", "3"]),
    values!(".option", "list", ["0", "1", "2", "3"]),
    values!(".option", "optlst", ["0", "1", "2", "3"]),
    values!(".option", "opfile", ["0", "1", "2"]),
    values!(".option", "psf", ["0", "1", "2"]),
    values!(".option", "artist", ["0", "1", "2"]),
    values!(".option", "redefsub", ["0", "1", "2"]),
    values!(".option", "altcc", ["-1", "0", "1"]),
    values!(".option", "macmod", ["0", "1", "2", "3"]),
    values!(".option", "bypass", ["0", "1", "2"]),
    values!(".option", "dvdt", ["0", "1", "2", "3", "4"]),
    values!(".option", "lvltim", ["1", "2", "3", "4"]),
    values!(".option", "maxord", ["1", "2", "3"]),
    values!(
        ".option",
        "converge",
        ["-1", "0", "1", "2", "3", "4", "5", "100"]
    ),
    values!(
        ".option",
        "sampling_method",
        [
            "srs",
            "lhs",
            "factorial",
            "ofat",
            "sobol",
            "niederreiter",
            "external"
        ]
    ),
    values!(".option", "set_missing_values", ["random", "zero"]),
    values!(".option", "gen_cur_pol", ["on", "off"]),
    values!(".option", "radegoutput", ["csv"]),
    values!(".option", "sim_la", ["pact", "pi", "0", "1", "2"]),
    values!(".option", "wdelayopt", ["0", "1", "2", "3"]),
    values!(".mosra", "relmode", ["0", "1", "2"]),
    values!(".mosra", "simmode", ["0", "1", "2", "3"]),
    values!(".mosra", "integmod", ["0", "1", "2"]),
    values!(".mosra", "xpolatemod", ["0", "1", "2"]),
    values!(
        ".fft",
        "window",
        [
            "rect", "bart", "hann", "hamm", "black", "harris", "gauss", "kaiser"
        ]
    ),
    values!(".fft", "format", ["norm", "unorm"]),
    values!(".store", "type", ["ic", "memdump"]),
];

/// Returns the documented values of a `key=value` argument.
///
/// # Arguments
/// * `command` - canonical lowercase command name
/// * `key` - key as written (option aliases are accepted)
pub fn values_of(command: &str, key: &str) -> Option<&'static [&'static str]> {
    let option = (command == ".option").then(|| find_option(key)).flatten();
    let key = option.map_or(key, |spec| spec.name);
    VALUE_SETS
        .iter()
        .find(|set| set.command == command && set.key.eq_ignore_ascii_case(key))
        .map(|set| set.values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_values_by_key_and_alias() {
        assert_eq!(
            values_of(".option", "METHOD"),
            Some(&["gear", "trap", "bdf"][..])
        );
        assert!(values_of(".option", "parhie").is_some_and(|values| values.contains(&"local")));
        assert!(values_of(".mosra", "SimMode").is_some_and(|values| values.len() == 4));
        assert!(values_of(".option", "reltol").is_none());
        assert!(values_of(".tran", "method").is_none());
    }

    #[test]
    fn option_keys_are_canonical_option_names() {
        let options = VALUE_SETS.iter().filter(|set| set.command == ".option");
        options.for_each(|set| {
            assert_eq!(find_option(set.key).map(|spec| spec.name), Some(set.key));
        });
        assert!(
            VALUE_SETS
                .iter()
                .flat_map(|set| set.values)
                .all(|value| *value == value.to_ascii_lowercase())
        );
    }
}
