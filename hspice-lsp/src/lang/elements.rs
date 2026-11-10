//! Element types keyed by their first letter, plus independent-source functions and ground names.

use super::{Named, find_named};

/// What an element's non-node positional arguments refer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// No model or subckt reference.
    None,
    /// A required `.model` of one of the given lowercase types.
    Model(&'static [&'static str]),
    /// An optional `.model` of one of the given lowercase types.
    OptionalModel(&'static [&'static str]),
    /// A `.subckt` name (X elements).
    Subckt,
    /// Names of controlling voltage sources (F/H elements).
    ControllingSources,
    /// Names of coupled inductors (K elements).
    Inductors,
}

/// A named argument or `key=value` parameter with a one-line description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    /// Lowercase parameter name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
}

/// Static description of one element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementSpec {
    /// Uppercase key letter, e.g. `'M'`.
    pub letter: char,
    /// Human-readable name, e.g. `MOSFET`.
    pub name: &'static str,
    /// Syntax synopsis in manual notation.
    pub synopsis: &'static str,
    /// One- or two-sentence description.
    pub description: &'static str,
    /// Terminal names in order (e.g. `["d", "g", "s", "b"]`); empty for variadic elements.
    pub terminals: &'static [&'static str],
    /// Minimum number of nodes.
    pub min_nodes: usize,
    /// Maximum number of nodes; `None` for variadic elements.
    pub max_nodes: Option<usize>,
    /// What the non-node positional arguments refer to.
    pub reference: ReferenceKind,
    /// Accepted `key=value` instance parameters.
    pub params: &'static [ParamSpec],
    /// Positional keywords (lowercase), e.g. `dc`, `ac`, `pulse`, `poly`.
    pub keywords: &'static [&'static str],
    /// LSP snippet for a complete element line, e.g. `M${1:1} ${2:d} ${3:g} ${4:s} ${5:b} ${6:model} W=${7} L=${8}`.
    pub snippet: &'static str,
}

/// Static description of an independent-source transient function such as `PULSE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFunctionSpec {
    /// Lowercase name, e.g. `pulse`.
    pub name: &'static str,
    /// Alternative spellings, e.g. `pu`.
    pub aliases: &'static [&'static str],
    /// Positional arguments in order, e.g. `v1 v2 td tr tf pw per` for `PULSE`.
    pub arguments: &'static [ParamSpec],
    /// Number of required positional arguments.
    pub required: usize,
    /// One-line description.
    pub description: &'static str,
}

impl Named for SourceFunctionSpec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }
}

impl SourceFunctionSpec {
    /// Returns the call form `NAME(arg arg ...)`.
    pub fn signature(&self) -> String {
        let names: Vec<&str> = self
            .arguments
            .iter()
            .map(|argument| argument.name)
            .collect();
        format!("{}({})", self.name.to_ascii_uppercase(), names.join(" "))
    }
}

/// Node names that HSPICE treats as global ground (lowercase).
pub const GROUND_NAMES: &[&str] = &["0", "gnd", "gnd!", "ground", "!gnd"];

/// Builds a `ParamSpec` table entry.
macro_rules! param {
    ($name:literal, $description:literal) => {
        ParamSpec {
            name: $name,
            description: $description,
        }
    };
}

