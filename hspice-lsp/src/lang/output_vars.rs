//! Output-variable functions for `.print`, `.probe` and `.measure` (`v()`, `i()`, `vdb()` …).

use super::Named;

/// What the argument of an output-variable function refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputArgKind {
    /// One node or a node pair, e.g. `v(out)`, `v(a,b)`.
    Nodes,
    /// An element name, e.g. `i(vdd)`, `lx4(m1)`.
    Element,
    /// A quoted expression, e.g. `par('a*b')`.
    Expression,
    /// A format selector such as `db` in `s11(db)`, or no argument at all.
    Other,
}

/// Static description of one output-variable function or word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputVarSpec {
    /// Lowercase name or prefix, e.g. `vdb`, `lx`.
    pub name: &'static str,
    /// Alternative spellings.
    pub aliases: &'static [&'static str],
    /// True if a trailing terminal/template number is allowed (`i1(m1)`, `lv9(m1)`).
    pub numbered: bool,
    /// Kind of argument.
    pub argument: OutputArgKind,
    /// Signature for hover, e.g. `V(n1<,n2>)`.
    pub signature: &'static str,
    /// One-line description.
    pub description: &'static str,
}

impl Named for OutputVarSpec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }
}

/// All output-variable functions and plain output words (`onoise`, `power` …).
pub static OUTPUT_VARS: &[OutputVarSpec] = &[
    OutputVarSpec {
        name: "v",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Nodes,
        signature: "V(n1<,n2>)",
        description: "Node voltage, node-pair difference, or terminal k voltage of an element (Vk(elem))",
    },
    OutputVarSpec {
        name: "vr",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VR(n1<,n2>)",
        description: "AC voltage real part",
    },
    OutputVarSpec {
        name: "vi",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VI(n1<,n2>)",
        description: "AC voltage imaginary part",
    },
    OutputVarSpec {
        name: "vm",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VM(n1<,n2>)",
        description: "AC voltage magnitude",
    },
    OutputVarSpec {
        name: "vp",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VP(n1<,n2>)",
        description: "AC voltage phase",
    },
    OutputVarSpec {
        name: "vdb",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VDB(n1<,n2>)",
        description: "AC voltage magnitude in dB",
    },
    OutputVarSpec {
        name: "vt",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Nodes,
        signature: "VT(n1<,n2>)",
        description: "AC voltage total (noise) voltage",
    },
    OutputVarSpec {
        name: "i",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "I(elem)",
        description: "Element current; In(elem) selects terminal n",
    },
    OutputVarSpec {
        name: "ir",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "IR(elem)",
        description: "AC current real part; optional terminal number",
    },
    OutputVarSpec {
        name: "ii",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "II(elem)",
        description: "AC current imaginary part; optional terminal number",
    },
    OutputVarSpec {
        name: "im",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "IM(elem)",
        description: "AC current magnitude; optional terminal number",
    },
    OutputVarSpec {
        name: "ip",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "IP(elem)",
        description: "AC current phase; optional terminal number",
    },
    OutputVarSpec {
        name: "idb",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "IDB(elem)",
        description: "AC current magnitude in dB; optional terminal number",
    },
    OutputVarSpec {
        name: "it",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "IT(elem)",
        description: "AC current total (noise) current; optional terminal number",
    },
    OutputVarSpec {
        name: "iall",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Element,
        signature: "IALL(elem)",
        description: "All terminal currents of an element",
    },
    OutputVarSpec {
        name: "p",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Element,
        signature: "P(elem)",
        description: "Instantaneous element power",
    },
    OutputVarSpec {
        name: "power",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "POWER",
        description: "Total power dissipation",
    },
    OutputVarSpec {
        name: "onoise",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "ONOISE<(R|I|M|P|DB)>",
        description: "Output noise",
    },
    OutputVarSpec {
        name: "inoise",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "INOISE<(R|I|M|P|DB)>",
        description: "Equivalent input noise",
    },
    OutputVarSpec {
        name: "par",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Expression,
        signature: "PAR('expr')",
        description: "Value of an expression; name=PAR('expr') names the column",
    },
    OutputVarSpec {
        name: "lv",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "LVn(elem)",
        description: "Element template variable n (e.g. LV9 = VTH, LV10 = VDSAT)",
    },
    OutputVarSpec {
        name: "lx",
        aliases: &[],
        numbered: true,
        argument: OutputArgKind::Element,
        signature: "LXn(elem)",
        description: "Element template variable n (e.g. LX4 = IDS, LX7 = gm, LX8 = gds)",
    },
    OutputVarSpec {
        name: "zin",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "ZIN(z)",
        description: "Input impedance; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "zout",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "ZOUT(z)",
        description: "Output impedance",
    },
    OutputVarSpec {
        name: "yin",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "YIN(z)",
        description: "Input admittance",
    },
    OutputVarSpec {
        name: "yout",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "YOUT(z)",
        description: "Output admittance",
    },
    OutputVarSpec {
        name: "s11",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "S11(z)",
        description: "S-parameter 11; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "s12",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "S12(z)",
        description: "S-parameter 12; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "s21",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "S21(z)",
        description: "S-parameter 21; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "s22",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "S22(z)",
        description: "S-parameter 22; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "z11",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Z11(z)",
        description: "Z-parameter 11; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "z12",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Z12(z)",
        description: "Z-parameter 12; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "z21",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Z21(z)",
        description: "Z-parameter 21; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "z22",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Z22(z)",
        description: "Z-parameter 22; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "y11",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Y11(z)",
        description: "Y-parameter 11; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "y12",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Y12(z)",
        description: "Y-parameter 12; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "y21",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Y21(z)",
        description: "Y-parameter 21; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "y22",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "Y22(z)",
        description: "Y-parameter 22; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "h11",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "H11(z)",
        description: "H-parameter 11; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "h12",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "H12(z)",
        description: "H-parameter 12; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "h21",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "H21(z)",
        description: "H-parameter 21; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "h22",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "H22(z)",
        description: "H-parameter 22; z in DB, M, P, R, I",
    },
    OutputVarSpec {
        name: "hd2",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "HD2<(R|I|M|P|DB)>",
        description: "Second-order harmonic distortion",
    },
    OutputVarSpec {
        name: "hd3",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "HD3<(R|I|M|P|DB)>",
        description: "Third-order harmonic distortion",
    },
    OutputVarSpec {
        name: "sim2",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "SIM2<(R|I|M|P|DB)>",
        description: "Second-order intermodulation (sum)",
    },
    OutputVarSpec {
        name: "dim2",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "DIM2<(R|I|M|P|DB)>",
        description: "Second-order intermodulation (difference)",
    },
    OutputVarSpec {
        name: "dim3",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "DIM3<(R|I|M|P|DB)>",
        description: "Third-order intermodulation",
    },
    OutputVarSpec {
        name: "lstb",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Other,
        signature: "LSTB<(DB|M|P|R|I)>",
        description: "Loop stability (loop gain)",
    },
    OutputVarSpec {
        name: "deriv",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Expression,
        signature: "deriv('v(x)')",
        description: "Derivative of an expression",
    },
    OutputVarSpec {
        name: "integ",
        aliases: &[],
        numbered: false,
        argument: OutputArgKind::Expression,
        signature: "integ('v(x)')",
        description: "Integral of an expression",
    },
];

