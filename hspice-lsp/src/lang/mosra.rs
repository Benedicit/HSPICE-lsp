//! Parameters of the `.MOSRA` reliability analysis command.

use super::{KeywordSpec, find_named};

/// `key=value` parameters of `.MOSRA`.
pub static MOSRA_PARAMS: &[KeywordSpec] = &[
    KeywordSpec {
        name: "reltotaltime",
        aliases: &[],
        takes_value: true,
        description: "Final reliability time of the post-stress simulation (required)",
    },
    KeywordSpec {
        name: "relstarttime",
        aliases: &[],
        takes_value: true,
        description: "First post-stress time point (default 0)",
    },
    KeywordSpec {
        name: "dec",
        aliases: &[],
        takes_value: true,
        description: "Post-stress time points per decade (overrides RELSTEP)",
    },
    KeywordSpec {
        name: "lin",
        aliases: &[],
        takes_value: true,
        description: "Number of linear post-stress time points from RELSTARTTIME to RELTOTALTIME (overrides RELSTEP)",
    },
    KeywordSpec {
        name: "relstep",
        aliases: &[],
        takes_value: true,
        description: "Post-stress time step: RELSTEP, 2*RELSTEP, ... up to RELTOTALTIME (default RELTOTALTIME)",
    },
    KeywordSpec {
        name: "relmode",
        aliases: &[],
        takes_value: true,
        description: "Degradation effects: 0 HCI and BTI (default), 1 HCI only, 2 BTI only",
    },
    KeywordSpec {
        name: "simmode",
        aliases: &[],
        takes_value: true,
        description: "0 pre-stress only, 1 post-stress only (reads *.radeg0), 2 both, 3 continual integration through .ALTER",
    },
    KeywordSpec {
        name: "agingstart",
        aliases: &[],
        takes_value: true,
        description: "Transient time at which stress calculation starts (default 0)",
    },
    KeywordSpec {
        name: "agingstop",
        aliases: &[],
        takes_value: true,
        description: "Transient time at which stress calculation stops (default: tstop of .TRAN)",
    },
    KeywordSpec {
        name: "agingperiod",
        aliases: &[],
        takes_value: true,
        description: "Stress period that scales the total degradation over time",
    },
    KeywordSpec {
        name: "agingwidth",
        aliases: &[],
        takes_value: true,
        description: "Circuit on-time within AGINGPERIOD",
    },
    KeywordSpec {
        name: "aginginst",
        aliases: &[],
        takes_value: true,
        description: "Quoted MOSFET instance names to age; wildcards allowed (default: all with a MOSRA model)",
    },
    KeywordSpec {
        name: "hcithreshold",
        aliases: &[],
        takes_value: true,
        description: "Post-stress HCI threshold per transistor (default 0)",
    },
    KeywordSpec {
        name: "nbtithreshold",
        aliases: &[],
        takes_value: true,
        description: "Post-stress BTI threshold per transistor (default 0)",
    },
    KeywordSpec {
        name: "integmod",
        aliases: &[],
        takes_value: true,
        description: "Integration method: 0 MOSRA API function, 1 true derivative and integration, 2 linearized",
    },
    KeywordSpec {
        name: "xpolatemod",
        aliases: &[],
        takes_value: true,
        description: "Extrapolation method: 0 MOSRA API function, 1 linearized, 2 two-point fitting",
    },
    KeywordSpec {
        name: "tsample1",
        aliases: &[],
        takes_value: true,
        description: "First stress sampling time for XPOLATEMOD=2",
    },
    KeywordSpec {
        name: "tsample2",
        aliases: &[],
        takes_value: true,
        description: "Second stress sampling time for XPOLATEMOD=2",
    },
    KeywordSpec {
        name: "agethreshold",
        aliases: &[],
        takes_value: true,
        description: "Only MOSFETs with degradation at or above this value are written to the output (default 0)",
    },
    KeywordSpec {
        name: "degradationtime",
        aliases: &[],
        takes_value: true,
        description: "Time at which the MOSRA API computes the degradation (writes a .degradation file)",
    },
    KeywordSpec {
        name: "mosralife",
        aliases: &[],
        takes_value: true,
        description: "Compute device lifetime for this degradation type (as .OPTION MOSRALIFE)",
    },
    KeywordSpec {
        name: "degf",
        aliases: &[],
        takes_value: true,
        description: "Failure criterion for lifetime computation (.OPTION DEGF takes precedence)",
    },
    KeywordSpec {
        name: "degfn",
        aliases: &[],
        takes_value: true,
        description: "NMOS failure criterion for lifetime computation (.OPTION DEGFN takes precedence)",
    },
    KeywordSpec {
        name: "degfp",
        aliases: &[],
        takes_value: true,
        description: "PMOS failure criterion for lifetime computation (.OPTION DEGFP takes precedence)",
    },
];

/// Looks up a `.MOSRA` parameter by name (case-insensitive).
///
/// # Arguments
/// * `name` - parameter as written
pub fn find_mosra_param(name: &str) -> Option<&'static KeywordSpec> {
    find_named(MOSRA_PARAMS, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_transient_aging_parameters() {
        assert!(find_mosra_param("RelTotalTime").is_some());
        assert!(find_mosra_param("relstep").is_some());
        assert!(find_mosra_param("bogus").is_none());
        assert!(find_mosra_param("DegradationTime").is_some());
        assert_eq!(MOSRA_PARAMS.len(), 24);
    }
}