/// All HSPICE element types.
pub static ELEMENTS: &[ElementSpec] = &[
    ElementSpec {
        letter: 'R',
        name: "Resistor",
        synopsis: "Rxxx n1 n2 [mname] [R=]value|'expr' [TC1=] [TC2=] [SCALE=] [M=] [AC=] [DTEMP=] [L=] [W=] [C=] [NOISE=]",
        description: "Resistor with optional model, temperature coefficients and geometry.",
        terminals: &["n1", "n2"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::OptionalModel(&["r"]),
        params: &[
            param!("r", "Resistance value"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("scale", "Element scale factor"),
            param!("m", "Parallel multiplier"),
            param!("ac", "AC resistance"),
            param!("dtemp", "Temperature difference to circuit temperature"),
            param!("l", "Length (model-based)"),
            param!("w", "Width (model-based)"),
            param!("c", "Parasitic capacitance to ground"),
            param!("noise", "Noise scaling factor"),
        ],
        keywords: &[],
        snippet: "R${1:1} ${2:n1} ${3:n2} ${4:1k}",
    },
    ElementSpec {
        letter: 'C',
        name: "Capacitor",
        synopsis: "Cxxx n1 n2 [mname] [C=]val [TC1=] [TC2=] [SCALE=] [IC=] [M=] [W=] [L=] [DTEMP=]",
        description: "Capacitor with optional model; also equation (C=expr, POLY, Q=expr) forms.",
        terminals: &["n1", "n2"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::OptionalModel(&["c"]),
        params: &[
            param!("c", "Capacitance value or equation"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("scale", "Element scale factor"),
            param!("ic", "Initial voltage"),
            param!("m", "Parallel multiplier"),
            param!("w", "Width (model-based)"),
            param!("l", "Length (model-based)"),
            param!("dtemp", "Temperature difference to circuit temperature"),
            param!("ctype", "Capacitance equation type"),
            param!("q", "Charge equation"),
        ],
        keywords: &["poly"],
        snippet: "C${1:1} ${2:n1} ${3:n2} ${4:1p}",
    },
    ElementSpec {
        letter: 'L',
        name: "Inductor",
        synopsis: "Lxxx n1 n2 [L=]val [TC1=] [TC2=] [SCALE=] [IC=] [M=] [DTEMP=] [R=]",
        description: "Inductor with optional model; also equation (L=expr, POLY) forms.",
        terminals: &["n1", "n2"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::OptionalModel(&["l"]),
        params: &[
            param!("l", "Inductance value or equation"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("scale", "Element scale factor"),
            param!("ic", "Initial current"),
            param!("m", "Parallel multiplier"),
            param!("dtemp", "Temperature difference to circuit temperature"),
            param!("r", "Series resistance"),
            param!("ltype", "Inductance equation type"),
            param!("nt", "Number of turns"),
        ],
        keywords: &["poly", "infinity"],
        snippet: "L${1:1} ${2:n1} ${3:n2} ${4:1n}",
    },
    ElementSpec {
        letter: 'K',
        name: "Mutual inductor",
        synopsis: "Kxxx Lyyy Lzzz [K=]coupling | Kaaa Lbbb [Lccc ...] mname [MAG=]",
        description: "Couples inductors (or an inductor to a magnetic core model); has no nodes.",
        terminals: &[],
        min_nodes: 0,
        max_nodes: Some(0),
        reference: ReferenceKind::Inductors,
        params: &[
            param!("k", "Coupling coefficient"),
            param!("mag", "Initial core magnetization"),
        ],
        keywords: &[],
        snippet: "K${1:1} ${2:L1} ${3:L2} ${4:0.99}",
    },
    ElementSpec {
        letter: 'D',
        name: "Diode",
        synopsis: "Dxxx n+ n- mname [[AREA=]area] [PJ=] [WP=] [LP=] [WM=] [LM=] [OFF] [IC=vd] [M=] [DTEMP=]",
        description: "Junction diode referencing a D model.",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::Model(&["d"]),
        params: &[
            param!("area", "Junction area"),
            param!("pj", "Junction perimeter"),
            param!("wp", "Perimeter width"),
            param!("lp", "Perimeter length"),
            param!("wm", "Metal width"),
            param!("lm", "Metal length"),
            param!("off", "Start in the off state"),
            param!("ic", "Initial diode voltage"),
            param!("m", "Parallel multiplier"),
            param!("dtemp", "Temperature difference to circuit temperature"),
        ],
        keywords: &["off"],
        snippet: "D${1:1} ${2:n+} ${3:n-} ${4:model}",
    },
    ElementSpec {
        letter: 'Q',
        name: "Bipolar transistor",
        synopsis: "Qxxx nc nb ne [ns] mname [area] [OFF] [IC=vbe,vce] [M=] [DTEMP=]",
        description: "BJT referencing an NPN or PNP model; the substrate node is optional.",
        terminals: &["c", "b", "e", "s"],
        min_nodes: 3,
        max_nodes: Some(4),
        reference: ReferenceKind::Model(&["npn", "pnp"]),
        params: &[
            param!("area", "Emitter area factor"),
            param!("areab", "Base area factor"),
            param!("areac", "Collector area factor"),
            param!("off", "Start in the off state"),
            param!("ic", "Initial vbe,vce"),
            param!("vbe", "Initial base-emitter voltage"),
            param!("vce", "Initial collector-emitter voltage"),
            param!("m", "Parallel multiplier"),
            param!("dtemp", "Temperature difference to circuit temperature"),
        ],
        keywords: &["off"],
        snippet: "Q${1:1} ${2:c} ${3:b} ${4:e} ${5:model}",
    },
    ElementSpec {
        letter: 'J',
        name: "JFET",
        synopsis: "Jxxx nd ng ns [nb] mname [[AREA=]area | W= L=] [OFF] [IC=vds,vgs] [M=] [DTEMP=]",
        description: "JFET/MESFET referencing an NJF or PJF model; the bulk node is optional.",
        terminals: &["d", "g", "s", "b"],
        min_nodes: 3,
        max_nodes: Some(4),
        reference: ReferenceKind::Model(&["njf", "pjf"]),
        params: &[
            param!("area", "Area factor"),
            param!("w", "Channel width"),
            param!("l", "Channel length"),
            param!("off", "Start in the off state"),
            param!("ic", "Initial vds,vgs"),
            param!("m", "Parallel multiplier"),
            param!("dtemp", "Temperature difference to circuit temperature"),
        ],
        keywords: &["off"],
        snippet: "J${1:1} ${2:d} ${3:g} ${4:s} ${5:model}",
    },
    ElementSpec {
        letter: 'M',
        name: "MOSFET",
        synopsis: "Mxxx nd ng ns [nb] mname [[L=]len] [[W=]wid] [AD=] [AS=] [PD=] [PS=] [NRD=] [NRS=] [RDC=] [RSC=] [OFF] [IC=vds,vgs,vbs] [M=] [DTEMP=] [GEO=] [DELVTO=]",
        description: "MOSFET referencing an NMOS/PMOS model; positional order is L then W (.OPTION WL swaps).",
        terminals: &["d", "g", "s", "b"],
        min_nodes: 3,
        max_nodes: Some(4),
        reference: ReferenceKind::Model(&["nmos", "pmos"]),
        params: &[
            param!("l", "Channel length"),
            param!("w", "Channel width"),
            param!("ad", "Drain diffusion area"),
            param!("as", "Source diffusion area"),
            param!("pd", "Drain diffusion perimeter"),
            param!("ps", "Source diffusion perimeter"),
            param!("nrd", "Drain squares for resistance"),
            param!("nrs", "Source squares for resistance"),
            param!("rdc", "Additional drain resistance"),
            param!("rsc", "Additional source resistance"),
            param!("off", "Start in the off state"),
            param!("ic", "Initial vds,vgs,vbs"),
            param!("m", "Parallel multiplier"),
            param!("dtemp", "Temperature difference to circuit temperature"),
            param!("geo", "Source/drain sharing geometry"),
            param!("delvto", "Threshold voltage shift"),
            param!(
                "nf",
                "Number of fingers (UNVERIFIED in manual, model-specific)"
            ),
            param!(
                "sa",
                "Distance gate to source-side diffusion edge (model-specific)"
            ),
            param!(
                "sb",
                "Distance gate to drain-side diffusion edge (model-specific)"
            ),
            param!(
                "sd",
                "Distance between neighbouring fingers (model-specific)"
            ),
            param!("mult", "Instance multiplier (model-specific)"),
        ],
        keywords: &["off"],
        snippet: "M${1:1} ${2:d} ${3:g} ${4:s} ${5:b} ${6:model} W=${7:1u} L=${8:0.1u}",
    },
    ElementSpec {
        letter: 'V',
        name: "Voltage source",
        synopsis: "Vxxx n+ n- [[DC=]dcval] [tranfun] [AC[=]acmag[,acphase]]",
        description: "Independent voltage source with optional transient function; a bare source is legal.",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::None,
        params: &[
            param!("dc", "DC value"),
            param!("ac", "AC magnitude and phase"),
            param!("power", "Source power"),
            param!("imp", "Source impedance"),
        ],
        keywords: &[
            "dc", "ac", "pulse", "pu", "sin", "exp", "pwl", "pl", "pwlz", "sffm", "am", "pat",
            "lfsr", "data", "power", "imp",
        ],
        snippet: "V${1:1} ${2:n+} ${3:n-} DC ${4:0}",
    },
    ElementSpec {
        letter: 'I',
        name: "Current source",
        synopsis: "Ixxx n+ n- [[DC=]dcval] [tranfun] [AC[=]acmag[,acphase]] [M=]",
        description: "Independent current source with optional transient function.",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::None,
        params: &[
            param!("dc", "DC value"),
            param!("ac", "AC magnitude and phase"),
            param!("power", "Source power"),
            param!("imp", "Source impedance"),
            param!("m", "Parallel multiplier"),
        ],
        keywords: &[
            "dc", "ac", "pulse", "pu", "sin", "exp", "pwl", "pl", "pwlz", "sffm", "am", "pat",
            "lfsr", "data", "power", "imp",
        ],
        snippet: "I${1:1} ${2:n+} ${3:n-} DC ${4:0}",
    },
    ElementSpec {
        letter: 'E',
        name: "Voltage-controlled voltage source",
        synopsis: "Exxx n+ n- [VCVS] in+ in- gain | POLY(n) ... | PWL(1) ... | VOL='expr' | DELAY | LAPLACE | ...",
        description: "Voltage-output controlled source: linear, polynomial, piecewise, logic, delay, Laplace and behavioural forms.",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: None,
        reference: ReferenceKind::None,
        params: &[
            param!("vol", "Output voltage expression"),
            param!("cur", "Output current expression"),
            param!("max", "Upper output limit"),
            param!("min", "Lower output limit"),
            param!("scale", "Scale factor"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("abs", "Use absolute value of the controlling quantity"),
            param!("ic", "Initial condition"),
            param!("td", "Delay time"),
            param!("npdelay", "Number of delay points"),
            param!("delta", "PWL breakpoint smoothing"),
            param!("level", "Element variant selector"),
            param!("smooth", "Smoothing for the controlling function"),
            param!("m", "Parallel multiplier"),
            param!("noise", "Noise expression"),
        ],
        keywords: &[
            "vcvs",
            "vccs",
            "poly",
            "pwl",
            "npwl",
            "ppwl",
            "and",
            "nand",
            "or",
            "nor",
            "delay",
            "laplace",
            "pole",
            "freq",
            "foster",
            "ztrans",
            "opamp",
            "transformer",
            "vcr",
            "vccap",
            "level",
        ],
        snippet: "E${1:1} ${2:n+} ${3:n-} ${4:in+} ${5:in-} ${6:gain}",
    },
    ElementSpec {
        letter: 'G',
        name: "Voltage-controlled current source",
        synopsis: "Gxxx n+ n- [VCCS] in+ in- gm | POLY(n) ... | CUR='expr' | VCR | VCCAP | ...",
        description: "Current-output controlled source: linear, polynomial, piecewise, behavioural, voltage-controlled resistor and capacitor forms.",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: None,
        reference: ReferenceKind::None,
        params: &[
            param!("vol", "Output voltage expression"),
            param!("cur", "Output current expression"),
            param!("max", "Upper output limit"),
            param!("min", "Lower output limit"),
            param!("scale", "Scale factor"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("abs", "Use absolute value of the controlling quantity"),
            param!("ic", "Initial condition"),
            param!("td", "Delay time"),
            param!("npdelay", "Number of delay points"),
            param!("delta", "PWL breakpoint smoothing"),
            param!("level", "Element variant selector"),
            param!("smooth", "Smoothing for the controlling function"),
            param!("m", "Parallel multiplier"),
            param!("noise", "Noise expression"),
        ],
        keywords: &[
            "vcvs",
            "vccs",
            "poly",
            "pwl",
            "npwl",
            "ppwl",
            "and",
            "nand",
            "or",
            "nor",
            "delay",
            "laplace",
            "pole",
            "freq",
            "foster",
            "ztrans",
            "opamp",
            "transformer",
            "vcr",
            "vccap",
            "level",
        ],
        snippet: "G${1:1} ${2:n+} ${3:n-} ${4:in+} ${5:in-} ${6:gm}",
    },
    ElementSpec {
        letter: 'F',
        name: "Current-controlled current source",
        synopsis: "Fxxx n+ n- [CCCS] vn1 gain | POLY(n) vn1 ... vnn p0 p1 ... | PWL(1) vn1 ... | DELAY vn1 TD=",
        description: "Current-output source controlled by the current through voltage source(s).",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::ControllingSources,
        params: &[
            param!("max", "Upper output limit"),
            param!("min", "Lower output limit"),
            param!("scale", "Scale factor"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("abs", "Use absolute value of the controlling quantity"),
            param!("ic", "Initial condition"),
            param!("td", "Delay time"),
            param!("m", "Parallel multiplier"),
        ],
        keywords: &[
            "cccs", "ccvs", "poly", "pwl", "and", "nand", "or", "nor", "delay",
        ],
        snippet: "F${1:1} ${2:n+} ${3:n-} ${4:vsense} ${5:gain}",
    },
    ElementSpec {
        letter: 'H',
        name: "Current-controlled voltage source",
        synopsis: "Hxxx n+ n- [CCVS] vn1 transres | POLY(n) vn1 ... vnn p0 p1 ... | PWL(1) vn1 ... | DELAY vn1 TD=",
        description: "Voltage-output source controlled by the current through voltage source(s).",
        terminals: &["n+", "n-"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::ControllingSources,
        params: &[
            param!("max", "Upper output limit"),
            param!("min", "Lower output limit"),
            param!("scale", "Scale factor"),
            param!("tc1", "First-order temperature coefficient"),
            param!("tc2", "Second-order temperature coefficient"),
            param!("abs", "Use absolute value of the controlling quantity"),
            param!("ic", "Initial condition"),
            param!("td", "Delay time"),
            param!("m", "Parallel multiplier"),
        ],
        keywords: &[
            "cccs", "ccvs", "poly", "pwl", "and", "nand", "or", "nor", "delay",
        ],
        snippet: "H${1:1} ${2:n+} ${3:n-} ${4:vsense} ${5:transres}",
    },
    ElementSpec {
        letter: 'X',
        name: "Subcircuit instance",
        synopsis: "Xxxx n1 ... nN subnam [p=v ...] [M=val] [S=val]",
        description: "Instance of a .subckt; the subcircuit name is the last positional token.",
        terminals: &[],
        min_nodes: 0,
        max_nodes: None,
        reference: ReferenceKind::Subckt,
        params: &[
            param!("m", "Instance multiplier"),
            param!("s", "Scale factor"),
        ],
        keywords: &[],
        snippet: "X${1:1} ${2:nodes} ${3:subckt}",
    },
    ElementSpec {
        letter: 'B',
        name: "IBIS buffer",
        synopsis: "Bxxx n1 ... nN file='f' model='m' key=val ... [M=]",
        description: "IBIS I/O buffer instance.",
        terminals: &[],
        min_nodes: 0,
        max_nodes: None,
        reference: ReferenceKind::None,
        params: &[
            param!("file", "IBIS file name"),
            param!("model", "IBIS model name"),
            param!("m", "Parallel multiplier"),
        ],
        keywords: &[],
        snippet: "B${1:1} ${2:nodes} FILE='${3:file}' MODEL='${4:model}'",
    },
    ElementSpec {
        letter: 'P',
        name: "Port",
        synopsis: "Pxxx p n port=num [Z0=] [DC mag] [AC mag phase] [HBAC ...]",
        description: "Port for RF and S-parameter analyses.",
        terminals: &["p", "n"],
        min_nodes: 2,
        max_nodes: Some(2),
        reference: ReferenceKind::None,
        params: &[
            param!("port", "Port number"),
            param!("z0", "Reference impedance"),
            param!("dc", "DC value"),
            param!("ac", "AC magnitude and phase"),
        ],
        keywords: &["dc", "ac"],
        snippet: "P${1:1} ${2:p} ${3:n} PORT=${4:1} Z0=${5:50}",
    },
    ElementSpec {
        letter: 'S',
        name: "S-parameter element",
        synopsis: "Sxxx nd1 ... ndN ndRef [MNAME=] [FQMODEL=] [TYPE=s|y] [Z0=] ...",
        description: "N-port element described by an S/Y-parameter model or data file.",
        terminals: &[],
        min_nodes: 0,
        max_nodes: None,
        reference: ReferenceKind::OptionalModel(&["s", "sp"]),
        params: &[
            param!("mname", "S-parameter model name"),
            param!("fqmodel", "Frequency-dependent model name"),
            param!("type", "Parameter type (s or y)"),
            param!("z0", "Reference impedance"),
        ],
        keywords: &[],
        snippet: "S${1:1} ${2:nodes} ${3:ref} MNAME=${4:model}",
    },
    ElementSpec {
        letter: 'T',
        name: "Transmission line",
        synopsis: "Txxx in refin out refout Z0= TD= [L=] [IC=v1,i1,v2,i2] | Z0= F= [NL=] | mname L=",
        description: "Ideal or model-based two-port transmission line.",
        terminals: &["in", "refin", "out", "refout"],
        min_nodes: 4,
        max_nodes: Some(4),
        reference: ReferenceKind::OptionalModel(&["u"]),
        params: &[
            param!("z0", "Characteristic impedance"),
            param!("td", "Transmission delay"),
            param!("l", "Line length"),
            param!("ic", "Initial v1,i1,v2,i2"),
            param!("f", "Frequency for the line length"),
            param!("nl", "Normalized length at frequency f"),
        ],
        keywords: &[],
        snippet: "T${1:1} ${2:in} ${3:refin} ${4:out} ${5:refout} Z0=${6:50} TD=${7:1n}",
    },
    ElementSpec {
        letter: 'U',
        name: "Lossy transmission line",
        synopsis: "Uxxx in1 [..in5] refin out1 [..] refout mname L= [LUMPS=]",
        description: "Lossy (U-model) transmission line with up to five conductors.",
        terminals: &[],
        min_nodes: 0,
        max_nodes: None,
        reference: ReferenceKind::Model(&["u"]),
        params: &[
            param!("l", "Line length"),
            param!("lumps", "Number of lumped sections"),
        ],
        keywords: &[],
        snippet: "U${1:1} ${2:in} ${3:refin} ${4:out} ${5:refout} ${6:model} L=${7:1}",
    },
    ElementSpec {
        letter: 'W',
        name: "W-element line",
        synopsis: "Wxxx in1 [in2 ...] refin out1 [out2 ...] refout N= L= RLGCMODEL=|TABLEMODEL=|UMODEL=|FSMODEL=|SMODEL=|RLGCFILE=",
        description: "Multiconductor transmission line (2N+2 terminals).",
        terminals: &[],
        min_nodes: 0,
        max_nodes: None,
        reference: ReferenceKind::None,
        params: &[
            param!("n", "Number of signal conductors"),
            param!("l", "Line length"),
            param!("rlgcmodel", "RLGC model name"),
            param!("tablemodel", "Table model name"),
            param!("umodel", "U model name"),
            param!("fsmodel", "Field-solver model name"),
            param!("smodel", "S-parameter model name"),
            param!("rlgcfile", "RLGC data file"),
        ],
        keywords: &[],
        snippet: "W${1:1} ${2:in} ${3:refin} ${4:out} ${5:refout} N=${6:1} L=${7:1} RLGCMODEL=${8:model}",
    },
];

/// Independent-source transient functions (`PULSE`, `SIN`, `PWL` …).
pub static SOURCE_FUNCTIONS: &[SourceFunctionSpec] = &[
    SourceFunctionSpec {
        name: "pulse",
        aliases: &["pu"],
        arguments: &[
            ParamSpec {
                name: "v1",
                description: "Initial value",
            },
            ParamSpec {
                name: "v2",
                description: "Pulsed value",
            },
            ParamSpec {
                name: "td",
                description: "Delay before the first edge (default 0)",
            },
            ParamSpec {
                name: "tr",
                description: "Rise time (default TSTEP)",
            },
            ParamSpec {
                name: "tf",
                description: "Fall time (default TSTEP)",
            },
            ParamSpec {
                name: "pw",
                description: "Pulse width at v2 (default TSTOP)",
            },
            ParamSpec {
                name: "per",
                description: "Period (default TSTOP)",
            },
        ],
        required: 2,
        description: "Pulse train: v1 v2 [td [tr [tf [pw [per]]]]] [PERJITTER= [SEED=]].",
    },
    SourceFunctionSpec {
        name: "sin",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "vo",
                description: "Offset value",
            },
            ParamSpec {
                name: "va",
                description: "Amplitude",
            },
            ParamSpec {
                name: "freq",
                description: "Frequency in Hz (default 1/TSTOP)",
            },
            ParamSpec {
                name: "td",
                description: "Delay before the sine starts (default 0)",
            },
            ParamSpec {
                name: "q",
                description: "Damping factor in 1/s (default 0)",
            },
            ParamSpec {
                name: "j",
                description: "Phase delay in degrees (default 0)",
            },
        ],
        required: 2,
        description: "Damped sinusoid: vo va [freq [td [q [j]]]].",
    },
    SourceFunctionSpec {
        name: "exp",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "v1",
                description: "Initial value",
            },
            ParamSpec {
                name: "v2",
                description: "Pulsed value",
            },
            ParamSpec {
                name: "td1",
                description: "Rise delay (default 0)",
            },
            ParamSpec {
                name: "t1",
                description: "Rise time constant (default TSTEP)",
            },
            ParamSpec {
                name: "td2",
                description: "Fall delay (default td1+TSTEP)",
            },
            ParamSpec {
                name: "t2",
                description: "Fall time constant (default TSTEP)",
            },
        ],
        required: 2,
        description: "Exponential rise and decay: v1 v2 [td1 [t1 [td2 [t2]]]].",
    },
    SourceFunctionSpec {
        name: "pwl",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "t1",
                description: "First time point",
            },
            ParamSpec {
                name: "v1",
                description: "Value at t1",
            },
            ParamSpec {
                name: "t2",
                description: "Next time point",
            },
            ParamSpec {
                name: "v2",
                description: "Value at t2",
            },
        ],
        required: 2,
        description: "Piecewise linear time/value pairs; also R[=repeat], TD=delay, PWLFILE=.",
    },
    SourceFunctionSpec {
        name: "pl",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "v1",
                description: "First value",
            },
            ParamSpec {
                name: "t1",
                description: "Time of v1",
            },
            ParamSpec {
                name: "v2",
                description: "Next value",
            },
            ParamSpec {
                name: "t2",
                description: "Time of v2",
            },
        ],
        required: 2,
        description: "Piecewise linear value/time pairs.",
    },
    SourceFunctionSpec {
        name: "pwlz",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "t1",
                description: "First time point",
            },
            ParamSpec {
                name: "v1",
                description: "Value at t1",
            },
            ParamSpec {
                name: "t2",
                description: "Next time point",
            },
            ParamSpec {
                name: "v2",
                description: "Value at t2",
            },
        ],
        required: 2,
        description: "Piecewise linear variant with zero-order hold (see manual).",
    },
    SourceFunctionSpec {
        name: "sffm",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "vo",
                description: "Offset value",
            },
            ParamSpec {
                name: "va",
                description: "Amplitude",
            },
            ParamSpec {
                name: "fc",
                description: "Carrier frequency in Hz (default 1/TSTOP)",
            },
            ParamSpec {
                name: "mdi",
                description: "Modulation index (default 0)",
            },
            ParamSpec {
                name: "fs",
                description: "Signal (modulating) frequency in Hz (default 1/TSTOP)",
            },
        ],
        required: 2,
        description: "Single-frequency FM: vo va [fc [mdi [fs]]].",
    },
    SourceFunctionSpec {
        name: "am",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "sa",
                description: "Signal amplitude",
            },
            ParamSpec {
                name: "oc",
                description: "Offset constant",
            },
            ParamSpec {
                name: "fm",
                description: "Modulation frequency in Hz",
            },
            ParamSpec {
                name: "fc",
                description: "Carrier frequency in Hz",
            },
            ParamSpec {
                name: "td",
                description: "Delay (default 0)",
            },
        ],
        required: 4,
        description: "Amplitude modulation: sa oc fm fc [td].",
    },
    SourceFunctionSpec {
        name: "pat",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "vhi",
                description: "High value of a 1 bit",
            },
            ParamSpec {
                name: "vlo",
                description: "Low value of a 0 bit",
            },
            ParamSpec {
                name: "td",
                description: "Delay before the pattern starts",
            },
            ParamSpec {
                name: "tr",
                description: "Rise time",
            },
            ParamSpec {
                name: "tf",
                description: "Fall time",
            },
            ParamSpec {
                name: "tsample",
                description: "Duration of one bit",
            },
            ParamSpec {
                name: "data",
                description: "Bit string such as b1010, or a .PAT pattern name",
            },
        ],
        required: 7,
        description: "Bit pattern: vhi vlo td tr tf tsample data [RB=] [R=] [ENCODE=] [RD_INIT=].",
    },
    SourceFunctionSpec {
        name: "lfsr",
        aliases: &[],
        arguments: &[
            ParamSpec {
                name: "vlow",
                description: "Low value",
            },
            ParamSpec {
                name: "vhigh",
                description: "High value",
            },
            ParamSpec {
                name: "tdelay",
                description: "Delay before the sequence starts",
            },
            ParamSpec {
                name: "trise",
                description: "Rise time",
            },
            ParamSpec {
                name: "tfall",
                description: "Fall time",
            },
            ParamSpec {
                name: "rate",
                description: "Bit rate",
            },
            ParamSpec {
                name: "seed",
                description: "Initial register value",
            },
            ParamSpec {
                name: "taps",
                description: "Feedback tap positions",
            },
        ],
        required: 7,
        description: "Pseudo-random bit sequence: vlow vhigh tdelay trise tfall rate seed [taps] [rout=].",
    },
];