/// Looks up an output variable, accepting numbered forms such as `i2` or `lx18` when `numbered` is set.
///
/// # Arguments
/// * `name` - function name as written, e.g. `VDB` or `lx4`
pub fn find_output_var(name: &str) -> Option<&'static OutputVarSpec> {
    let lowercase = name.to_ascii_lowercase();
    let without_digits = lowercase.trim_end_matches(|character: char| character.is_ascii_digit());
    OUTPUT_VARS.iter().find(|spec| {
        spec.spellings().any(|candidate| candidate == lowercase)
            || (spec.numbered
                && without_digits.len() < lowercase.len()
                && spec
                    .spellings()
                    .any(|candidate| candidate == without_digits))
    })
}

/// Looks up an output variable, accepting a terminal number on any name (`v2`, `i1`).
///
/// # Arguments
/// * `name` - function name as written
pub fn find_terminal_output_var(name: &str) -> Option<&'static OutputVarSpec> {
    find_output_var(name).or_else(|| {
        find_output_var(name.trim_end_matches(|character: char| character.is_ascii_digit()))
    })
}

/// Returns what the argument of an output variable names; `Vk(elem)` names an element terminal.
///
/// # Arguments
/// * `name` - function name as written, e.g. `v1` or `VDB`
pub fn output_argument(name: &str) -> Option<OutputArgKind> {
    let spec = find_output_var(name)?;
    let terminal_voltage = spec.name == "v" && name.ends_with(|c: char| c.is_ascii_digit());
    Some(if terminal_voltage {
        OutputArgKind::Element
    } else {
        spec.argument
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::assert_unique_lowercase;

    #[test]
    fn finds_plain_and_numbered_forms() {
        assert_eq!(find_output_var("VDB").map(|v| v.name), Some("vdb"));
        assert_eq!(find_output_var("i2").map(|v| v.name), Some("i"));
        assert_eq!(find_output_var("lx18").map(|v| v.name), Some("lx"));
        assert_eq!(find_output_var("IR3").map(|v| v.name), Some("ir"));
        assert_eq!(find_output_var("s11").map(|v| v.name), Some("s11"));
        assert!(find_output_var("vdb2").is_none());
        assert!(find_output_var("bogus").is_none());
    }

    #[test]
    fn argument_kinds() {
        let kind = |name| find_output_var(name).map(|v| v.argument);
        assert_eq!(kind("v"), Some(OutputArgKind::Nodes));
        assert_eq!(kind("par"), Some(OutputArgKind::Expression));
        assert_eq!(kind("power"), Some(OutputArgKind::Other));
        assert_eq!(kind("p"), Some(OutputArgKind::Element));
        assert_eq!(output_argument("V"), Some(OutputArgKind::Nodes));
        assert_eq!(output_argument("v1"), Some(OutputArgKind::Element));
        assert_eq!(output_argument("i2"), Some(OutputArgKind::Element));
    }

    #[test]
    fn table_invariants() {
        assert_unique_lowercase(OUTPUT_VARS);
        assert!(
            OUTPUT_VARS
                .iter()
                .all(|v| !v.signature.is_empty() && !v.description.is_empty())
        );
    }
}
