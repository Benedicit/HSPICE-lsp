//! Keywords for `.measure`, analysis statements and output analysis types.

use super::{KeywordSpec, find_named};

/// Analysis types usable in `.print`, `.probe` and `.measure` (`tran`, `ac`, `dc` …).
pub static ANALYSIS_TYPES: &[KeywordSpec] = &[
    KeywordSpec {
        name: "dc",
        aliases: &[],
        takes_value: false,
        description: "DC sweep results",
    },
    KeywordSpec {
        name: "ac",
        aliases: &[],
        takes_value: false,
        description: "AC small-signal results",
    },
    KeywordSpec {
        name: "tran",
        aliases: &[],
        takes_value: false,
        description: "Transient results",
    },
    KeywordSpec {
        name: "noise",
        aliases: &[],
        takes_value: false,
        description: "Noise analysis results",
    },
    KeywordSpec {
        name: "disto",
        aliases: &[],
        takes_value: false,
        description: "Distortion analysis results",
    },
    KeywordSpec {
        name: "dc_cont",
        aliases: &[],
        takes_value: false,
        description: "DC results with continuous sweep (measure per sweep point)",
    },
    KeywordSpec {
        name: "ac_cont",
        aliases: &[],
        takes_value: false,
        description: "AC results with continuous sweep",
    },
    KeywordSpec {
        name: "tran_cont",
        aliases: &[],
        takes_value: false,
        description: "Transient results with continuous sweep",
    },
    KeywordSpec {
        name: "fft",
        aliases: &[],
        takes_value: false,
        description: "FFT results",
    },
    KeywordSpec {
        name: "hb",
        aliases: &[],
        takes_value: false,
        description: "Harmonic balance results",
    },
    KeywordSpec {
        name: "hbac",
        aliases: &[],
        takes_value: false,
        description: "Harmonic balance AC results",
    },
    KeywordSpec {
        name: "hbnoise",
        aliases: &[],
        takes_value: false,
        description: "Harmonic balance noise results",
    },
    KeywordSpec {
        name: "hbxf",
        aliases: &[],
        takes_value: false,
        description: "Harmonic balance transfer function results",
    },
    KeywordSpec {
        name: "env",
        aliases: &[],
        takes_value: false,
        description: "Envelope analysis results",
    },
    KeywordSpec {
        name: "trannoise",
        aliases: &[],
        takes_value: false,
        description: "Transient noise results",
    },
    KeywordSpec {
        name: "phasenoise",
        aliases: &[],
        takes_value: false,
        description: "Phase noise results",
    },
];