/// Looks up the element type from an element name's first letter.
///
/// # Arguments
/// * `element_name` - full element name as written, e.g. `M1` or `xinv`
pub fn find_element(element_name: &str) -> Option<&'static ElementSpec> {
    let letter = element_name.chars().next()?.to_ascii_uppercase();
    ELEMENTS.iter().find(|spec| spec.letter == letter)
}

/// Looks up a source function by name or alias (case-insensitive).
///
/// # Arguments
/// * `name` - function name as written, e.g. `PULSE`
pub fn find_source_function(name: &str) -> Option<&'static SourceFunctionSpec> {
    find_named(SOURCE_FUNCTIONS, name)
}

/// Returns true if `node` is one of HSPICE's ground aliases.
///
/// # Arguments
/// * `node` - node name as written
pub fn is_ground(node: &str) -> bool {
    GROUND_NAMES
        .iter()
        .any(|ground| ground.eq_ignore_ascii_case(node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finds_elements_by_first_letter_any_case() {
        assert_eq!(find_element("m1").map(|e| e.letter), Some('M'));
        assert_eq!(find_element("XINV").map(|e| e.letter), Some('X'));
        assert!(find_element("Y1").is_none());
        assert!(find_element("").is_none());
    }

    #[test]
    fn finds_source_functions_by_name_and_alias() {
        assert_eq!(find_source_function("PU").map(|f| f.name), Some("pulse"));
        assert_eq!(find_source_function("Sin").map(|f| f.name), Some("sin"));
        assert!(find_source_function("saw").is_none());
    }

    #[test]
    fn element_table_invariants() {
        let letters: HashSet<char> = ELEMENTS.iter().map(|e| e.letter).collect();
        assert_eq!(letters.len(), ELEMENTS.len());
        assert_eq!(ELEMENTS.len(), 21);
        for element in ELEMENTS {
            assert!(!element.synopsis.is_empty() && !element.description.is_empty());
            assert!(element.snippet.starts_with(element.letter));
            assert!(element.max_nodes.is_none_or(|max| element.min_nodes <= max));
            assert!(
                element
                    .params
                    .iter()
                    .all(|p| p.name == p.name.to_ascii_lowercase() && !p.description.is_empty())
            );
            if let Some(max) = element.max_nodes.filter(|_| !element.terminals.is_empty()) {
                assert_eq!(element.terminals.len(), max, "{}", element.letter);
            }
        }
    }

    #[test]
    fn mosfet_shape() {
        let Some(mosfet) = find_element("M1") else {
            panic!("M element missing");
        };
        assert_eq!((mosfet.min_nodes, mosfet.max_nodes), (3, Some(4)));
        assert!(matches!(mosfet.reference, ReferenceKind::Model(types) if types.contains(&"nmos")));
    }

    #[test]
    fn source_function_invariants() {
        for function in SOURCE_FUNCTIONS {
            assert!(
                function.required <= function.arguments.len(),
                "{}",
                function.name
            );
            assert!(!function.description.is_empty());
            assert!(
                function
                    .arguments
                    .iter()
                    .all(|argument| !argument.description.is_empty())
            );
        }
    }

    #[test]
    fn ground_aliases() {
        assert!(is_ground("GND!"));
        assert!(is_ground("0"));
        assert!(!is_ground("out"));
    }
}
