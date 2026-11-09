//! `.model` types, MOSFET LEVEL names and common model parameters.

use super::mosra_model::MOSRA_MODEL_PARAMS;
use super::{Named, Status, find_named};

/// Static description of a `.model` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTypeSpec {
    /// Lowercase type name, e.g. `nmos`.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Uppercase element letters that may reference this model type.
    pub element_letters: &'static [char],
    /// Support status (`sw`/`csw` are foreign).
    pub status: Status,
}

impl Named for ModelTypeSpec {
    fn name(&self) -> &'static str {
        self.name
    }
}

/// A common model-card parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelParamSpec {
    /// Lowercase parameter name.
    pub name: &'static str,
    /// Default value as documented, if any.
    pub default: Option<&'static str>,
    /// One-line description.
    pub description: &'static str,
}

/// All `.model` types.
pub static MODEL_TYPES: &[ModelTypeSpec] = &[
    ModelTypeSpec {
        name: "amp",
        description: "Behavioral amplifier model",
        element_letters: &[],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "c",
        description: "Capacitor model",
        element_letters: &['C'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "core",
        description: "Magnetic core model",
        element_letters: &['K'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "d",
        description: "Diode model",
        element_letters: &['D'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "l",
        description: "Inductor model",
        element_letters: &['L'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "njf",
        description: "N-channel JFET model",
        element_letters: &['J'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "pjf",
        description: "P-channel JFET model",
        element_letters: &['J'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "nmos",
        description: "N-channel MOSFET model",
        element_letters: &['M'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "pmos",
        description: "P-channel MOSFET model",
        element_letters: &['M'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "npn",
        description: "NPN bipolar transistor model",
        element_letters: &['Q'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "pnp",
        description: "PNP bipolar transistor model",
        element_letters: &['Q'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "opt",
        description: "Optimization model",
        element_letters: &[],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "r",
        description: "Resistor model",
        element_letters: &['R'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "u",
        description: "Lossy transmission line model (U and T elements)",
        element_letters: &['T', 'U'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "w",
        description: "W-element transmission line model",
        element_letters: &['W'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "s",
        description: "S-parameter model",
        element_letters: &['S'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "sp",
        description: "S-parameter (SP) model",
        element_letters: &['S'],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "mosra",
        description: "MOSFET reliability analysis model",
        element_letters: &[],
        status: Status::Supported,
    },
    ModelTypeSpec {
        name: "plot",
        description: "Plot model (obsolete)",
        element_letters: &[],
        status: Status::Obsolete,
    },
    ModelTypeSpec {
        name: "plotq",
        description: "Plot model (obsolete)",
        element_letters: &[],
        status: Status::Obsolete,
    },
    ModelTypeSpec {
        name: "sw",
        description: "Voltage-controlled switch; not HSPICE (PSpice/ngspice)",
        element_letters: &[],
        status: Status::Foreign,
    },
    ModelTypeSpec {
        name: "csw",
        description: "Current-controlled switch; not HSPICE (PSpice/ngspice)",
        element_letters: &[],
        status: Status::Foreign,
    },
];

/// MOSFET `LEVEL` numbers and the model they select.
pub static MOSFET_LEVELS: &[(u32, &str)] = &[
    (1, "Shichman-Hodges"),
    (2, "Grove-Frohman (SPICE 2G)"),
    (3, "SPICE 2G empirical"),
    (4, "Grove-Frohman 2E.3"),
    (5, "AMI-ASPEC"),
    (6, "Lattin-Jenkins-Grove"),
    (7, "Lattin-Jenkins-Grove"),
    (8, "Advanced LEVEL 2"),
    (13, "BSIM1"),
    (27, "SOSFET"),
    (28, "Modified BSIM"),
    (38, "Cypress depletion"),
    (39, "BSIM2"),
    (40, "HP a-Si TFT"),
    (41, "TI analog"),
    (46, "SGS-Thomson MOS3"),
    (47, "BSIM3v2"),
    (49, "BSIM3v3 (HSPICE)"),
    (50, "Philips MOS9"),
    (53, "BSIM3v3 (Berkeley)"),
    (54, "BSIM4"),
    (55, "EKV 2.6"),
    (57, "BSIM3-SOI PD"),
    (58, "UFSOI"),
    (59, "BSIM3-SOI FD"),
    (60, "BSIM3-SOI DD"),
    (61, "RPI a-Si TFT"),
    (62, "RPI poly-Si TFT"),
    (63, "Philips MOS11"),
    (64, "HiSIM"),
    (65, "SSIMSOI"),
    (66, "HVMOS"),
    (68, "HiSIM2"),
    (69, "PSP"),
    (70, "BSIMSOI4"),
    (71, "TFT"),
    (72, "BSIM-CMG"),
    (73, "HiSIM-HV"),
    (74, "MOS Model 20"),
    (77, "BSIM6 (unverified)"),
];

/// Common NMOS/PMOS model parameters.
pub static MOSFET_MODEL_PARAMS: &[ModelParamSpec] = &[
    ModelParamSpec {
        name: "level",
        default: Some("1"),
        description: "Model level selecting the MOSFET model",
    },
    ModelParamSpec {
        name: "version",
        default: Some("3.3"),
        description: "Model release version",
    },
    ModelParamSpec {
        name: "paramchk",
        default: None,
        description: "Parameter range checking",
    },
    ModelParamSpec {
        name: "tox",
        default: None,
        description: "Gate oxide thickness (TOXE/TOXP/TOXM in BSIM4)",
    },
    ModelParamSpec {
        name: "vth0",
        default: None,
        description: "Zero-bias threshold voltage (VTO in levels 1-3)",
    },
    ModelParamSpec {
        name: "vto",
        default: None,
        description: "Zero-bias threshold voltage, levels 1-3",
    },
    ModelParamSpec {
        name: "u0",
        default: None,
        description: "Low-field mobility (UO)",
    },
    ModelParamSpec {
        name: "uo",
        default: None,
        description: "Low-field mobility, levels 1-3",
    },
    ModelParamSpec {
        name: "k1",
        default: None,
        description: "First-order body effect coefficient",
    },
    ModelParamSpec {
        name: "k2",
        default: None,
        description: "Second-order body effect coefficient",
    },
    ModelParamSpec {
        name: "nch",
        default: None,
        description: "Channel doping concentration (NDEP)",
    },
    ModelParamSpec {
        name: "nsub",
        default: None,
        description: "Substrate doping concentration",
    },
    ModelParamSpec {
        name: "xj",
        default: None,
        description: "Junction depth",
    },
    ModelParamSpec {
        name: "vsat",
        default: None,
        description: "Saturation velocity",
    },
    ModelParamSpec {
        name: "lint",
        default: None,
        description: "Length offset fitting parameter",
    },
    ModelParamSpec {
        name: "wint",
        default: None,
        description: "Width offset fitting parameter",
    },
    ModelParamSpec {
        name: "rdsw",
        default: None,
        description: "Parasitic resistance per unit width",
    },
    ModelParamSpec {
        name: "rsh",
        default: None,
        description: "Source/drain sheet resistance",
    },
    ModelParamSpec {
        name: "cj",
        default: None,
        description: "Zero-bias bulk junction capacitance per area",
    },
    ModelParamSpec {
        name: "cjsw",
        default: None,
        description: "Zero-bias sidewall junction capacitance per length",
    },
    ModelParamSpec {
        name: "cjswg",
        default: None,
        description: "Zero-bias gate-side sidewall junction capacitance",
    },
    ModelParamSpec {
        name: "mj",
        default: None,
        description: "Bulk junction grading coefficient",
    },
    ModelParamSpec {
        name: "mjsw",
        default: None,
        description: "Sidewall junction grading coefficient",
    },
    ModelParamSpec {
        name: "pb",
        default: None,
        description: "Bulk junction potential",
    },
    ModelParamSpec {
        name: "pbsw",
        default: None,
        description: "Sidewall junction potential",
    },
    ModelParamSpec {
        name: "js",
        default: None,
        description: "Bulk junction saturation current density",
    },
    ModelParamSpec {
        name: "jsw",
        default: None,
        description: "Sidewall junction saturation current density",
    },
    ModelParamSpec {
        name: "cgso",
        default: None,
        description: "Gate-source overlap capacitance per width",
    },
    ModelParamSpec {
        name: "cgdo",
        default: None,
        description: "Gate-drain overlap capacitance per width",
    },
    ModelParamSpec {
        name: "cgbo",
        default: None,
        description: "Gate-bulk overlap capacitance per length",
    },
    ModelParamSpec {
        name: "pclm",
        default: None,
        description: "Channel length modulation parameter",
    },
    ModelParamSpec {
        name: "dvt0",
        default: None,
        description: "Short-channel effect coefficient 0",
    },
    ModelParamSpec {
        name: "eta0",
        default: None,
        description: "DIBL coefficient in subthreshold region",
    },
    ModelParamSpec {
        name: "nfactor",
        default: None,
        description: "Subthreshold swing factor",
    },
    ModelParamSpec {
        name: "voff",
        default: None,
        description: "Offset voltage in the subthreshold region",
    },
    ModelParamSpec {
        name: "capmod",
        default: None,
        description: "Capacitance model selector",
    },
    ModelParamSpec {
        name: "mobmod",
        default: None,
        description: "Mobility model selector",
    },
    ModelParamSpec {
        name: "noimod",
        default: None,
        description: "Noise model selector",
    },
    ModelParamSpec {
        name: "acm",
        default: None,
        description: "Area calculation method",
    },
    ModelParamSpec {
        name: "xpart",
        default: None,
        description: "Charge partitioning flag",
    },
    ModelParamSpec {
        name: "kf",
        default: None,
        description: "Flicker noise coefficient",
    },
    ModelParamSpec {
        name: "af",
        default: None,
        description: "Flicker noise exponent",
    },
    ModelParamSpec {
        name: "lmin",
        default: None,
        description: "Minimum channel length for this model",
    },
    ModelParamSpec {
        name: "lmax",
        default: None,
        description: "Maximum channel length for this model",
    },
    ModelParamSpec {
        name: "wmin",
        default: None,
        description: "Minimum channel width for this model",
    },
    ModelParamSpec {
        name: "wmax",
        default: None,
        description: "Maximum channel width for this model",
    },
    ModelParamSpec {
        name: "binunit",
        default: None,
        description: "Binning unit selector",
    },
    ModelParamSpec {
        name: "tnom",
        default: None,
        description: "Parameter measurement temperature",
    },
    ModelParamSpec {
        name: "kp",
        default: None,
        description: "Transconductance parameter (levels 1-3)",
    },
    ModelParamSpec {
        name: "gamma",
        default: None,
        description: "Body effect parameter (levels 1-3)",
    },
    ModelParamSpec {
        name: "phi",
        default: None,
        description: "Surface potential (levels 1-3)",
    },
    ModelParamSpec {
        name: "lambda",
        default: None,
        description: "Channel length modulation (levels 1-3)",
    },
    ModelParamSpec {
        name: "ld",
        default: None,
        description: "Lateral diffusion (levels 1-3)",
    },
];

/// Common diode model parameters.
pub static DIODE_MODEL_PARAMS: &[ModelParamSpec] = &[
    ModelParamSpec {
        name: "is",
        default: None,
        description: "Saturation current",
    },
    ModelParamSpec {
        name: "n",
        default: None,
        description: "Emission coefficient",
    },
    ModelParamSpec {
        name: "rs",
        default: None,
        description: "Ohmic series resistance",
    },
    ModelParamSpec {
        name: "cjo",
        default: None,
        description: "Zero-bias junction capacitance",
    },
    ModelParamSpec {
        name: "vj",
        default: None,
        description: "Junction potential",
    },
    ModelParamSpec {
        name: "m",
        default: None,
        description: "Junction grading coefficient",
    },
    ModelParamSpec {
        name: "tt",
        default: None,
        description: "Transit time",
    },
    ModelParamSpec {
        name: "bv",
        default: None,
        description: "Reverse breakdown voltage",
    },
];

/// Common bipolar transistor (NPN/PNP) model parameters.
pub static BJT_MODEL_PARAMS: &[ModelParamSpec] = &[
    ModelParamSpec {
        name: "is",
        default: None,
        description: "Transport saturation current",
    },
    ModelParamSpec {
        name: "bf",
        default: None,
        description: "Ideal maximum forward beta",
    },
    ModelParamSpec {
        name: "br",
        default: None,
        description: "Ideal maximum reverse beta",
    },
    ModelParamSpec {
        name: "vaf",
        default: None,
        description: "Forward Early voltage",
    },
    ModelParamSpec {
        name: "nf",
        default: None,
        description: "Forward current emission coefficient",
    },
    ModelParamSpec {
        name: "cje",
        default: None,
        description: "Base-emitter zero-bias capacitance",
    },
    ModelParamSpec {
        name: "cjc",
        default: None,
        description: "Base-collector zero-bias capacitance",
    },
    ModelParamSpec {
        name: "tf",
        default: None,
        description: "Forward transit time",
    },
    ModelParamSpec {
        name: "tr",
        default: None,
        description: "Reverse transit time",
    },
    ModelParamSpec {
        name: "rb",
        default: None,
        description: "Zero-bias base resistance",
    },
    ModelParamSpec {
        name: "rc",
        default: None,
        description: "Collector resistance",
    },
    ModelParamSpec {
        name: "re",
        default: None,
        description: "Emitter resistance",
    },
];

/// Looks up a model type by name (case-insensitive).
///
/// # Arguments
/// * `name` - type as written in `.model name type`
pub fn find_model_type(name: &str) -> Option<&'static ModelTypeSpec> {
    find_named(MODEL_TYPES, name)
}

/// Returns the model name selected by a MOSFET `LEVEL` value.
///
/// # Arguments
/// * `level` - the `LEVEL=` value of an NMOS/PMOS model card
pub fn mosfet_level_name(level: u32) -> Option<&'static str> {
    MOSFET_LEVELS
        .iter()
        .find(|(number, _)| *number == level)
        .map(|(_, name)| *name)
}

/// Returns the common parameters for a model type, or an empty slice if none are tabulated.
///
/// # Arguments
/// * `model_type` - lowercase or mixed-case model type, e.g. `NMOS`
pub fn model_params_for(model_type: &str) -> &'static [ModelParamSpec] {
    match model_type.to_ascii_lowercase().as_str() {
        "nmos" | "pmos" => MOSFET_MODEL_PARAMS,
        "d" => DIODE_MODEL_PARAMS,
        "npn" | "pnp" => BJT_MODEL_PARAMS,
        "mosra" => MOSRA_MODEL_PARAMS,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finds_model_types_any_case() {
        assert_eq!(find_model_type("NMOS").map(|m| m.name), Some("nmos"));
        assert_eq!(
            find_model_type("sw").map(|m| m.status),
            Some(Status::Foreign)
        );
        assert_eq!(
            find_model_type("plot").map(|m| m.status),
            Some(Status::Obsolete)
        );
        assert!(find_model_type("xyz").is_none());
    }

    #[test]
    fn element_letters_match_reference() {
        let letters = |name| find_model_type(name).map(|m| m.element_letters);
        assert_eq!(letters("nmos"), Some(&['M'][..]));
        assert_eq!(letters("pnp"), Some(&['Q'][..]));
        assert_eq!(letters("u"), Some(&['T', 'U'][..]));
        assert_eq!(letters("core"), Some(&['K'][..]));
    }

    #[test]
    fn mosfet_levels() {
        assert_eq!(mosfet_level_name(54), Some("BSIM4"));
        assert_eq!(mosfet_level_name(49), Some("BSIM3v3 (HSPICE)"));
        assert_eq!(mosfet_level_name(9999), None);
        let numbers: Vec<u32> = MOSFET_LEVELS.iter().map(|(n, _)| *n).collect();
        assert_eq!(numbers.len(), numbers.iter().collect::<HashSet<_>>().len());
    }

    #[test]
    fn parameter_tables() {
        assert_eq!(model_params_for("NMOS"), MOSFET_MODEL_PARAMS);
        assert_eq!(model_params_for("pnp"), BJT_MODEL_PARAMS);
        assert_eq!(model_params_for("d"), DIODE_MODEL_PARAMS);
        assert_eq!(model_params_for("MOSRA"), MOSRA_MODEL_PARAMS);
        assert!(model_params_for("r").is_empty());
        for table in [
            MOSFET_MODEL_PARAMS,
            DIODE_MODEL_PARAMS,
            BJT_MODEL_PARAMS,
            MOSRA_MODEL_PARAMS,
        ] {
            let names: HashSet<&str> = table.iter().map(|p| p.name).collect();
            assert_eq!(names.len(), table.len());
            assert!(
                table
                    .iter()
                    .all(|p| p.name == p.name.to_ascii_lowercase() && !p.description.is_empty())
            );
        }
        assert!(MOSFET_MODEL_PARAMS.len() >= 50);
        assert_eq!(MOSRA_MODEL_PARAMS.len(), 94);
    }

    #[test]
    fn model_type_names_are_unique_and_lowercase() {
        let names: HashSet<&str> = MODEL_TYPES.iter().map(|m| m.name).collect();
        assert_eq!(names.len(), MODEL_TYPES.len());
        assert!(
            MODEL_TYPES
                .iter()
                .all(|m| m.name == m.name.to_ascii_lowercase() && !m.description.is_empty())
        );
    }
}
