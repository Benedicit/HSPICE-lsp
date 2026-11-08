//! Dot commands: names, aliases, synopses and completion snippets.

use super::elements::ParamSpec;
use super::{Named, Status, Usage, find_named};

/// Broad grouping of dot commands, used for documentation and symbol kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// Simulation analyses (`.tran`, `.ac`, `.dc` …).
    Analysis,
    /// Output requests (`.print`, `.probe`, `.measure` …).
    Output,
    /// Setup and control (`.option`, `.param`, `.ic`, `.if` …).
    Control,
    /// File handling (`.include`, `.lib`, `.hdl` …).
    Files,
    /// Models and hierarchy (`.model`, `.subckt` …).
    Models,
    /// HSPICE RF, IBIS and field-solver commands.
    Advanced,
}

/// Static description of one dot command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// Canonical lowercase name including the dot, e.g. `.tran`.
    pub name: &'static str,
    /// Accepted alternative spellings including the dot, e.g. `.meas` for `.measure`.
    pub aliases: &'static [&'static str],
    /// One-line syntax synopsis in manual notation (`<optional>`, `a|b`).
    pub synopsis: &'static str,
    /// One- or two-sentence description.
    pub description: &'static str,
    /// Command group.
    pub category: CommandCategory,
    /// How common the command is.
    pub usage: Usage,
    /// Support status.
    pub status: Status,
    /// LSP snippet including the leading dot (e.g. `.TRAN ${1:tstep} ${2:tstop}`); `None` inserts the name.
    pub snippet: Option<&'static str>,
    /// Arguments of the synopsis with their meaning (empty if not documented).
    pub arguments: &'static [ParamSpec],
}

impl Named for CommandSpec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }
}

/// Builds one `CommandSpec`, optionally with `[("arg", "meaning"), ...]` argument docs.
macro_rules! command {
    ($name:literal, [$($alias:literal),*], $category:ident, $usage:ident, $status:ident, $snippet:expr, $synopsis:literal, $description:literal) => {
        command!($name, [$($alias),*], $category, $usage, $status, $snippet, $synopsis, $description, [])
    };
    ($name:literal, [$($alias:literal),*], $category:ident, $usage:ident, $status:ident, $snippet:expr, $synopsis:literal, $description:literal, [$(($argument:literal, $meaning:literal)),* $(,)?]) => {
        CommandSpec {
            name: $name,
            aliases: &[$($alias),*],
            synopsis: $synopsis,
            description: $description,
            category: CommandCategory::$category,
            usage: Usage::$usage,
            status: Status::$status,
            snippet: $snippet,
            arguments: &[$(ParamSpec { name: $argument, description: $meaning }),*],
        }
    };
}