/// `.measure` body keywords (`trig`, `targ`, `when`, `find`, `rise=` …).
pub static MEASURE_KEYWORDS: &[KeywordSpec] = &[
    KeywordSpec {
        name: "trig",
        aliases: &[],
        takes_value: false,
        description: "Trigger event start of a delay measurement",
    },
    KeywordSpec {
        name: "targ",
        aliases: &[],
        takes_value: false,
        description: "Target event end of a delay measurement",
    },
    KeywordSpec {
        name: "val",
        aliases: &[],
        takes_value: true,
        description: "Voltage or current level of an event",
    },
    KeywordSpec {
        name: "td",
        aliases: &[],
        takes_value: true,
        description: "Delay before event detection starts",
    },
    KeywordSpec {
        name: "rise",
        aliases: &[],
        takes_value: true,
        description: "Rising-edge count (or LAST)",
    },
    KeywordSpec {
        name: "fall",
        aliases: &[],
        takes_value: true,
        description: "Falling-edge count (or LAST)",
    },
    KeywordSpec {
        name: "cross",
        aliases: &[],
        takes_value: true,
        description: "Crossing count (or LAST)",
    },
    KeywordSpec {
        name: "last",
        aliases: &[],
        takes_value: false,
        description: "Use the last event; reserved word",
    },
    KeywordSpec {
        name: "at",
        aliases: &[],
        takes_value: true,
        description: "Evaluate at a fixed time, frequency or sweep value",
    },
    KeywordSpec {
        name: "when",
        aliases: &[],
        takes_value: false,
        description: "Evaluate when a condition holds",
    },
    KeywordSpec {
        name: "find",
        aliases: &[],
        takes_value: false,
        description: "Value of a quantity at an event",
    },
    KeywordSpec {
        name: "from",
        aliases: &[],
        takes_value: true,
        description: "Start of the evaluation window",
    },
    KeywordSpec {
        name: "to",
        aliases: &[],
        takes_value: true,
        description: "End of the evaluation window",
    },
    KeywordSpec {
        name: "avg",
        aliases: &[],
        takes_value: false,
        description: "Average over the window",
    },
    KeywordSpec {
        name: "rms",
        aliases: &[],
        takes_value: false,
        description: "Root mean square over the window",
    },
    KeywordSpec {
        name: "min",
        aliases: &[],
        takes_value: false,
        description: "Minimum over the window",
    },
    KeywordSpec {
        name: "max",
        aliases: &[],
        takes_value: false,
        description: "Maximum over the window",
    },
    KeywordSpec {
        name: "pp",
        aliases: &[],
        takes_value: false,
        description: "Peak-to-peak over the window",
    },
    KeywordSpec {
        name: "em_avg",
        aliases: &[],
        takes_value: false,
        description: "Electromigration average",
    },
    KeywordSpec {
        name: "integ",
        aliases: &["integral"],
        takes_value: false,
        description: "Integral over the window",
    },
    KeywordSpec {
        name: "deriv",
        aliases: &["derivative"],
        takes_value: false,
        description: "Derivative at a point or event",
    },
    KeywordSpec {
        name: "err",
        aliases: &[],
        takes_value: false,
        description: "Error function between measured and calculated data",
    },
    KeywordSpec {
        name: "err1",
        aliases: &[],
        takes_value: false,
        description: "Relative error function",
    },
    KeywordSpec {
        name: "err2",
        aliases: &[],
        takes_value: false,
        description: "Normalized error function",
    },
    KeywordSpec {
        name: "err3",
        aliases: &[],
        takes_value: false,
        description: "Absolute error function",
    },
    KeywordSpec {
        name: "param",
        aliases: &[],
        takes_value: true,
        description: "Expression of earlier measure results",
    },
    KeywordSpec {
        name: "goal",
        aliases: &[],
        takes_value: true,
        description: "Optimization target value",
    },
    KeywordSpec {
        name: "minval",
        aliases: &[],
        takes_value: true,
        description: "Lower bound used in the error calculation",
    },
    KeywordSpec {
        name: "weight",
        aliases: &[],
        takes_value: true,
        description: "Weight in optimization",
    },
    KeywordSpec {
        name: "print",
        aliases: &[],
        takes_value: true,
        description: "Print the result (0 or 1)",
    },
    KeywordSpec {
        name: "reverse",
        aliases: &[],
        takes_value: false,
        description: "Search events in reverse order",
    },
    KeywordSpec {
        name: "ignor",
        aliases: &[],
        takes_value: true,
        description: "Ignore data below this value in error calculation",
    },
    KeywordSpec {
        name: "ymin",
        aliases: &[],
        takes_value: true,
        description: "Ignore data below this value in error calculation",
    },
    KeywordSpec {
        name: "ymax",
        aliases: &[],
        takes_value: true,
        description: "Ignore data above this value in error calculation",
    },
    KeywordSpec {
        name: "nbharm",
        aliases: &[],
        takes_value: true,
        description: "Number of harmonics for FFT measures",
    },
    KeywordSpec {
        name: "maxfreq",
        aliases: &[],
        takes_value: true,
        description: "Maximum frequency for FFT measures",
    },
    KeywordSpec {
        name: "binsiz",
        aliases: &[],
        takes_value: true,
        description: "Bin size for FFT measures",
    },
    KeywordSpec {
        name: "pushout",
        aliases: &[],
        takes_value: true,
        description: "Pushout tolerance for pass/fail optimization",
    },
    KeywordSpec {
        name: "pushout_per",
        aliases: &[],
        takes_value: true,
        description: "Relative pushout tolerance",
    },
    KeywordSpec {
        name: "thd",
        aliases: &[],
        takes_value: false,
        description: "FFT total harmonic distortion",
    },
    KeywordSpec {
        name: "snr",
        aliases: &[],
        takes_value: false,
        description: "FFT signal-to-noise ratio",
    },
    KeywordSpec {
        name: "sndr",
        aliases: &[],
        takes_value: false,
        description: "FFT signal-to-noise-and-distortion ratio",
    },
    KeywordSpec {
        name: "enob",
        aliases: &[],
        takes_value: false,
        description: "FFT effective number of bits",
    },
];

