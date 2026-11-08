# HSPICE language reference (condensed)

Compiled from: HSPICE User Guide: Simulation and Analysis E-2010.12, HSPICE Reference Manual: Commands and
Control Options E-2010.12, MOSFET Models D-2010.12, Signal Integrity guide E-2010.12, Synopsys quick
reference cards K-2015.06 / M-2017.03, HSPICE Quick Reference U-2003.03, Star-Hspice 1998.2, IBIS-ISS 1.0,
Sandia SAND2018-14272 (behaviour tested on HSPICE M-2017.03).

Tags: **[V]** verified by ≥2 sources or explicit manual text, **[1]** single source, **[CONFLICT]** sources
disagree, **UNVERIFIED** no source found. Rule for the language server: constructs that are UNVERIFIED or
CONFLICT must be *accepted silently* (no error diagnostics).

---

## 1. Lexical rules

### Title line
- Top-level netlist: the **first line is always the title**, whatever it contains (a `.TRAN` on line 1 is not
  executed). `.TITLE text` may be used instead. [V]
- Included files: "the first line of an included file is a normal line and not a comment". [1]
- Several decks may be concatenated in one file, each ending with `.END`; the line after an `.END` is the
  title of the next deck. [V]
- `.ALTER title` carries its own title. `.TITLE` text ≤ 73 chars (not enforced).

### Comments
- `*` as the **first non-blank** character → full-line comment. [V]
- `$` as the first non-blank character → full-line comment. [V]
- Inline `$` comment: must be preceded by whitespace, a comma, or a numeric value (`.param a=1k$c` → a=1e3).
  Otherwise `$` is a legal name character (`net$1`). `$` inside quotes is not a comment. [V]
- `//` is **not** an HSPICE comment (UNVERIFIED, most likely unsupported). `;` is not a comment.