/// All known dot commands, including obsolete and foreign ones (flagged via `status`).
pub static COMMANDS: &[CommandSpec] = &[
    command!(
        ".ac",
        [],
        Analysis,
        Common,
        Supported,
        Some(".AC ${1:DEC} ${2:np} ${3:fstart} ${4:fstop}"),
        ".AC DEC|OCT|LIN|POI np fstart fstop <SWEEP spec>\n.AC DATA=datanm <SWEEP spec>",
        "AC small-signal frequency sweep (needs a source with an AC value).",
        [
            (
                "DEC|OCT|LIN|POI",
                "Sweep type: points per decade, per octave, linear total, or a list of points"
            ),
            (
                "np",
                "Number of points (per decade/octave for DEC/OCT, total for LIN/POI)"
            ),
            ("fstart", "Start frequency"),
            ("fstop", "Stop frequency"),
            (
                "SWEEP",
                "Optional outer sweep of a parameter, source, TEMP, DATA or MONTE"
            ),
        ]
    ),
    command!(
        ".dc",
        [],
        Analysis,
        Common,
        Supported,
        Some(".DC ${1:source} ${2:start} ${3:stop} ${4:step}"),
        ".DC var1 start1 stop1 incr1 <var2 start2 stop2 incr2 | SWEEP spec>\n.DC var1 DEC|OCT|LIN|POI np start stop\n.DC DATA=d\n.DC MONTE=n",
        "DC sweep of a source, parameter or TEMP.",
        [
            ("var1", "Swept independent source, .PARAM name or TEMP"),
            ("start1", "Start value"),
            ("stop1", "Stop value"),
            ("incr1", "Step size"),
            ("var2 ...", "Optional nested second sweep"),
            ("SWEEP", "Optional outer sweep (DATA, MONTE, OPTIMIZE ...)"),
        ]
    ),
    command!(
        ".tran",
        [],
        Analysis,
        Common,
        Supported,
        Some(".TRAN ${1:tstep} ${2:tstop}"),
        ".TRAN tstep1 tstop1 <tstep2 tstop2 ...> <START=val> <UIC> <SWEEP spec>\n.TRAN DATA=d",
        "Transient analysis.",
        [
            (
                "tstep",
                "Printing/plotting increment, also the initial timestep guess"
            ),
            ("tstop", "End time of the simulation"),
            ("START", "Time from which results are written (default 0)"),
            (
                "UIC",
                "Use the .IC initial conditions and skip the DC operating point"
            ),
            (
                "SWEEP",
                "Optional outer sweep of a parameter, source, TEMP, DATA or MONTE"
            ),
        ]
    ),
    command!(
        ".op",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".OP <ALL|BRIEF|CURRENT|DEBUG|NONE|VOLTAGE> <time> <format time ...> <INTERPOLATION>",
        "DC operating point.",
        [
            (
                "ALL|BRIEF|CURRENT|DEBUG|NONE|VOLTAGE",
                "What to print: everything, a summary, currents, debug data, nothing, or node voltages"
            ),
            (
                "time",
                "Transient times at which an operating point is printed"
            ),
            (
                "INTERPOLATION",
                "Interpolate to the exact times instead of using the nearest timestep"
            ),
        ]
    ),
    command!(
        ".noise",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".NOISE ov srcnam <inter> <listckt=1|0> <listfreq=(f..)|none|all> <listcount=n> <listfloor=v> <listsources=1|0|yes|no>",
        "Noise analysis (used together with .AC).",
        [
            ("ov", "Output variable: V(n), V(n1,n2) or I(elem)"),
            ("srcnam", "Independent source used as noise input reference"),
            (
                "inter",
                "Print the noise summary every inter frequency points"
            ),
        ]
    ),
    command!(
        ".tf",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".TF ov srcnam",
        "DC small-signal transfer function.",
        [
            ("ov", "Output variable: V(n), V(n1,n2) or I(elem)"),
            ("srcnam", "Independent input source"),
        ]
    ),
    command!(
        ".sens",
        [],
        Analysis,
        Occasional,
        Supported,
        None,
        ".SENS ov1 <ov2 ...>",
        "DC sensitivity analysis."
    ),
    command!(
        ".pz",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".PZ ov srcnam",
        "Pole/zero analysis.",
        [
            ("ov", "Output variable: V(n) or I(elem)"),
            ("srcnam", "Independent input source"),
        ]
    ),
    command!(
        ".four",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".FOUR freq ov1 <ov2 ...>",
        "Fourier analysis of the transient result.",
        [
            ("freq", "Fundamental frequency"),
            ("ov1 ...", "Output variables to analyse"),
        ]
    ),
    command!(
        ".fft",
        [],
        Analysis,
        Common,
        Supported,
        None,
        ".FFT ov <START=|FROM=v> <STOP=|TO=v> <NP=v> <FORMAT=NORM|UNORM> <WINDOW=RECT|BART|HANN|HAMM|BLACK|HARRIS|GAUSS|KAISER> <ALFA=v> <FREQ=v> <FMIN=v> <FMAX=v>",
        "Spectrum of the transient result.",
        [
            ("ov", "Output variable to transform"),
            ("START/STOP", "Time window (default whole transient)"),
            ("NP", "Number of points, a power of 2 (default 1024)"),
            ("FORMAT", "NORM (normalized) or UNORM"),
            (
                "WINDOW",
                "Window function: RECT, BART, HANN, HAMM, BLACK, HARRIS, GAUSS, KAISER"
            ),
        ]
    ),
    command!(
        ".temp",
        [".temperature"],
        Analysis,
        Common,
        Supported,
        Some(".TEMP ${1:25}"),
        ".TEMP t1 <t2 ...>",
        "Simulation temperature(s).",
        [(
            "t1 ...",
            "Simulation temperatures in degrees Celsius; each runs all analyses"
        ),]
    ),
    command!(
        ".disto",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".DISTO Rload <inter <skw2 <refpwr <spwf>>>>",
        "Small-signal distortion analysis."
    ),
    command!(
        ".sample",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".SAMPLE FS=freq <TOL=> <NUMF=> <MAXFLD=> <BETA=0|1>",
        "Sampling noise analysis."
    ),
    command!(
        ".lstb",
        [],
        Analysis,
        Occasional,
        Supported,
        None,
        ".LSTB <name> mode=single|diff|comm vsource=vlstb|vlstbp,vlstbn",
        "Loop stability analysis."
    ),
    command!(
        ".dcmatch",
        [],
        Analysis,
        Occasional,
        Supported,
        None,
        ".DCMATCH ov <THRESHOLD=T> <FILE=s> <INTERVAL=n>",
        "DC mismatch analysis."
    ),
    command!(
        ".acmatch",
        [],
        Analysis,
        Occasional,
        Supported,
        None,
        ".ACMATCH ov <THRESHOLD=T> <FILE=s> <INTERVAL=n>",
        "AC mismatch analysis."
    ),
    command!(
        ".dcsens",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".DCSENS ov <File=s> <Perturbation=x> <Interval=v> <Threshold=x> <GroupByDevice=0|1>",
        "DC sensitivity table."
    ),
    command!(
        ".lin",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".LIN <sparcalc=1|0> <modelname=n> <filename=f> <format=...> <noisecalc=...> <gdcalc=...> <dataformat=ri|ma|db>",
        "Linear network / S-parameter extraction."
    ),
    command!(
        ".trannoise",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".TRANNOISE ov <METHOD=MC> <SEED=> <SAMPLES=> <START=> <AUTOCORRELATION=> <FMIN=> <FMAX=> <SCALE=> <PHASENOISE=> <JITTER=> <REF=> <PSD=>",
        "Transient noise analysis."
    ),
    command!(
        ".stateye",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".STATEYE T=t Trf=t <Tr=t> <Tf=t> Incident_port=... Probe_port=... ...",
        "Statistical eye diagram."
    ),
    command!(
        ".power",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".POWER signal <REF=vname FROM=t1 TO=t2>",
        "Power table."
    ),
    command!(
        ".powerdc",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".POWERDC TOP|ALL <subckt ...>",
        "DC leakage power."
    ),
    command!(
        ".sweepblock",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".SWEEPBLOCK name sweepspec <sweepspec ...>",
        "Named union of sweep specifications."
    ),
    command!(
        ".store",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".STORE <file=f> <time=t> <repeat=t>",
        "Transient checkpoint."
    ),
    command!(
        ".surge",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".SURGE threshold width node1 <node2 ...>",
        "Current surge detection."
    ),
    command!(
        ".check",
        [],
        Analysis,
        Rare,
        Supported,
        None,
        ".CHECK EDGE|FALL|GLOBAL_LEVEL|HOLD|IRDROP|RISE|SETUP|SLEW ...",
        "Timing checks."
    ),
    command!(
        ".hb",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HB ...",
        "Harmonic balance analysis."
    ),
    command!(
        ".hbac",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBAC ...",
        "Harmonic balance small-signal AC analysis."
    ),
    command!(
        ".hbxf",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBXF ...",
        "Harmonic balance transfer function analysis."
    ),
    command!(
        ".hblin",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBLIN ...",
        "Harmonic balance linear network analysis."
    ),
    command!(
        ".hblsp",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBLSP ...",
        "Harmonic balance large-signal S-parameter analysis."
    ),
    command!(
        ".hbnoise",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBNOISE ...",
        "Harmonic balance noise analysis."
    ),
    command!(
        ".hbosc",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".HBOSC ...",
        "Harmonic balance oscillator analysis."
    ),
    command!(
        ".sn",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SN ...",
        "Shooting Newton analysis."
    ),
    command!(
        ".snac",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SNAC ...",
        "Shooting Newton AC analysis."
    ),
    command!(
        ".snxf",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SNXF ...",
        "Shooting Newton transfer function analysis."
    ),
    command!(
        ".snft",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SNFT ...",
        "Shooting Newton Fourier transform."
    ),
    command!(
        ".snnoise",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SNNOISE ...",
        "Shooting Newton noise analysis."
    ),
    command!(
        ".snosc",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SNOSC ...",
        "Shooting Newton oscillator analysis."
    ),
    command!(
        ".env",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".ENV ...",
        "Envelope analysis."
    ),
    command!(
        ".envfft",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".ENVFFT ...",
        "Envelope FFT."
    ),
    command!(
        ".envosc",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".ENVOSC ...",
        "Envelope oscillator analysis."
    ),
    command!(
        ".phasenoise",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".PHASENOISE ...",
        "Phase noise analysis."
    ),
    command!(
        ".acphasenoise",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".ACPHASENOISE ...",
        "AC phase noise analysis."
    ),
    command!(
        ".ptdnoise",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".PTDNOISE ...",
        "Periodic time-domain noise analysis."
    ),
    command!(
        ".jitter",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".JITTER ...",
        "Jitter analysis."
    ),
    command!(
        ".print",
        [],
        Output,
        Common,
        Supported,
        Some(".PRINT ${1:TRAN} V(${2:node})"),
        ".PRINT <DC|AC|TRAN|NOISE|DISTO> ov1 <ov2 ...>",
        "Print output variables to the listing file (.lis).",
        [
            ("antype", "Analysis type: DC, AC, TRAN, NOISE or DISTO"),
            (
                "ov1 ...",
                "Output variables: V(n), V(n1,n2), I(elem), PAR('expr') ..."
            ),
        ]
    ),
    command!(
        ".probe",
        [],
        Output,
        Common,
        Supported,
        Some(".PROBE ${1:TRAN} V(${2:node})"),
        ".PROBE <antype> ov1 <ov2 ...>",
        "Save output variables to the waveform file.",
        [
            ("antype", "Analysis type: DC, AC, TRAN ..."),
            (
                "ov1 ...",
                "Output variables written to the waveform file: V(n), I(elem) ..."
            ),
        ]
    ),
    command!(
        ".measure",
        [".meas"],
        Output,
        Common,
        Supported,
        Some(
            ".MEAS TRAN ${1:name} TRIG V(${2:in}) VAL=${3} RISE=1 TARG V(${4:out}) VAL=${5} FALL=1"
        ),
        ".MEAS <DC|AC|TRAN|DC_CONT|AC_CONT|TRAN_CONT> result <body> <GOAL=v> <MINVAL=v> <WEIGHT=v> <PRINT=0|1>",
        "Measurement on simulation results.",
        [
            ("DC|AC|TRAN", "Analysis whose results are measured"),
            ("result", "Name of the measurement result"),
            (
                "body",
                "TRIG ... TARG ..., FIND ... WHEN/AT ..., AVG/RMS/MIN/MAX/PP ... FROM= TO=, INTEG, DERIV or PARAM='expr'"
            ),
            (
                "GOAL/MINVAL/WEIGHT",
                "Optimization target, minimum value and weight"
            ),
        ]
    ),
    command!(
        ".biaschk",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".BIASCHK 'expr' <limit=> <noise=> <max=> <min=> <simulation=op|dc|tr|all> <monitor=v|i|w|l> <tstart=> <tstop=> <autostop> <interval=>",
        "Bias check."
    ),
    command!(
        ".dout",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".DOUT nd VTH (time state ...)",
        "Expected digital output."
    ),
    command!(
        ".stim",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".STIM <tran|ac|dc> PWL|DATA|VEC <filename=f> ...",
        "Convert results to stimulus."
    ),
    command!(
        ".lprint",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".LPRINT (v1,v2) ov ...",
        "VCD output."
    ),
    command!(
        ".model_info",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".MODEL_INFO ALL | inst1, inst2, ...",
        "Model information output."
    ),
    command!(
        ".mosraprint",
        [],
        Output,
        Rare,
        Supported,
        None,
        ".MOSRAPRINT name type(elem, vds=, vgs=, vbs=)",
        "MOSFET reliability (MOSRA) print."
    ),
    command!(
        ".option",
        [".options", ".opt"],
        Control,
        Common,
        Supported,
        Some(".OPTION ${1:POST}"),
        ".OPTION opt1<=v> <opt2<=v> ...>",
        "Simulator options; a bare name sets the option to 1, the last setting wins.",
        [(
            "opt",
            "Option name; options with values are written opt=value"
        ),]
    ),
    command!(
        ".param",
        [".parameter", ".parameters", ".para"],
        Control,
        Common,
        Supported,
        Some(".PARAM ${1:name}=${2:value}"),
        ".PARAM n=val|'expr' ...\n.PARAM f(a,b)='expr'",
        "Parameters and user-defined functions.",
        [
            ("n", "Parameter name"),
            (
                "val|'expr'",
                "Number, quoted expression or distribution such as AGAUSS(nom,abs,sigma)"
            ),
            ("f(a,b)", "Function definition with formal arguments"),
        ]
    ),
    command!(
        ".ic",
        [],
        Control,
        Common,
        Supported,
        Some(".IC V(${1:node})=${2:0}"),
        ".IC V(n1)=v1 <V(n2)=v2 ...> <subckt=name>",
        "Transient initial conditions.",
        [
            (
                "V(n)",
                "Node whose voltage is fixed during the initial DC solution"
            ),
            ("v", "Initial voltage"),
            ("subckt", "Apply only inside instances of this subcircuit"),
        ]
    ),
    command!(
        ".nodeset",
        [],
        Control,
        Common,
        Supported,
        None,
        ".NODESET V(n1)=v1 ...\n.NODESET n1 v1 ...",
        "DC operating-point hints.",
        [
            ("V(n)", "Node with an initial guess for the DC solution"),
            ("v", "Initial guess, released after the first iterations"),
        ]
    ),
    command!(
        ".dcvolt",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".DCVOLT V(n1)=v1 ...\n.DCVOLT n1 v1 ...",
        "Same as .IC."
    ),
    command!(
        ".data",
        [],
        Control,
        Common,
        Supported,
        Some(".DATA ${1:name} ${2:params}\n+ $0\n.ENDDATA"),
        ".DATA name p1 <p2..> v1 <v2..> ... .ENDDATA\n.DATA name MER|LAM FILE='f' p1=col ... .ENDDATA",
        "Data table for sweeps; closed by .ENDDATA.",
        [
            ("name", "Name of the data block, used with DATA=name"),
            ("p1 ...", "Column (parameter) names"),
            ("v1 ...", "Values, one row per sweep point"),
        ]
    ),
    command!(
        ".enddata",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".ENDDATA",
        "End of a .DATA block."
    ),
    command!(
        ".global",
        [],
        Control,
        Common,
        Supported,
        Some(".GLOBAL ${1:vdd}"),
        ".GLOBAL n1 <n2 ...>",
        "Global nodes visible in all subcircuits.",
        [("n1 ...", "Nodes shared by all subcircuits"),]
    ),
    command!(
        ".title",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".TITLE string",
        "Netlist title (alternative to the first line)."
    ),
    command!(
        ".end",
        [],
        Control,
        Common,
        Supported,
        Some(".END"),
        ".END <comment>",
        "End of the netlist (deck); text after it starts a new deck."
    ),
    command!(
        ".alter",
        [],
        Control,
        Common,
        Supported,
        Some(".ALTER ${1:title}"),
        ".ALTER <title>",
        "Re-run the simulation with changes.",
        [(
            "title",
            "Title of the rerun; following statements replace earlier ones"
        ),]
    ),
    command!(
        ".save",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".SAVE <TYPE=NODESET|IC> <FILE=f> <LEVEL=ALL|TOP|SELECT|NONE> <TIME=t>",
        "Save the operating point."
    ),
    command!(
        ".load",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".LOAD <FILE=f> <RUN=PREVIOUS|CURRENT>",
        "Load a saved operating point."
    ),
    command!(
        ".connect",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".CONNECT n1 n2",
        "Connect two nodes."
    ),
    command!(
        ".if",
        [],
        Control,
        Occasional,
        Supported,
        Some(".IF (${1:condition})\n\t$0\n.ENDIF"),
        ".IF (cond) ... .ELSEIF (cond) ... .ELSE ... .ENDIF",
        "Conditional block."
    ),
    command!(
        ".elseif",
        [],
        Control,
        Occasional,
        Supported,
        None,
        ".ELSEIF (cond)",
        "Alternative branch of a conditional block."
    ),
    command!(
        ".else",
        [],
        Control,
        Occasional,
        Supported,
        None,
        ".ELSE",
        "Final branch of a conditional block."
    ),
    command!(
        ".endif",
        [],
        Control,
        Occasional,
        Supported,
        None,
        ".ENDIF",
        "End of a conditional block."
    ),
    command!(
        ".design_exploration",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".DESIGN_EXPLORATION ... .END_DESIGN_EXPLORATION",
        "Start of a design exploration block."
    ),
    command!(
        ".end_design_exploration",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".END_DESIGN_EXPLORATION",
        "End of a design exploration block."
    ),
    command!(
        ".pat",
        [],
        Control,
        Rare,
        Supported,
        None,
        ".PAT name=data <RB=v> <R=n>",
        "Pattern definition."
    ),
    command!(
        ".include",
        [".inc", ".incl"],
        Files,
        Common,
        Supported,
        Some(".INCLUDE '${1:file}'"),
        ".INCLUDE '<path>file'",
        "Include another file; nested paths resolve relative to the including file.",
        [("file", "Netlist file to insert, in quotes"),]
    ),
    command!(
        ".lib",
        [],
        Files,
        Common,
        Supported,
        Some(".LIB '${1:file}' ${2:section}"),
        ".LIB '<path>file' entry\n.LIB entry",
        "Call a library section or start a library section definition.",
        [
            ("file", "Library file, in quotes"),
            ("entry", "Section between .LIB entry and .ENDL entry"),
        ]
    ),
    command!(
        ".endl",
        [],
        Files,
        Common,
        Supported,
        None,
        ".ENDL <entry>",
        "End of a library section definition."
    ),
    command!(
        ".del",
        [],
        Files,
        Rare,
        Supported,
        None,
        ".DEL LIB '<path>file' entry\n.DEL LIB libnumber entry",
        "Delete a library section from the active set."
    ),
    command!(
        ".hdl",
        [],
        Files,
        Occasional,
        Supported,
        None,
        r#".HDL "file" <module> <alias>"#,
        "Load a Verilog-A module."
    ),
    command!(
        ".vec",
        [],
        Files,
        Rare,
        Supported,
        None,
        ".VEC 'file'",
        "Digital vector file."
    ),
    command!(
        ".protect",
        [".prot"],
        Files,
        Rare,
        Supported,
        None,
        ".PROTECT\n.PROT <method>",
        "Start of a protected (hidden or encrypted) region."
    ),
    command!(
        ".unprotect",
        [".unprot"],
        Files,
        Rare,
        Supported,
        None,
        ".UNPROTECT",
        "End of a protected region."
    ),
    command!(
        ".model",
        [],
        Models,
        Common,
        Supported,
        Some(".MODEL ${1:name} ${2:NMOS} LEVEL=${3}"),
        ".MODEL mname type <LEVEL=n> <VERSION=v> <p1=v1 ...>",
        "Model card; parameters may be wrapped in parentheses.",
        [
            ("mname", "Model name used by elements"),
            (
                "type",
                "Model type: NMOS, PMOS, NPN, PNP, D, R, C, L, SW ..."
            ),
            ("LEVEL", "Model level (e.g. 54 for BSIM4)"),
            ("p1=v1 ...", "Model parameters"),
        ]
    ),
    command!(
        ".subckt",
        [".macro"],
        Models,
        Common,
        Supported,
        Some(".SUBCKT ${1:name} ${2:ports}\n\t$0\n.ENDS ${1:name}"),
        ".SUBCKT name n1 <n2 ...> <p=val ...>",
        "Start of a subcircuit definition.",
        [
            ("name", "Subcircuit name used by X instances"),
            ("n1 ...", "Port nodes in order"),
            ("p=val ...", "Parameters with default values"),
        ]
    ),
    command!(
        ".ends",
        [".eom"],
        Models,
        Common,
        Supported,
        None,
        ".ENDS <name>",
        "End of a subcircuit definition.",
        [("name", "Optional name of the subcircuit being closed"),]
    ),
    command!(
        ".alias",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".ALIAS model1 model2",
        "Model alias."
    ),
    command!(
        ".malias",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".MALIAS model=alias1 <alias2 ...>",
        "Multiple model aliases."
    ),
    command!(
        ".appendmodel",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".APPENDMODEL Src kw1 Dest kw2",
        "Append model parameters."
    ),
    command!(
        ".variation",
        [],
        Models,
        Occasional,
        Supported,
        None,
        ".VARIATION ... .END_VARIATION",
        "Start of a variation block (own sub-language)."
    ),
    command!(
        ".end_variation",
        [],
        Models,
        Occasional,
        Supported,
        None,
        ".END_VARIATION",
        "End of a variation block."
    ),
    command!(
        ".global_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".GLOBAL_VARIATION ... .END_GLOBAL_VARIATION",
        "Start of a global (process) variation block."
    ),
    command!(
        ".end_global_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".END_GLOBAL_VARIATION",
        "End of a global (process) variation block."
    ),
    command!(
        ".local_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".LOCAL_VARIATION ... .END_LOCAL_VARIATION",
        "Start of a local (mismatch) variation block."
    ),
    command!(
        ".end_local_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".END_LOCAL_VARIATION",
        "End of a local (mismatch) variation block."
    ),
    command!(
        ".element_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".ELEMENT_VARIATION ... .END_ELEMENT_VARIATION",
        "Start of a element variation inside a local variation block."
    ),
    command!(
        ".end_element_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".END_ELEMENT_VARIATION",
        "End of a element variation inside a local variation block."
    ),
    command!(
        ".spatial_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".SPATIAL_VARIATION ... .END_SPATIAL_VARIATION",
        "Start of a spatial variation block."
    ),
    command!(
        ".end_spatial_variation",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".END_SPATIAL_VARIATION",
        "End of a spatial variation block."
    ),
    command!(
        ".mosra",
        [],
        Models,
        Rare,
        Supported,
        Some(".MOSRA RELTOTALTIME=${1:time} RELSTEP=${2:step}"),
        ".MOSRA RelTotalTime=t <RelStartTime=t> <DEC=n|LIN=n|RelStep=t> <RelMode=0|1|2> <SimMode=0|1|2|3> <AgingStart=t> <AgingStop=t> <AgingPeriod=t> <AgingWidth=t> <AgingInst=\"inst\"> <...>",
        "MOSFET HCI/BTI reliability (aging) analysis: a fresh pre-stress transient run followed by post-stress runs.",
        [
            (
                "RelTotalTime",
                "Final reliability time of the post-stress simulation (required)"
            ),
            ("RelStep", "Post-stress time step (or DEC/LIN points)"),
            ("RelMode", "0 HCI and BTI, 1 HCI only, 2 BTI only"),
            (
                "SimMode",
                "0 pre-stress, 1 post-stress, 2 both, 3 continual through .ALTER"
            ),
            (
                "AgingStart/AgingStop",
                "Transient window in which stress is computed"
            ),
            (
                "AgingInst",
                "Quoted MOSFET instances to age (wildcards allowed)"
            ),
        ]
    ),
    command!(
        ".mosra_subckt_pin_volt",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".MOSRA_SUBCKT_PIN_VOLT subckt=n1,..",
        "MOSRA subcircuit pin voltages."
    ),
    command!(
        ".ivth",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".IVTH model Ivth0=v DW=v DL=v VDSMIN=v",
        "Constant-current threshold voltage definition."
    ),
    command!(
        ".flat",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".FLAT <subckt> elem",
        "Flatten a subcircuit."
    ),
    command!(
        ".cfl_prototype",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".CFL_PROTOTYPE fn(type,..)",
        "Custom function prototype."
    ),
    command!(
        ".module",
        [],
        Models,
        Rare,
        Supported,
        None,
        ".MODULE label",
        "Module declaration (closing .ENDMODULE unverified)."
    ),
    command!(
        ".ibis",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".IBIS 'name' file='f' component='c'",
        "IBIS buffer file."
    ),
    command!(
        ".ebd",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".EBD name file= component=",
        "IBIS electrical board description."
    ),
    command!(
        ".icm",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".ICM name file= model=",
        "IBIS interconnect model."
    ),
    command!(
        ".pkg",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".PKG name file= model=",
        "IBIS package model."
    ),
    command!(
        ".fsoptions",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".FSOPTIONS name <ACCURACY=...>",
        "Field solver options."
    ),
    command!(
        ".layerstack",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".LAYERSTACK name <BACKGROUND=m> <LAYER=(m,thick)...>",
        "Field solver layer stack."
    ),
    command!(
        ".material",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".MATERIAL name METAL|DIELECTRIC <ER=> <UR=> <CONDUCTIVITY=> <LOSSTANGENT=> <ROUGHNESS=>",
        "Field solver material."
    ),
    command!(
        ".shape",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".SHAPE name RECTANGLE|CIRCLE|POLYGON|STRIP|TRAPEZOID ...",
        "Field solver conductor shape."
    ),
    command!(
        ".ba_acheck",
        [],
        Advanced,
        Rare,
        Supported,
        None,
        ".BA_ACHECK ...",
        "Behavioral AC check."
    ),
    command!(
        ".plot",
        [],
        Output,
        Rare,
        Obsolete,
        None,
        ".PLOT antype ov1 <(lo,hi)> ...",
        "Obsolete line-printer plot; use .PRINT or .PROBE."
    ),
    command!(
        ".graph",
        [],
        Output,
        Rare,
        Obsolete,
        None,
        ".GRAPH antype <MODEL=mname> ov ...",
        "Obsolete graph output; use .PROBE."
    ),
    command!(
        ".width",
        [],
        Control,
        Rare,
        Obsolete,
        None,
        ".WIDTH OUT=80|132",
        "Obsolete output width."
    ),
    command!(
        ".net",
        [],
        Control,
        Rare,
        Obsolete,
        None,
        ".NET input <RIN=v>",
        "Obsolete network analysis."
    ),
    command!(
        ".acdcfactor",
        [],
        Control,
        Rare,
        Obsolete,
        None,
        ".ACDCFACTOR ...",
        "Obsolete AC/DC factor command."
    ),
    command!(
        ".deginfo",
        [],
        Control,
        Rare,
        Obsolete,
        None,
        ".DEGINFO ...",
        "Obsolete degradation info command."
    ),
    command!(
        ".func",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".FUNC name(args) expr",
        "Not HSPICE (PSpice/ngspice); use .PARAM f(x)=expr."
    ),
    command!(
        ".change",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".CHANGE ...",
        "Not HSPICE."
    ),
    command!(
        ".bisection",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".BISECTION ...",
        "Not HSPICE; use .MODEL m OPT METHOD=BISECTION."
    ),
    command!(
        ".wide",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".WIDE",
        "Not HSPICE."
    ),
    command!(
        ".step",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".STEP ...",
        "Not HSPICE (LTspice); use a sweep in .DC/.AC/.TRAN."
    ),
    command!(
        ".control",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".CONTROL",
        "Not HSPICE (ngspice control block)."
    ),
    command!(
        ".endc",
        [],
        Control,
        Rare,
        Foreign,
        None,
        ".ENDC",
        "Not HSPICE (ngspice control block end)."
    ),
];