/// Keywords of `.tran`/`.ac`/`.dc`/`.op` and sweep specs (`dec`, `sweep`, `uic`, `monte=` …).
pub static ANALYSIS_KEYWORDS: &[KeywordSpec] = &[
    KeywordSpec {
        name: "dec",
        aliases: &[],
        takes_value: false,
        description: "Decade sweep with points per decade",
    },
    KeywordSpec {
        name: "oct",
        aliases: &[],
        takes_value: false,
        description: "Octave sweep with points per octave",
    },
    KeywordSpec {
        name: "lin",
        aliases: &[],
        takes_value: false,
        description: "Linear sweep with number of points",
    },
    KeywordSpec {
        name: "poi",
        aliases: &[],
        takes_value: false,
        description: "List of points follows",
    },
    KeywordSpec {
        name: "sweep",
        aliases: &[],
        takes_value: false,
        description: "Start of a sweep specification",
    },
    KeywordSpec {
        name: "uic",
        aliases: &[],
        takes_value: false,
        description: "Use initial conditions, skip the DC operating point",
    },
    KeywordSpec {
        name: "start",
        aliases: &[],
        takes_value: true,
        description: "Start value of the analysis or sweep",
    },
    KeywordSpec {
        name: "stop",
        aliases: &[],
        takes_value: true,
        description: "Stop value of the analysis or sweep",
    },
    KeywordSpec {
        name: "step",
        aliases: &[],
        takes_value: true,
        description: "Step size of the sweep",
    },
    KeywordSpec {
        name: "data",
        aliases: &[],
        takes_value: true,
        description: "Sweep over a .DATA table",
    },
    KeywordSpec {
        name: "monte",
        aliases: &[],
        takes_value: true,
        description: "Monte Carlo iterations (number or list)",
    },
    KeywordSpec {
        name: "firstrun",
        aliases: &[],
        takes_value: true,
        description: "Index of the first Monte Carlo run",
    },
    KeywordSpec {
        name: "optimize",
        aliases: &[],
        takes_value: true,
        description: "Optimization sweep using an OPTxxx parameter",
    },
    KeywordSpec {
        name: "results",
        aliases: &[],
        takes_value: true,
        description: "Measure names used as optimization results",
    },
    KeywordSpec {
        name: "model",
        aliases: &[],
        takes_value: true,
        description: "Optimization model name",
    },
    KeywordSpec {
        name: "temp",
        aliases: &[],
        takes_value: false,
        description: "Temperature sweep variable",
    },
    KeywordSpec {
        name: "interpolation",
        aliases: &[],
        takes_value: false,
        description: "Interpolate .OP output at requested times",
    },
    KeywordSpec {
        name: "all",
        aliases: &[],
        takes_value: false,
        description: ".OP: print all operating-point information",
    },
    KeywordSpec {
        name: "brief",
        aliases: &[],
        takes_value: false,
        description: ".OP: brief operating-point output",
    },
    KeywordSpec {
        name: "current",
        aliases: &[],
        takes_value: false,
        description: ".OP: print element currents",
    },
    KeywordSpec {
        name: "debug",
        aliases: &[],
        takes_value: false,
        description: ".OP: print debug information",
    },
    KeywordSpec {
        name: "none",
        aliases: &[],
        takes_value: false,
        description: ".OP: print nothing",
    },
    KeywordSpec {
        name: "voltage",
        aliases: &[],
        takes_value: false,
        description: ".OP: print node voltages",
    },
    KeywordSpec {
        name: "tempvec",
        aliases: &[],
        takes_value: true,
        description: "Temperature vector for .TRAN",
    },
    KeywordSpec {
        name: "runlvl",
        aliases: &[],
        takes_value: true,
        description: "Run level for the analysis",
    },
];

/// Names a `.measure` result may not use (`dc`, `ac`, `tran`, `last`).
pub const RESERVED_MEASURE_NAMES: &[&str] = &["dc", "ac", "tran", "last"];

/// Looks up a `.measure` keyword by name (case-insensitive).
///
/// # Arguments
/// * `name` - keyword as written
pub fn find_measure_keyword(name: &str) -> Option<&'static KeywordSpec> {
    find_named(MEASURE_KEYWORDS, name)
}

/// Looks up an analysis or sweep keyword by name (case-insensitive).
///
/// # Arguments
/// * `name` - keyword as written
pub fn find_analysis_keyword(name: &str) -> Option<&'static KeywordSpec> {
    find_named(ANALYSIS_KEYWORDS, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finds_keywords_by_name_and_alias() {
        assert_eq!(find_measure_keyword("TRIG").map(|k| k.name), Some("trig"));
        assert_eq!(
            find_measure_keyword("Integral").map(|k| k.name),
            Some("integ")
        );
        assert!(find_measure_keyword("rise").is_some_and(|k| k.takes_value));
        assert!(find_analysis_keyword("UIC").is_some_and(|k| !k.takes_value));
        assert!(find_analysis_keyword("monte").is_some_and(|k| k.takes_value));
        assert!(find_analysis_keyword("bogus").is_none());
    }

    #[test]
    fn analysis_types_present() {
        assert_eq!(ANALYSIS_TYPES.len(), 16);
        assert!(find_named(ANALYSIS_TYPES, "TRAN").is_some());
    }

    #[test]
    fn tables_are_lowercase_unique_and_described() {
        for table in [ANALYSIS_TYPES, MEASURE_KEYWORDS, ANALYSIS_KEYWORDS] {
            let names: Vec<&str> = table
                .iter()
                .flat_map(|k| std::iter::once(k.name).chain(k.aliases.iter().copied()))
                .collect();
            assert_eq!(names.len(), names.iter().collect::<HashSet<_>>().len());
            assert!(names.iter().all(|n| *n == n.to_ascii_lowercase()));
            assert!(table.iter().all(|k| !k.description.is_empty()));
        }
    }
}