### Continuation
- `+` as the first non-blank character continues the previous line; acts as a token delimiter. [V]
- Comment lines (`*`, `$`) may appear **between** a line and its `+` continuation (manual example). [1]
- `\\` (or `\`) at end of line also continues (inside a token/string too); exact whitespace semantics
  UNVERIFIED. [V for existence]
- Blank lines between a line and its continuation: UNVERIFIED.

### Delimiters
- Space, tab, comma, `(`, `)`. `=` only between a name and its value. `'` and `"` delimit expressions and
  file names. `:` delimits element attributes (`M1:VGS`), `.` marks hierarchy. [V]
- Therefore `PULSE(0 1 …)`, `PULSE 0 1 …`, `SIN (0 1 1MEG)` are equivalent.

### Case
- Case-insensitive except file names / paths (case-sensitive since 2009.09). `-case 1` makes names
  case-sensitive (keywords/commands/options always case-insensitive). [V]

### Line length
- [CONFLICT] 1024 chars per logical line vs "no limit". Do not enforce.

### Names (SA Table 10)
| Char | Node name | Element/param name (after key letter) |
|---|---|---|
| `~ ! @ # % & _ < > ? / \| ` | anywhere | not first; `/ < >` illegal in param names |
| `^` | anywhere (avoid in params) | not first |
| `$` | not first; avoid right after a digit | not first |
| `*` | not first (avoid; wildcard in .print) | not first |
| `- +` | not first | not first; avoid in params |
| `{ }` | not first (converted to `[ ]`) | not first |
| `[ ] : ;` | not first | not first; `:` illegal in params |
| `.` | [CONFLICT] (hierarchy separator) | not first |
| `( ) , = ' "` whitespace | illegal | illegal |

- Parameter names start with a letter. Subckt names cannot start with a digit. [V]
- Numeric node names 0…999999999999999; 0 is ground. Trailing letters on numeric nodes: [CONFLICT].
- Hierarchical references `X1.X2.n1` in `.PRINT/.PROBE/.IC/.NODESET/.MEASURE`, wildcards allowed
  (`v(x*.*)`). Short form `10:M5`. `valp(x1/p1)` uses `/` for subckt param paths. [V]
- **Ground aliases**: `0`, `GND`, `GND!`, `GROUND`, `!GND`. Ground may not appear in a `.SUBCKT` port list. [V]

### Reserved words (context dependent, SA Tables 14–15) [1]
- Sources reject param names: AC DC PULSE PU PWL PL SIN EXP SFFM AM PAT PE R RD LFSR DATA.
- E/G elements reject: POLY PWL AND NAND OR NOR LAPLACE POLE FREQ FOSTER DELAY OPAMP TRANSFORMER VCR VCCAP
  VCVS VCCS NOISE MAX SCALE PAR.
- `time`, `temper`, `hertz` cannot be redefined (`.param temper=100` is illegal). [V]

## 2. Numbers
- Integer, float, exponent with `E` or `D` (`1D-3`), or scale factor. Exponent + scale factor together is
  illegal (`1e-6u`). [V]
- Scale factors (case-insensitive): `T`=1e12 `G`=1e9 `MEG`=1e6 `X`=1e6 `K`=1e3 `MIL`=25.4e-6 `M`=1e-3
  `U`=1e-6 `N`=1e-9 `P`=1e-12 `F`=1e-15 `A`=1e-18. [V]
- Matching order after the numeral: `MEG`, then `MIL`, then single letters. `M` = milli. `1F` = 1 femto,
  `1MS` = 1 milli. [V]
- Trailing letters after the number/scale factor are unit text and ignored: `10pF`=10p, `1kV`=1k, `1w`=1,
  `1.5VOLTS`=1.5. Special case: `20amps` = 20 (not 20e-18). [V]
  → Rule: a single trailing letter is a scale factor only if it is one of the scale letters; `A` followed by
  more letters (e.g. `amps`) is unit text. [V]
- PCB infix forms `3k3`=3.3k, `0u1`=0.1u, `5R6`=5.6 (`.OPTION PCB_SCALE_FORMAT`). [1] Optional support.

## 3. Expressions and parameters
- Quotes: "Enclose a complex expression in single quotes unless the expression begins with an alphabetic
  character and contains no spaces." Manuals' own examples use unquoted expressions widely
  (`.PARAM pWidth =nWidth * npRatio`, `R1 1 0 r=ABS(v(1)/i(m1))+10`, `Rterm in gnd R=sqrt(HERTZ)`,
  `Evco out 0 VOL=v0+gain*SIN(…)`). Strict parser (≥2009.03): `.param b=-a` is an error, use `'-a'`.
  → Accept quoted and unquoted; warn on unquoted `.param` values that start with `-` or contain spaces.
- Bare parameter names as values: `Rload 1 GND RVAL`, `VB 2 0 DC=VCC`. [V]
- Double quotes also delimit expressions (`.par x="temper/2"`). `{}` is **not** an expression delimiter. [V]
- String params: `.param m=str('nch')`, used as `str(pmod)` (e.g. MOSFET model slot). [V]
- Operators: `+ - * /`, `**` and `^` (power), `< <= > >= == !=`, `&& ||`, `cond ? a : b`, unary `- +`.
  `!`, single `&`/`|`: UNVERIFIED. Precedence: undocumented (use C-like).
- `x**y`: x<0 uses integer part of y. `pow(x,y)` integer part of y; `pwr(x,y)` = sign(x)·|x|^y.

### Built-in functions [V]
| Arity | Functions |
|---|---|
| 1 | sin cos tan asin acos atan sinh cosh tanh abs sqrt log log10 exp db int nint sgn def |
| 2 | pow pwr sign min max |
| 2–3 | unif(nom, rel[, mult]) aunif(nom, abs[, mult]) |
| 3–4 | gauss(nom, rel, nsig[, mult]) agauss(nom, abs, nsig[, mult]) |
| 2 | limit(nom, abs) |
| 3–4 | opt-style `OPTxxx(init, lo, hi[, delta])` (name is user-chosen; accept any call starting with `opt`) |
- `sqrt log log10 db` keep the sign of the argument. `sign(x,y)` = |x| with sign of y. `def(x)` = 1 if param
  x is defined.
- Other callables: `val(elem)`, `val(elem.param)`, `val(type:model.param)`, `valm(elem.modelparam)`,
  `valp(param)`, `lv<n>(elem)`, `lx<n>(elem)`, `v(n)`, `v(n1,n2)`, `i(elem)`, `p(elem)`, `par('expr')`,
  `str('s')`.
- `floor`, `ceil`: UNVERIFIED → accept.
- Special variables: `time`, `temper`, `hertz` (not redefinable). `pi`: UNVERIFIED (manual example defines
  `.PARAM Pi=355/113`) → accept as a known name without error.
- A param defined with `v()`/`i()` is evaluated once at t=0. Expressions with `v()`/`i()` allowed directly
  only in R, C, L, E, G values.

### User functions [V]
- `.PARAM f(a,b)=expr g(d)=…` (several per line). There is **no `.FUNC`** in HSPICE.
- ≥1 argument; nesting depth [CONFLICT] 2 or 3; names must differ from built-ins.

### Param commands and scoping [V]
- `.PARAM`, `.PARAMETER`, `.PARAMETERS` (IBIS-ISS: any truncation down to `.PARA`).
- `.PARAM n=val|'expr'|n2|Dist(args)|OPTxxx(init,lo,hi[,delta])|str('s') [n2=…]`. Bare `.PARAM` illegal.
- `.SUBCKT name n1 n2 p=v …` — bare `key=val` defaults; **no `PARAMS:` keyword** (accept leniently).
- Instance overrides `X1 a b name p=v`. A `.PARAM` inside a subckt is local to it and below.
- `.OPTION PARHIER=GLOBAL|LOCAL` (default GLOBAL). GLOBAL priority: sweep > .PARAM > instance > subckt
  default. LOCAL: sweep > instance > subckt default > .PARAM.
- Duplicate definitions: last wins silently, even after use. A param must be defined before another param
  definition uses it.

## 4. Elements
General: nodes and model come first, `key=val` pairs follow in any order. `M=` (multiplier) on all elements
except voltage sources. [V]

| Letter | Syntax | Nodes | Reference |
|---|---|---|---|
| R | `Rxxx n1 n2 [mname] [R=]value\|'expr' [TC1=] [TC2=] [SCALE=] [M=] [AC=] [DTEMP=] [L=] [W=] [C=] [NOISE=]`; TC1 TC2 also positional after value | 2 | optional R model |
| C | `Cxxx n1 n2 [mname] [C=]val [TC1=] [TC2=] [SCALE=] [IC=] [M=] [W=] [L=] [DTEMP=]`; `C=equation [CTYPE=]`; `POLY c0 c1…`; `Q='expr'` | 2 | optional C model (model wins over same-named param) |
| L | `Lxxx n1 n2 [L=]val [TC1=] [TC2=] [SCALE=] [IC=] [M=] [DTEMP=] [R=]`; `L=equation [LTYPE=]`; `POLY …`; `NT=`; `[L=]INFINITY` | 2 (reluctor form variadic) | optional L model |
| K | `Kxxx Lyyy Lzzz [K=]coupling`; `Kaaa Lbbb [Lccc …] mname [MAG=]` (magnetic core) | — | inductor element names |
| D | `Dxxx n+ n- mname [[AREA=]area] [PJ=] [WP=] [LP=] [WM=] [LM=] [OFF] [IC=vd] [M=] [DTEMP=]` | 2 | required D model |
| Q | `Qxxx nc nb ne [ns] mname [area] [OFF] [IC=vbe,vce] [M=] [DTEMP=]`, `AREA= AREAB= AREAC= VBE= VCE=` | 3–4 | required NPN/PNP model |
| J | `Jxxx nd ng ns [nb] mname [[AREA=]area \| W= L=] [OFF] [IC=vds,vgs] [M=] [DTEMP=]` | 3–4 | required NJF/PJF model |
| M | `Mxxx nd ng ns [nb] mname [[L=]len] [[W=]wid] [AD=] [AS=] [PD=] [PS=] [NRD=] [NRS=] [RDC=] [RSC=] [OFF] [IC=vds,vgs,vbs] [M=] [DTEMP=] [GEO=] [DELVTO=]`; positional order L then W (`.OPTION WL` swaps); `NF` UNVERIFIED (model-specific instance params allowed) | 3–4 | NMOS/PMOS model; with `.OPTION MACMOD=1` may name a `.subckt` (3–7 terminals) |
| V | `Vxxx n+ n- [[DC=]dcval] [tranfun] [AC[=]acmag[,acphase]]`; bare `VMEAS 12 9` legal | 2 | — |
| I | same as V plus `[M=]` | 2 | — |
| E | `Exxx n+ n- [VCVS\|LEVEL=0] in+ in- gain [MAX=] [MIN=] [SCALE=] [TC1=] [TC2=] [ABS=1] [IC=]`; `POLY(n)`/`POLY=n` + n node pairs + coefficients; `PWL(1) in+ in- [DELTA=] x1,y1 …`; `AND(k) NAND(k) OR(k) NOR(k)`; `DELAY in+ in- TD= [NPDELAY=]`; `LAPLACE in+ in- k0,k1,… / d0,d1,…`; `POLE`, `FREQ`, `FOSTER`, `ZTRANS`; `OPAMP`/`LEVEL=1`; `TRANSFORMER k`/`LEVEL=2`; `VOL='expr' [MAX=] [MIN=]`; `noise=expr`; `TABLE`, `VALUE=` UNVERIFIED | 2 + controls | — |
| G | as E with `CUR='expr'` and `[M=]`; `VCR` (`LEVEL=1`), `VCCAP` with `NPWL(1)`/`PPWL(1)` (`LEVEL=2/3`), `SMOOTH=` | 2 + controls | — |
| F | `Fxxx n+ n- [CCCS] vn1 gain …`; `POLY(n) vn1 … vnn p0 p1 …`; `PWL(1) vn1 …`; gate types; `DELAY vn1 TD=` | 2 | controlling V source names |
| H | as F with `[CCVS]` and transresistance | 2 | controlling V source names |
| X | `Xxxx n1 … nN subnam [p=v …] [M=val] [S=val]`; subckt name is the last positional token; if undefined HSPICE looks for `subnam.inc` in the input dir and `.OPTION SEARCH` paths | N | subckt (MACMOD=2: may map to a MOSFET model) |
| B | `Bxxx n1 … nN file='f' model='m' key=val … [M=]` (IBIS buffer) | variadic | — |
| P | `Pxxx p n port=num [Z0=] [DC mag] [AC mag phase] [HBAC …]` | 2 | — |
| S | `Sxxx nd1 … ndN ndRef [MNAME=] [FQMODEL=] [TYPE=s\|y] [Z0=] …` | variadic | optional S model |
| T | `Txxx in refin out refout Z0= TD= [L=] [IC=v1,i1,v2,i2]`; `Z0= F= [NL=]`; `Txxx … mname L=` | 4 | optional U model |
| U | `Uxxx in1 [..in5] refin out1 [..] refout mname L= [LUMPS=]` (lossy line) | variadic | U model |
| W | `Wxxx in1 [in2…] refin out1 [out2…] refout N= L= RLGCMODEL=\|TABLEMODEL=\|UMODEL=\|FSMODEL=\|SMODEL=\|RLGCFILE=` | variadic (2N+2) | — |
- Y, N, A, Z are **not** HSPICE element letters. Verilog-A instances use `X` via `.HDL`.

### Independent source transient functions (V/I) — each list may be wrapped in optional `( )`
| Function | Arguments |
|---|---|
| `PULSE` / `PU` | `v1 v2 [td [tr [tf [pw [per]]]]] [PERJITTER= [SEED=]]` |
| `SIN` | `vo va [freq [td [q [j]]]]` |
| `EXP` | `v1 v2 [td1 [t1 [td2 [t2]]]]` |
| `PWL` | `t1 v1 [t2 v2 …] [R[=repeat]] [TD=delay]`; `PWL PWLFILE='file' [col1,[col2]]`; `PWL(TIME, pv)` with `.DATA`; `PWLZ` |
| `PL` | `v1 t1 [v2 t2 …]` (value/time order) |
| `SFFM` | `vo va [fc [mdi [fs]]]` |
| `AM` | `sa oc fm fc [td]` |
| `PAT` | `vhi vlo td tr tf tsample data\|[comp…]\|PatName [RB=] [R=] [ENCODE=DW8b10b] [RD_INIT=]` |
| `LFSR` | `vlow vhigh tdelay trise tfall rate seed [taps] [rout=] …` |
Keywords also: `DC`, `AC`, `power=`, `imp=`.

## 5. Includes and libraries
- `.INCLUDE` / `.INC` / `.INCL 'path'` — quotes single or double; unquoted absolute paths also appear in
  examples [CONFLICT] → accept both.
- `.LIB 'file' entry` (call) vs `.LIB entry` … `.ENDL [entry]` (definition). Entry names cannot start with a
  digit; first duplicate entry wins; a library cannot call its own entry, cannot contain `.ALTER`/`.END`, may
  nest calls into other files. Quoted entry names are literal (`.lib 'f' 'slow'`).
- `.DEL LIB 'file' entry` or `.DEL LIB libnumber entry`.
- `.HDL "file" [module] [alias]` (Verilog-A).
- Text after the arguments is ignored for `.include .lib .enddata .end .endl .ends .eom`.
- **Relative paths**: nested `.INCLUDE`/`.LIB` resolve relative to the **directory of the parent file**
  (working dir also tried, with a warning). Top-level: the directory where HSPICE is run (≈ netlist dir).
  `.OPTION SEARCH='dir'` adds search dirs (also for implicit `subname.inc`). `.gz` files accepted.
- Environment variables expand inside quoted paths (`.inc '$MY_OWN_INCLUDE_FILE'`). [1]
- `.PROTECT`/`.PROT` … `.UNPROTECT`/`.UNPROT`: plaintext → just hides listing. After `metaencrypt`,
  `.PROT FREELIB` (or other method word) is followed by unreadable lines (may start with `+`); the `.UNPROT`
  itself is encrypted; the next plaintext line is typically `.ENDS`/`.ENDL`. Treat everything after
  `.prot <method>` as opaque until the next recognisable dot command.

## 6. Block constructs
| Open | Close | Notes |
|---|---|---|
| `.SUBCKT name nodes [p=v]` / `.MACRO` | `.ENDS [name]` / `.EOM [name]` | name after .ENDS optional; nested definitions UNVERIFIED; duplicate port names = error |
| `.LIB entry` | `.ENDL [entry]` | |
| `.IF (cond)` / `.ELSEIF (cond)` / `.ELSE` | `.ENDIF` | nestable; `.else if` UNVERIFIED; not allowed inside: `.END .ALTER .GLOBAL .DEL LIB .MALIAS .ALIAS .LIST .NOLIST .CONNECT` |
| `.DATA name p1 p2 …` (+ value rows, or `MER`/`LAM FILE=…`) | `.ENDDATA` | ≤ 50 params |
| `.PROTECT`/`.PROT` | `.UNPROTECT`/`.UNPROT` | see encryption above |
| `.VARIATION` | `.END_VARIATION` | contains `.GLOBAL_VARIATION`, `.LOCAL_VARIATION` (with `.ELEMENT_VARIATION`), `.SPATIAL_VARIATION` and matching `.END_*`; own sub-language (lines without dot: `option`, `parameter`, `nmos nch vth0=…`) |
| `.DESIGN_EXPLORATION` | `.END_DESIGN_EXPLORATION` | |
| `.ALTER [title]` | next `.ALTER` or `.END` | last `.ALTER` must come just before `.END` |
| — | `.END [comment]` | text on the line is a comment; content after `.END` is a new deck |

---

## 7. Dot commands
`[C]` = common in university analog courses, `[c]` = sometimes. Notation: `<x>` optional, `a|b` alternatives.

### Analysis
| Command | Aliases | Synopsis | Description |
|---|---|---|---|
| `.AC` [C] | | `.AC DEC\|OCT\|LIN\|POI np fstart fstop <SWEEP spec>` ; `.AC DATA=datanm <SWEEP spec>` | AC small-signal frequency sweep (needs a source with AC value) |
| `.DC` [C] | | `.DC var1 start1 stop1 incr1 <var2 start2 stop2 incr2 \| SWEEP spec>` ; `.DC var1 DEC\|OCT\|LIN\|POI np start stop` ; `.DC var START= STOP= STEP=` ; `.DC DATA=d` ; `.DC MONTE=n` | DC sweep of a source, parameter or TEMP |
| `.TRAN` [C] | | `.TRAN tstep1 tstop1 <tstep2 tstop2 …> <START=val> <UIC> <SWEEP spec>` ; `.TRAN DATA=d` | Transient analysis |
| `.OP` [C] | | `.OP <ALL\|BRIEF\|CURRENT\|DEBUG\|NONE\|VOLTAGE> <time> <format time …> <INTERPOLATION>` | DC operating point |
| `.NOISE` [C] | | `.NOISE ov srcnam <inter> <listckt=1\|0> <listfreq=(f..)\|none\|all> <listcount=n> <listfloor=v> <listsources=1\|0\|yes\|no>` | Noise analysis (with .AC) |
| `.TF` [C] | | `.TF ov srcnam` | DC small-signal transfer function |
| `.SENS` [c] | | `.SENS ov1 <ov2 …>` | DC sensitivity |
| `.PZ` [C] | | `.PZ ov srcnam` | Pole/zero analysis |
| `.FOUR` [C] | | `.FOUR freq ov1 <ov2 …>` | Fourier analysis of transient result |
| `.FFT` [C] | | `.FFT ov <START=\|FROM=v> <STOP=\|TO=v> <NP=v> <FORMAT=NORM\|UNORM> <WINDOW=RECT\|BART\|HANN\|HAMM\|BLACK\|HARRIS\|GAUSS\|KAISER> <ALFA=v> <FREQ=v> <FMIN=v> <FMAX=v>` | Spectrum of transient result |
| `.TEMP` [C] | `.TEMPERATURE` | `.TEMP t1 <t2 …>` | Simulation temperature(s) |
| `.DISTO` | | `.DISTO Rload <inter <skw2 <refpwr <spwf>>>>` | Small-signal distortion |
| `.SAMPLE` | | `.SAMPLE FS=freq <TOL=> <NUMF=> <MAXFLD=> <BETA=0\|1>` | Sampling noise |
| `.LSTB` [c] | | `.LSTB <name> mode=single\|diff\|comm vsource=vlstb\|vlstbp,vlstbn` | Loop stability |
| `.DCMATCH` [c] | | `.DCMATCH ov <THRESHOLD=T> <FILE=s> <INTERVAL=n>` | DC mismatch |
| `.ACMATCH` [c] | | `.ACMATCH ov <THRESHOLD=T> <FILE=s> <INTERVAL=n>` | AC mismatch |
| `.DCSENS` | | `.DCSENS ov <File=s> <Perturbation=x> <Interval=v> <Threshold=x> <GroupByDevice=0\|1>` | DC sensitivity table |
| `.LIN` | | `.LIN <sparcalc=1\|0> <modelname=n> <filename=f> <format=…> <noisecalc=…> <gdcalc=…> <dataformat=ri\|ma\|db>` | Linear network / S-param extraction |
| `.TRANNOISE` | | `.TRANNOISE ov <METHOD=MC> <SEED=> <SAMPLES=> <START=> <AUTOCORRELATION=> <FMIN=> <FMAX=> <SCALE=> <PHASENOISE=> <JITTER=> <REF=> <PSD=>` | Transient noise |
| `.STATEYE` | | `.STATEYE T=t Trf=t <Tr=t> <Tf=t> Incident_port=… Probe_port=… …` | Statistical eye |
| `.POWER` | | `.POWER signal <REF=vname FROM=t1 TO=t2>` | Power table |
| `.POWERDC` | | `.POWERDC TOP\|ALL <subckt …>` | DC leakage power |
| `.SWEEPBLOCK` | | `.SWEEPBLOCK name sweepspec <sweepspec …>` | Named sweep union |
| `.STORE` | | `.STORE <file=f> <time=t> <repeat=t>` | Transient checkpoint |
| `.SURGE` | | `.SURGE threshold width node1 <node2 …>` | Current surge detection |
| `.CHECK` | | `.CHECK EDGE\|FALL\|GLOBAL_LEVEL\|HOLD\|IRDROP\|RISE\|SETUP\|SLEW …` | Timing checks |
RF analyses (accept): `.HB .HBAC .HBXF .HBLIN .HBLSP .HBNOISE .HBOSC .SN .SNAC .SNXF .SNFT .SNNOISE .SNOSC
.ENV .ENVFFT .ENVOSC .PHASENOISE .ACPHASENOISE .PTDNOISE .JITTER`.

### Output
| Command | Aliases | Synopsis | Description |
|---|---|---|---|
| `.PRINT` [C] | | `.PRINT <DC\|AC\|TRAN\|NOISE\|DISTO> ov1 <ov2 …>` (antype optional; `name=PAR('expr')`, wildcards) | Print to .lis |
| `.PROBE` [C] | | `.PROBE <antype> ov1 <ov2 …>` | Save to waveform file |
| `.MEASURE` [C] | `.MEAS` | see §8 | Measurements |
| `.BIASCHK` | | `.BIASCHK 'expr' <limit=> <noise=> <max=> <min=> <simulation=op\|dc\|tr\|all> <monitor=v\|i\|w\|l> <tstart=> <tstop=> <autostop> <interval=>` | Bias check |
| `.DOUT` | | `.DOUT nd VTH (time state …)` | Expected digital output |
| `.STIM` | | `.STIM <tran\|ac\|dc> PWL\|DATA\|VEC <filename=f> …` | Results → stimulus |
| `.LPRINT` | | `.LPRINT (v1,v2) ov …` | VCD output |
| `.MODEL_INFO` | | `.MODEL_INFO ALL \| inst1, inst2,…` | Model info |
| `.MOSRAPRINT` | | `.MOSRAPRINT name type(elem, vds=, vgs=, vbs=)` | Reliability print |

### Setup and control
| Command | Aliases | Synopsis | Description |
|---|---|---|---|
| `.OPTION` [C] | `.OPTIONS`, `.OPT` | `.OPTION opt1<=v> <opt2<=v> …>` (bare name = 1; last wins) | Simulator options |
| `.PARAM` [C] | `.PARAMETER`, `.PARAMETERS` | `.PARAM n=val\|'expr' …` ; `.PARAM f(a,b)='expr'` | Parameters / user functions |
| `.IC` [C] | | `.IC V(n1)=v1 <V(n2)=v2 …> <subckt=name>` | Transient initial conditions |
| `.NODESET` [C] | | `.NODESET V(n1)=v1 …` or `.NODESET n1 v1 …` | DC operating-point hints |
| `.DCVOLT` | | `.DCVOLT V(n1)=v1 …` or `.DCVOLT n1 v1 …` | Same as .IC |
| `.DATA` [C] | | `.DATA name p1 <p2..> v1 <v2..> … .ENDDATA` ; `.DATA name MER\|LAM FILE='f' p1=col … .ENDDATA` | Data table for sweeps |
| `.ENDDATA` | | `.ENDDATA` | End of .DATA |
| `.GLOBAL` [C] | | `.GLOBAL n1 <n2 …>` | Global nodes |
| `.TITLE` | | `.TITLE string` | Title |
| `.END` [C] | | `.END <comment>` | End of netlist (deck) |
| `.ALTER` [C] | | `.ALTER <title>` | Re-run with changes |
| `.SAVE` | | `.SAVE <TYPE=NODESET\|IC> <FILE=f> <LEVEL=ALL\|TOP\|SELECT\|NONE> <TIME=t>` | Save operating point |
| `.LOAD` | | `.LOAD <FILE=f> <RUN=PREVIOUS\|CURRENT>` | Load operating point |
| `.CONNECT` | | `.CONNECT n1 n2` | Connect two nodes |
| `.IF` / `.ELSEIF` / `.ELSE` / `.ENDIF` | | `.IF (cond)` … | Conditional block |
| `.DESIGN_EXPLORATION` / `.END_DESIGN_EXPLORATION` | | | Design exploration block |
| `.PAT` | | `.PAT name=data <RB=v> <R=n>` | Pattern definition |

### Files and libraries
| Command | Aliases | Synopsis |
|---|---|---|
| `.INCLUDE` [C] | `.INC`, `.INCL` | `.INCLUDE '<path>file'` |
| `.LIB` [C] | | `.LIB '<path>file' entry` (call) ; `.LIB entry` (definition) |
| `.ENDL` [C] | | `.ENDL <entry>` |
| `.DEL LIB` | | `.DEL LIB '<path>file' entry` |
| `.HDL` [c] | | `.HDL "file" <module> <alias>` |
| `.VEC` | | `.VEC 'file'` |
| `.PROTECT` | `.PROT` | start of protected/encrypted region |
| `.UNPROTECT` | `.UNPROT` | end of protected region |

### Models and hierarchy
| Command | Aliases | Synopsis |
|---|---|---|
| `.MODEL` [C] | | `.MODEL mname type <LEVEL=n> <VERSION=v> <p1=v1 …>` (params may be wrapped in `( )`, space or comma separated) |
| `.SUBCKT` [C] | `.MACRO` | `.SUBCKT name n1 <n2 …> <p=val …>` |
| `.ENDS` [C] | `.EOM` | `.ENDS <name>` |
| `.ALIAS` | | `.ALIAS model1 model2` |
| `.MALIAS` | | `.MALIAS model=alias1 <alias2 …>` |
| `.APPENDMODEL` | | `.APPENDMODEL Src kw1 Dest kw2` |
| `.VARIATION` [c] | | block, see §10 |
| `.MOSRA` | | `.MOSRA RelTotalTime=t <RelStartTime=t> <DEC=n\|LIN=n\|RelStep=t> <RelMode=0\|1\|2> <SimMode=0\|1\|2\|3> …` (see below) |
| `.MOSRA_SUBCKT_PIN_VOLT` | | `.MOSRA_SUBCKT_PIN_VOLT subckt=n1,..` |
| `.IVTH` | | `.IVTH model Ivth0=v DW=v DL=v VDSMIN=v` |
| `.FLAT` | | `.FLAT <subckt> elem` |
| `.CFL_PROTOTYPE` | | `.CFL_PROTOTYPE fn(type,..)` |
| `.MODULE` | | `.MODULE label` (closing `.ENDMODULE` UNVERIFIED) |

`.MOSRA` keys [V] (E-2010.12, K-2015.06): `RelTotalTime` (required), `RelStartTime`, `DEC`, `LIN`, `RelStep`,
`RelMode` (0 HCI+BTI, 1 HCI, 2 BTI), `SimMode` (0 pre-stress, 1 post-stress, 2 both, 3 continual through
`.ALTER`), `AgingStart`, `AgingStop`, `AgingPeriod`, `AgingWidth`, `AgingInst="pattern"`, `HciThreshold`,
`NbtiThreshold`, `Integmod`, `Xpolatemod`, `Tsample1`, `Tsample2`, `Agethreshold`, `MosraLife`, `DegF`,
`DegFN`, `DegFP`.

### IBIS / field solver (accept)
`.IBIS 'name' file='f' component='c'`, `.EBD name file= component=`, `.ICM name file= model=`,
`.PKG name file= model=`, `.FSOPTIONS name <ACCURACY=…>`, `.LAYERSTACK name <BACKGROUND=m> <LAYER=(m,thick)…>`,
`.MATERIAL name METAL|DIELECTRIC <ER=> <UR=> <CONDUCTIVITY=> <LOSSTANGENT=> <ROUGHNESS=>`,
`.SHAPE name RECTANGLE|CIRCLE|POLYGON|STRIP|TRAPEZOID …`, `.BA_ACHECK …`.

### Obsolete (accept with deprecation hint)
`.PLOT antype ov1 <(lo,hi)> …`, `.GRAPH antype <MODEL=mname> ov …`, `.WIDTH OUT=80|132`, `.NET input <RIN=v>`,
`.ACDCFACTOR …`, `.DEGINFO …`.

### Not HSPICE (foreign dialects → warning)
`.FUNC` (PSpice/ngspice; HSPICE uses `.PARAM f(x)=…`), `.CHANGE`, `.BISECTION` (HSPICE: `.MODEL m OPT
METHOD=BISECTION`), `.WIDE`, `.STEP` (LTspice), `.CONTROL`/`.ENDC` (ngspice).

## 8. Analysis argument details
**Sweep spec** (after `SWEEP` in `.AC/.DC/.TRAN`): `var start stop incr` | `var START=e STOP=e STEP=e` |
`var DEC|OCT|LIN np start stop` | `var POI np v1 … vnp` | `DATA=datanm` | `MONTE=n [firstrun=k]` |
`MONTE=list(n1:n2 n3)` | `OPTIMIZE=OPTxxx RESULTS=meas MODEL=optmod`. `var` = independent source, `.PARAM`
name or `TEMP`.

- `.TRAN`: `tstep tstop` minimum; multi-segment `tstep1 tstop1 tstep2 tstop2 …`; SPICE positional
  `tstep tstop tstart tmax` (3rd/4th < tstop and no START=); `START=`, `UIC`, `SWEEP`, `RUNLVL=(…)`,
  `tempvec=(…)`. Limit `tstop ≤ 1e9·tstep`.
- `.AC`: `DEC|OCT|LIN|POI np fstart fstop` (POI: list of np frequencies). Example `.AC DEC 10 1K 100MEG`.
- `.DC`: `.DC VIN 0.25 5 0.25`; nested `.DC VDS 0 10 0.5 VGS 0 5 1`; `.DC TEMP -55 125 10`;
  `.DC XVAL 1K 10K .5K SWEEP TEMP LIN 5 25 125`; `.DC DATA=d`; `.DC MONTE=n`.
- `.OP`: format keywords (first letter suffices) ALL BRIEF CURRENT DEBUG NONE VOLTAGE; times; `INTERPOLATION`.
- `.NOISE ov srcnam [inter]`, `ov` = `V(n)`, `V(n1,n2)`, `I(elem)`. Results `ONOISE`, `INOISE`.
- `.TF V(5,3) VIN`, `.SENS V(9) I(VOUT)`, `.PZ V(10) VIN`.

### `.MEASURE` / `.MEAS`
General: `.MEAS <DC|AC|TRAN|DC_CONT|AC_CONT|TRAN_CONT> result <body> <GOAL=v> <MINVAL=v> <WEIGHT=v> <PRINT=0|1>`.
`result` must not be `DC`/`AC`/`TRAN`; `LAST` reserved. Bodies:
1. `TRIG ov VAL=v <TD=t> <RISE=r|LAST> <FALL=f|LAST> <CROSS=c|LAST>` | `TRIG AT=t`, then
   `TARG ov VAL=v <TD=t|TRIG> <RISE=…> <FALL=…> <CROSS=…> <REVERSE>` | `TARG AT=t`
2. `WHEN ov=v|ov2 <TD=> <FROM=> <TO=> <RISE=|FALL=|CROSS=> <REVERSE>`
3. `FIND ov1 WHEN ov2=v|ov3 …`
4. `FIND ov1 AT=v <FROM=> <TO=>`
5. `AVG|RMS|MIN|MAX|PP|EM_AVG ov <FROM=> <TO=>`
6. `INTEG|INTEGRAL ov <FROM=> <TO=>`
7. `DERIV|DERIVATIVE ov AT=v` | `DERIV ov WHEN …`
8. `ERR|ERR1|ERR2|ERR3 meas_var calc_var <MINVAL=> <IGNOR|YMIN=> <YMAX=> <WEIGHT=> <FROM=> <TO=>`
9. `PARAM='expr'` (or `"expr"`)
10. Special: `pushout=`, `pushout_per=`, `.MEAS FFT r FIND vm|vp|vr|vi|vdb|im|ip|ir|ii|idb(sig) AT=f`,
    `.MEAS FFT r THD|SNR|SNDR|ENOB sig <nbharm=n> <maxfreq=v> <BINSIZ=n>`, `LSTB(x)`, `ACM_*`, `DCM_*`.
Keywords: TRIG TARG VAL TD RISE FALL CROSS LAST AT WHEN FIND FROM TO AVG RMS MIN MAX PP EM_AVG INTEG INTEGRAL
DERIV DERIVATIVE ERR ERR1 ERR2 ERR3 PARAM GOAL MINVAL WEIGHT PRINT REVERSE IGNOR YMIN YMAX. `power` is a
valid out_var. Measure results can be used as names in later `.MEAS PARAM=` expressions.

### Output variables (`.PRINT`, `.PROBE`, `.MEAS`)
- Voltage: `V(n)`, `V(n1,n2)`, `V(x1.x2.n)`, `Vk(elem)` (terminal k voltage), `V(inst.port)`.
- Current: `I(Vxxx)`, `I(elem)`, `In(elem)` (n = terminal 1–4), `IALL(elem)`.
- AC: `VR VI VM VP VDB VT` on `(n1<,n2>)`; currents `IR II IM IP IDB IT` and `IRn…`.
- Network: `ZIN(z) ZOUT(z) YIN(z) YOUT(z)`, `Sij(z) Zij(z) Yij(z) Hij(z)` (i,j ∈ {1,2}, z ∈ {DB,M,P,R,I}).
- Noise/distortion: `ONOISE INOISE HD2 HD3 SIM2 DIM2 DIM3` with optional `(R|I|M|P|DB)`.
- Loop gain: `LSTB`, `LSTB(DB|M|P|R|I)`. Power: `P(elem)`, `POWER`.
- Templates: `LVn(elem)`, `LXn(elem)` (MOSFET: LV9=VTH, LV10=VDSAT, LX2=VGS, LX3=VDS, LX4=IDS, LX7=gm,
  LX8=gds, LX9=gmbs, LX18=CGGBO).
- Expressions: `PAR('expr')`, `name=PAR('expr')`, `deriv('v(x)')`, `integ('v(x)')`; wildcards.

## 9. `.MODEL` types and MOSFET data
Model types: `AMP C CORE D L NJF PJF NMOS PMOS NPN PNP OPT R U W S`; also `SP`, `MOSRA`, `PLOT`/`PLOTQ`
(obsolete). `SW`/`CSW` are **not** HSPICE (PSpice/ngspice; flag). Verilog-A module names as model types:
UNVERIFIED (accept).

Element → model type compatibility: M→NMOS/PMOS, Q→NPN/PNP, J→NJF/PJF, D→D, R→R, C→C, L→L/CORE(K),
U/T→U, W→W, S→S.

### MOSFET LEVEL map
1 Shichman-Hodges · 2 Grove-Frohman (SPICE 2G) · 3 SPICE 2G empirical · 4 Grove-Frohman 2E.3 · 5 AMI-ASPEC ·
6/7 Lattin-Jenkins-Grove · 8 advanced LEVEL 2 · 13 BSIM1 · 27 SOSFET · 28 modified BSIM · 38 Cypress depletion ·
39 BSIM2 · 40 HP a-Si TFT · 41 TI analog · 46 SGS-Thomson MOS3 · 47 BSIM3v2 · **49 BSIM3v3 (HSPICE)** ·
50 Philips MOS9 · **53 BSIM3v3 (Berkeley)** · **54 BSIM4** · 55 EKV 2.6 · 57 BSIM3-SOI PD · 58 UFSOI ·
59 BSIM3-SOI FD · 60 BSIM3-SOI DD · 61 RPI a-Si TFT · 62 RPI poly-Si TFT · 63 Philips MOS11 · 64 HiSIM ·
65 SSIMSOI · 66 HVMOS · 68 HiSIM2 · 69 PSP · 70 BSIMSOI4 · 71 TFT · **72 BSIM-CMG** · 73 HiSIM-HV ·
74 MOS Model 20 · 77 BSIM6 (UNVERIFIED). `VERSION=` selects the model release.

### Common NMOS/PMOS model parameters
LEVEL (1), VERSION (3.3), PARAMCHK, TOX (TOXE/TOXP/TOXM in BSIM4), VTH0 (VTO in levels 1–3), U0 (UO), K1, K2,
NCH (NDEP), NSUB, XJ, VSAT, LINT, WINT, RDSW, RSH, CJ, CJSW, CJSWG, MJ, MJSW, PB, PBSW, JS, JSW, CGSO, CGDO,
CGBO, PCLM, DVT0, ETA0, NFACTOR, VOFF, CAPMOD, MOBMOD, NOIMOD, ACM, XPART, KF, AF, LMIN, LMAX, WMIN, WMAX,
BINUNIT, TNOM. Classic levels 1–3: KP, GAMMA, PHI, LAMBDA, LD, VTO, UO.

`OPT` model args: METHOD=BISECTION|PASSFAIL, LEVEL=1, CLOSE, MAX, CUT, DIFSIZ, GRAD, PARMIN, RELIN, RELOUT,
ABSOUT, ITROPT, ABSIN, DYNACC, CENDIF.

Binned models: `nch.1 … nch.N` with LMIN/LMAX/WMIN/WMAX; elements reference the base name `nch`.

## 10. `.OPTION` names (flag = bare name sets 1)
| Option | Value | Default | Description |
|---|---|---|---|
| POST [C] | flag/0–3/ASCII/BINARY/CSDF | 0 | Waveform output (1 binary, 2 ASCII, 3 New-Wave) |
| POST_VERSION | 9007/9601/2001 | 9601 | Waveform format |
| PROBE [C] | flag | 0 | Only .PROBE/.PRINT variables in waveform file |
| CSDF | flag | 0 | Viewlogic CSDF format |
| PSF | 0/1/2 | 0 | Cadence PSF output |
| ARTIST | 0/1/2 | 0 | Cadence Artist interface |
| INGOLD [C] | 0/1/2 | 0 | .lis number format |
| NUMDGT [C] | int | 4 | Digits in .lis |
| MEASDGT [C] | int | 4 | Digits in measure output |
| MEASOUT | flag | 0 | Measure results to ASCII file |
| MEASFORM | 0–3 | 0 | Measure output format |
| MEASFAIL | 0/1 | 1 | Failed measure printing |
| MEASFILE | 0/1 | 0 | Single measure file |
| PUTMEAS | 0/1 | 1 | Measures into waveform file |
| AUTOSTOP / AUTOST [c] | flag/'expr' | 0 | Stop .TRAN when all measures done |
| LIST [c] | 0–3 | 0 | Element/parameter listing |
| NODE [c] | flag | 0 | Node cross-reference table |
| NOMOD [C] | flag | 0 | Suppress model printout |
| BRIEF [C] | flag | 0 | Suppress netlist echo |
| OPTS | flag | | Print option settings |
| CAPTAB [c] | flag | 0 | Node capacitance table |
| DCCAP [c] | flag | 0 | Capacitances in DC sweep |
| OPFILE | 0/1/2 | 0 | Operating point to file |
| LIS_NEW | flag | 0 | Streamlined .lis |
| UNWRAP [c] | flag | 0 | Unwrapped AC phase |
| ACOUT | 0/1 | 0 | AC difference calculation |
| VFLOOR | num | | Minimum printed voltage |
| INTERP | flag | 0 | Output only at tstep multiples |
| POSTLVL | int | | Write only nodes at hierarchy level n |
| POSTTOP | int | | Write only the top n hierarchy levels |
| WARN | 0/1 | 1 | SOA voltage warnings |
| NOWARN | flag/0/1 | 0 | Suppress conflicting-parameter warnings |
| WARNLIMIT / WARNLIM | int | | Limit repeated warnings |
| MESSAGE_LIMIT | 'idx:n' | | Per-message limit |
| LIMPTS | int | 2001 | Max AC print points |
| LIMTIM | s | 2 | Reserved CPU time |
| CPTIME | s | | Max CPU time |
| BEEP BADCHR BINPRNT STATFL VERIFY NXX NOTOP NOELCK | flag | 0 | Misc |
| SEARCH [C] | 'path' | | Library/include search path |
| PARHIER / PARHIE [c] | GLOBAL/LOCAL | GLOBAL | Parameter scoping |
| SCALE [C] | num | 1 | Element geometry scale |
| SCALM | num | 1 | Model scale |
| GEOSHRINK | num | | Shrink factor |
| TNOM [C] | °C | 25 | Reference temperature |
| XDTEMP | flag | 0 | DTEMP interpretation |
| WL | flag | 0 | Swap MOSFET positional W/L |
| DEFL / DEFW | m | 100u | Default MOSFET L/W |
| DEFAD DEFAS DEFPD DEFPS DEFNRD DEFNRS | num | 0 | Default MOSFET diode geometry |
| ALTCHK | flag | 0 | Topology check after .ALTER |
| ALTCC | -1/0/1 | 0 | Read netlist once for .ALTER |
| REDEFSUB | 0/1/2 | 0 | Allow subckt redefinition |
| HIER_DELIM / HIER_SCALE | flag | 0 | Hierarchy delimiter / scaling |
| VAMODEL / SPMODEL | [=name] | | Verilog-A vs SPICE model |
| CMIFLAG / TMIFLAG / TMIPATH | | | Custom model interfaces |
| MACMOD | 0/1/2 | | M element may call subckt (1) / X may map to model (2) |
| GENK / KLIM | | | Mutual inductors |
| EM_RECOVERY | num | 1 | EM recovery factor |
| RUNLVL [C] / RUN_LVL | 0–6 | 3 | Speed/accuracy level (spelling RUN_LVL UNVERIFIED, reported for newer releases) |
| ACCURATE [C] | flag | 0 | Accurate timestep control |
| FAST | flag | 0 | Faster, less accurate |
| METHOD [C] | GEAR/TRAP/BDF | TRAP | Integration method |
| MAXORD | 1–3 | 2 | GEAR order |
| PURETP | flag | 0 | Pure trapezoidal |
| MU | num | 0.5 | Integration coefficient |
| LVLTIM | 1–4 | 1 | Timestep algorithm |
| DVDT | 0–4 | 4 | Timestep from dV/dt |
| RELTOL [C] | num | 1e-3 | Relative tolerance |
| ABSTOL [C] | A | 1e-9 | Absolute current tolerance |
| ABSI | A | 1e-9 | Absolute current tolerance of diodes, BJTs and JFETs |
| VNTOL [C] / ABSV | V | 50u | Absolute voltage tolerance |
| CHGTOL [C] | C | 1e-15 | Charge tolerance |
| RELV | num | 1e-3 | Relative voltage tolerance |
| RELI | num | 0.01 | Relative current tolerance |
| RELQ | num | 0.01 | Relative charge tolerance |
| RELVAR / ABSVAR | num | 0.3/0.5 | Voltage change limits |
| RELMOS / ABSMOS | num | 0.05/1u | MOSFET current tolerances |
| RELVDC / ABSVDC | num | /50u | DC voltage tolerances |
| RELH / ABSH | num | /0 | Branch current tolerances |
| TRTOL | num | 7 | Truncation error tolerance |
| DELMAX [C] | s | auto | Max internal timestep |
| RMAX / RMIN | num | 5/1e-9 | Timestep limits |
| FS / FT | num | 0.25 | Timestep reduction factors |
| IMIN / ITL3, IMAX / ITL4 | int | 3/8 | Timestep iteration counts |
| ITL1 [C] | int | 200 | DC iteration limit |
| ITL2 | int | 50 | DC transfer-curve limit |
| ITL5 | int | 0 | Transient iteration limit |
| GMIN [C] | S | 1e-12 | Junction conductance (tran) |
| GMINDC [C] | S | 1e-12 | Junction conductance (DC) |
| GSHUNT / CSHUNT | num | 0 | Node-to-ground shunts |
| GSHDC / CSHDC | num | | DC-only shunts |
| GRAMP | flag | 0 | Ramp GMINDC |
| DCON | int | 0 | Autoconverge |
| CONVERGE [c] | -1–5,100 | | Convergence method |
| DCSTEP | s | 0 | Capacitors as conductances in DC |
| DCHOLD | int | 1 | Hold .NODESET iterations |
| DCIC | 0/1 | | Use .IC in DC |
| DCFOR DCTRAN SYMB OFF NOPIV PIVOT PIVTOL KCLTEST NEWTOL MAXAMP IC_ACCURATE | | | Convergence aids |
| ICSWEEP | 0/1 | 1 | Reuse previous sweep point |
| BYPASS | 0/1/2 | 2 | Latent device bypass |
| BYTOL | V | 100u | Bypass tolerance |
| DV / DI | num | 1000/100 | Max iteration change |
| DVTR | V | 1k | Transient voltage limit |
| EXPMAX | num | 80 | Exponent limit |
| GMAX | S | 100 | Max conductance for .IC/.NODESET |
| SIM_ACCURACY | num | 1 | RF accuracy |
| RISETIME / RISETI, WACC | | | Transmission lines |
| DIAGNOSTIC / DIAGNO | flag | | Log negative conductances |
| ITLPZ PZABS PZTOL FSCAL GSCAL CSCAL LSCAL | | | Pole-zero |
| FFTOUT FFT_ACCURATE | flag | 0 | FFT |
| SEED | x/'random' | 1 | Monte Carlo seed |
| MCBRIEF | 0–3 | 0 | Monte Carlo output |
| MODMONTE | flag | 0 | Per-model Monte Carlo |
| MONTECON | 0/1 | 1 | Continue after failed MC run |
| SAMPLING_METHOD | SRS/LHS/Factorial/OFAT/Sobol/Niederreiter | SRS | Sampling |
| REPLICATES | int | | Replicates |
| RANDGEN | 0/'moa'/1 | 0 | Random generator |
| SI_SCALE_SYMBOLS | 0/1 | 0 | With -case 1: M = mega, m = milli |
| PCB_SCALE_FORMAT | 0/1 | 0 | Infix scale factors |
Obsolete options (deprecation hint): ACCT ALT999 ALT9999 BKPSIZ CDS CO H9007 MEASSORT MENTOR MODSRH PIVREF NOPAGE
PIVREL PLIM SDA SPICE SIM_LA_MINMODE ZUKEN.

Options are numerous; unknown options are only a hint. This table lists the common ones; the complete list
(every option of the E-2010.12 reference manual, including BA_*, HB*/SN*, SIM_LA/DSPF/SPEF/POWER*, MOSRA
and pole/zero options) is `hspice-lsp/src/lang/options.rs`.

## 11. Monte Carlo / variation
- `.PARAM TOX=AGAUSS(200,20,3) XL=AGAUSS(0.1u,0.13u,3)`; `UNIF AUNIF GAUSS AGAUSS LIMIT`.
- `.TRAN 10n 1u SWEEP MONTE=30`, `SWEEP MONTE=10 firstrun=11`, `SWEEP MONTE=list(10 20:30)`, `.DC MONTE=n`.
- Model tolerances: `.MODEL m type (LEVEL=n p=val LOT</n></dist>=v DEV</n></dist>=v …)`, e.g.
  `vth0=0.5 dev/gauss=0.01` (layout partly UNVERIFIED).
- `.VARIATION` block: `option <name>=<val>` (no dot), `parameter p=val`, `.GLOBAL_VARIATION` … with
  `parameter a=N()`, `U()`, `CDF(x1 y1 …)`, `nmos nch vth0=0.01 u0=10 %`; `.LOCAL_VARIATION` with
  `.ELEMENT_VARIATION` (`R r=10 %`), `.SPATIAL_VARIATION`; functions `N() U() CDF() perturb() get_E() get_P()`.