/// Looks up a dot command by name or alias (case-insensitive, with the leading dot).
///
/// # Arguments
/// * `name` - command as written, e.g. `.TRAN`
pub fn find_command(name: &str) -> Option<&'static CommandSpec> {
    find_named(COMMANDS, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finds_by_name_alias_and_mixed_case() {
        assert_eq!(find_command(".TRAN").map(|c| c.name), Some(".tran"));
        assert_eq!(find_command(".Meas").map(|c| c.name), Some(".measure"));
        assert_eq!(find_command(".EOM").map(|c| c.name), Some(".ends"));
        assert_eq!(find_command(".temperature").map(|c| c.name), Some(".temp"));
        assert_eq!(find_command(".macro").map(|c| c.name), Some(".subckt"));
        assert!(find_command(".nonsense").is_none());
    }

    #[test]
    fn status_flags_are_set() {
        assert_eq!(
            find_command(".plot").map(|c| c.status),
            Some(Status::Obsolete)
        );
        assert_eq!(
            find_command(".func").map(|c| c.status),
            Some(Status::Foreign)
        );
        assert_eq!(
            find_command(".step").map(|c| c.status),
            Some(Status::Foreign)
        );
        assert_eq!(
            find_command(".tran").map(|c| c.status),
            Some(Status::Supported)
        );
    }

    #[test]
    fn names_are_lowercase_dotted_and_unique() {
        let all: Vec<&str> = COMMANDS
            .iter()
            .flat_map(|c| std::iter::once(c.name).chain(c.aliases.iter().copied()))
            .collect();
        assert!(
            all.iter()
                .all(|n| n.starts_with('.') && *n == n.to_ascii_lowercase())
        );
        assert_eq!(all.len(), all.iter().collect::<HashSet<_>>().len());
    }

    #[test]
    fn texts_are_non_empty_and_snippets_start_with_name() {
        for command in COMMANDS {
            assert!(!command.synopsis.is_empty() && !command.description.is_empty());
            assert!(
                command
                    .arguments
                    .iter()
                    .all(|argument| !argument.description.is_empty())
            );
            if let Some(snippet) = command.snippet {
                let starts_with_spelling = std::iter::once(command.name)
                    .chain(command.aliases.iter().copied())
                    .any(|spelling| snippet.to_ascii_lowercase().starts_with(spelling));
                assert!(starts_with_spelling, "{}", command.name);
            }
        }
    }
}
