//! Unit tests of the rule families on small inline netlists.

use super::{Severity, compute};
use crate::analysis::{ExternalContext, analyze};
use crate::config::DiagnosticSettings;
use crate::state::document::parse_document;

/// Runs all diagnostics on a top-level netlist with complete context and returns (code, severity) pairs.
///
/// # Arguments
/// * `text` - netlist text, first line is the title
fn run(text: &str) -> Vec<(&'static str, Severity)> {
    run_with(text, true, true, &DiagnosticSettings::default())
}

/// Runs all diagnostics with explicit document kind, context completeness and settings.
///
/// # Arguments
/// * `text` - netlist text
/// * `top_level` - whether the first line is a title
/// * `complete` - whether the context is complete
/// * `settings` - diagnostic toggles
fn run_with(
    text: &str,
    top_level: bool,
    complete: bool,
    settings: &DiagnosticSettings,
) -> Vec<(&'static str, Severity)> {
    let syntax = parse_document(text, top_level);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete,
            ..Default::default()
        },
    );
    compute(&syntax, &analysis, settings, text)
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.severity))
        .collect()
}

/// Returns true if a code was reported.
///
/// # Arguments
/// * `text` - top-level netlist text
/// * `code` - diagnostic code
fn has(text: &str, code: &str) -> bool {
    run(text).iter().any(|(found, _)| *found == code)
}

/// Returns true if no diagnostic besides the listed codes was reported.
///
/// # Arguments
/// * `text` - top-level netlist text
/// * `allowed` - codes that may appear
fn only(text: &str, allowed: &[&str]) -> bool {
    run(text).iter().all(|(code, _)| allowed.contains(code))
}

const BASE: &str = "t\nV1 a 0 1\nR1 a 0 1k\n";

#[test]
fn clean_netlist_has_no_diagnostics() {
    assert!(run("t\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\n.tran 1n 10n\n.end\n").is_empty());
}

#[test]
fn block_structure_rules() {
    assert!(has(
        &format!("{BASE}.subckt s a b\nR2 a b 1\n"),
        "unclosed-block"
    ));
    assert!(has(&format!("{BASE}.if (1)\nR2 a 0 1\n"), "unclosed-block"));
    assert!(has(&format!("{BASE}.ends\n"), "unmatched-end"));
    assert!(has(&format!("{BASE}.else\n"), "unmatched-end"));
    assert!(has(
        &format!("{BASE}.subckt s a b\nR2 a b 1\nR3 a b 1\n.ends t\n"),
        "ends-name-mismatch"
    ));
    assert!(only(
        &format!("{BASE}.subckt s a b\nR2 a b 1\nR3 a b 1\n.ends S\n"),
        &[]
    ));
}

#[test]
fn unclosed_message_names_the_block() {
    let text = format!("{BASE}.subckt inv a b\nR2 a b 1\n");
    let syntax = parse_document(&text, true);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            ..Default::default()
        },
    );
    let diagnostics = compute(&syntax, &analysis, &DiagnosticSettings::default(), &text);
    let message = diagnostics
        .iter()
        .find(|d| d.code == "unclosed-block")
        .map(|d| d.message.as_str());
    assert_eq!(message, Some("`.subckt inv` is never closed with `.ends`"));
}

#[test]
fn bad_port_rule() {
    assert!(has(
        &format!("{BASE}.subckt s a 0\nR2 a 0 1\nR3 a 0 1\n.ends\n"),
        "bad-port"
    ));
    assert!(has(
        &format!("{BASE}.subckt s a a\nR2 a 0 1\nR3 a 0 1\n.ends\n"),
        "bad-port"
    ));
}

#[test]
fn duplicate_definition_rules() {
    let subckts = ".subckt s a b\nR2 a b 1\nR3 a b 1\n.ends\n";
    assert!(has(
        &format!("{BASE}{subckts}{subckts}"),
        "duplicate-subckt"
    ));
    assert!(!has(
        &format!("{BASE}{subckts}.alter\n{subckts}"),
        "duplicate-subckt"
    ));
    assert!(has(
        &format!("{BASE}.model m d\n.model m d\n"),
        "duplicate-model"
    ));
    assert!(!has(
        &format!("{BASE}.lib a\n.model m d\n.endl\n.lib b\n.model m d\n.endl\n"),
        "duplicate-model"
    ));
    assert!(has(
        &format!("{BASE}.param p=1\n.param p=2\n"),
        "duplicate-param"
    ));
    assert!(!has(
        &format!("{BASE}.param c=1\n.if (c==1)\n.param p=1\n.else\n.param p=2\n.endif\n"),
        "duplicate-param"
    ));
    assert!(has(&format!("{BASE}R1 a 0 2k\n"), "duplicate-element"));
    assert!(!has(
        &format!("{BASE}.alter\nR1 a 0 2k\n"),
        "duplicate-element"
    ));
}

#[test]
fn missing_end_and_title_rules() {
    assert!(has("t\nV1 a 0 1\nR1 a 0 1k\n.op\n", "missing-end"));
    assert!(!has("t\nV1 a 0 1\nR1 a 0 1k\n.op\n.end\n", "missing-end"));
    assert!(!has(BASE, "missing-end"));
    assert!(has(
        ".tran 1n 1u\nV1 a 0 1\nR1 a 0 1k\n.end\n",
        "title-is-statement"
    ));
    assert!(!has(
        "My .tran title\nV1 a 0 1\nR1 a 0 1k\n.end\n",
        "title-is-statement"
    ));
}

#[test]
fn undefined_references_are_errors_or_warnings() {
    assert!(has(&format!("{BASE}R2 a 0 'nope'\n"), "undefined-param"));
    assert!(has(&format!("{BASE}R2 a 0 nope\n"), "undefined-param"));
    assert!(has(
        &format!("{BASE}R2 a 0 'foo(1)'\n"),
        "undefined-function"
    ));
    assert!(has(&format!("{BASE}X1 a 0 nosub\n"), "undefined-subckt"));
    assert!(has(&format!("{BASE}M1 a a 0 0 nomod\n"), "undefined-model"));
    assert!(has(&format!("{BASE}F1 a 0 Vnone 2\n"), "undefined-source"));
    assert!(has(
        &format!("{BASE}.noise v(a) vnone\n"),
        "undefined-source"
    ));
    assert!(has(
        &format!("{BASE}.dc vnone 0 1 0.1\n"),
        "undefined-source"
    ));
    assert!(has(
        &format!("{BASE}L1 a 0 1n\nK1 L1 Lnone 0.5\n"),
        "undefined-inductor"
    ));
    assert!(!has(
        &format!(
            "{BASE}.param p=1\nR2 a 0 'p*2'\n.dc p 0 1 0.1\n.dc temp 0 50 10\n.dc V1 0 1 .1\n"
        ),
        "undefined-source"
    ));
    assert!(!has(
        &format!("{BASE}.param p=1\nR2 a 0 'max(p, 1)'\n"),
        "undefined-function"
    ));
}

#[test]
fn incomplete_context_downgrades_to_warnings() {
    let text = format!("{BASE}X1 a 0 nosub\nM1 a a 0 0 nomod\nR2 a 0 'nope'\n");
    let found = run_with(&text, true, false, &DiagnosticSettings::default());
    let undefined: Vec<_> = found
        .iter()
        .filter(|(code, _)| code.starts_with("undefined-"))
        .collect();
    assert_eq!(undefined.len(), 3);
    assert!(
        undefined
            .iter()
            .all(|(_, severity)| *severity == Severity::Warning)
    );
    let complete = run(&text);
    assert!(
        complete
            .iter()
            .filter(|(code, _)| code.starts_with("undefined-"))
            .all(|(_, severity)| *severity == Severity::Error)
    );
}

#[test]
fn model_and_port_rules() {
    assert!(has(
        &format!("{BASE}.model dm d\nM1 a a 0 0 dm\n"),
        "model-type-mismatch"
    ));
    assert!(!has(
        &format!("{BASE}.model nm nmos\nM1 a a 0 0 nm\n"),
        "model-type-mismatch"
    ));
    assert!(!has(
        &format!("{BASE}.subckt big d g s b\nR2 d s 1\nR3 g b 1\n.ends\nM1 a a 0 0 big\n"),
        "model-type-mismatch"
    ));
    let subckt = ".subckt inv in out vdd gnd\nR2 in out 1\nR3 vdd gnd 1\n.ends\n";
    assert!(has(&format!("{BASE}{subckt}X1 a 0 0 inv\n"), "port-count"));
    assert!(!has(
        &format!("{BASE}{subckt}X1 a 0 0 0 inv\n"),
        "port-count"
    ));
    assert!(has(
        &format!("{BASE}.subckt w a b g=1\nR2 a b 1\nR3 a b g\n.ends\nX1 a 0 w zz=1\n"),
        "unknown-instance-param"
    ));
    assert!(!has(
        &format!("{BASE}.subckt w a b g=1\nR2 a b 1\nR3 a b g\n.ends\nX1 a 0 w g=2 m=2\n"),
        "unknown-instance-param"
    ));
}

#[test]
fn port_count_message_lists_ports() {
    let text = format!(
        "{BASE}.subckt inv in out vdd gnd\nR2 in out 1\nR3 vdd gnd 1\n.ends\nX1 a 0 0 inv\n"
    );
    let syntax = parse_document(&text, true);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            ..Default::default()
        },
    );
    let diagnostics = compute(&syntax, &analysis, &DiagnosticSettings::default(), &text);
    let message = diagnostics
        .iter()
        .find(|d| d.code == "port-count")
        .map(|d| d.message.as_str());
    assert_eq!(
        message,
        Some("`inv` expects 4 ports (in out vdd gnd), got 3")
    );
}

#[test]
fn element_rules() {
    assert!(has(&format!("{BASE}Y1 a 0 1\n"), "unknown-element"));
    assert!(has(&format!("{BASE}R2 a\n"), "missing-nodes"));
    assert!(has(&format!("{BASE}X1 sub\n"), "missing-nodes"));
    assert!(has(&format!("{BASE}M1 a b\n"), "missing-nodes"));
}

#[test]
fn function_arity_rules() {
    assert!(has(&format!("{BASE}R2 a 0 'max(1)'\n"), "function-arity"));
    assert!(has(
        &format!("{BASE}R2 a 0 'sqrt(1,2)'\n"),
        "function-arity"
    ));
    assert!(!has(
        &format!("{BASE}R2 a 0 'max(1,2)'\n"),
        "function-arity"
    ));
    assert!(has(
        &format!("{BASE}.param f(x,y)='x+y'\nR2 a 0 'f(1)'\n"),
        "function-arity"
    ));
    assert!(!has(
        &format!("{BASE}.param f(x,y)='x+y'\nR2 a 0 'f(1,2)'\n"),
        "function-arity"
    ));
}

#[test]
fn analysis_argument_rules() {
    assert!(has(&format!("{BASE}.tran 1n\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.tran 1n 10n uic\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.tran data=d\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.ac dec 10 1\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.ac 10 1 1k\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.ac dec 10 1 1meg\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.dc V1 0 1\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.dc V1 0 1 0.1\n"), "analysis-args"));
    assert!(!has(
        &format!("{BASE}.dc V1 0 1 0.1 R1 1k 2k 500\n"),
        "analysis-args"
    ));
    assert!(!has(
        &format!("{BASE}.dc V1 dec 10 1 100\n"),
        "analysis-args"
    ));
    assert!(!has(&format!("{BASE}.dc temp poi 1 25\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.dc V1 dec 10 1\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.noise v(a)\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.tf v(a)\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.tf v(a) V1\n"), "analysis-args"));
}

#[test]
fn parameter_rules() {
    assert!(has(&format!("{BASE}.param temper=27\n"), "reserved-param"));
    assert!(has(
        &format!("{BASE}.param neg=-p\n.param p=1\n"),
        "unquoted-expression"
    ));
    assert!(!has(
        &format!("{BASE}.param neg=-0.5\n"),
        "unquoted-expression"
    ));
    assert!(!has(
        &format!("{BASE}.param neg='-p'\n.param p=1\n"),
        "unquoted-expression"
    ));
    assert!(has(&format!("{BASE}.param unused=1\n"), "unused-param"));
    assert!(!has(
        &format!("{BASE}.param used=1\nR2 a 0 used\n"),
        "unused-param"
    ));
    let found = run_with(
        &format!("{BASE}.param unused=1\n"),
        false,
        true,
        &DiagnosticSettings::default(),
    );
    assert!(found.iter().all(|(code, _)| *code != "unused-param"));
    let found = run_with(
        &format!("{BASE}.param unused=1\n"),
        true,
        false,
        &DiagnosticSettings::default(),
    );
    assert!(found.iter().all(|(code, _)| *code != "unused-param"));
    let off = DiagnosticSettings {
        unused_params: false,
        ..Default::default()
    };
    assert!(run_with(&format!("{BASE}.param unused=1\n"), true, true, &off).is_empty());
}

#[test]
fn params_read_by_included_files_are_used() {
    let text = format!("{BASE}.param vth_shift=0.1 other=1\n");
    let syntax = parse_document(&text, true);
    let words = std::collections::HashSet::from(["vth_shift".to_string()]);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            words: vec![std::sync::Arc::new(words)],
            ..Default::default()
        },
    );
    let unused: Vec<String> = compute(&syntax, &analysis, &DiagnosticSettings::default(), &text)
        .into_iter()
        .filter(|d| d.code == "unused-param")
        .map(|d| d.message)
        .collect();
    assert_eq!(unused, vec!["Parameter `other` is never used".to_string()]);
}

#[test]
fn fft_keyword_values_are_not_params() {
    assert_eq!(
        run(&format!(
            "{BASE}.tran 1n 10n\n.fft v(a) window=hann format=norm np=1024\n.end\n"
        )),
        vec![]
    );
    assert!(has(
        &format!("{BASE}.tran 1n 10n\n.fft v(a) np=npts\n"),
        "undefined-param"
    ));
}

#[test]
fn unused_param_is_tagged_unnecessary() {
    let text = format!("{BASE}.param unused=1\n");
    let syntax = parse_document(&text, true);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            ..Default::default()
        },
    );
    let diagnostics = compute(&syntax, &analysis, &DiagnosticSettings::default(), &text);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "unused-param" && d.unnecessary)
    );
}

#[test]
fn node_rules() {
    assert!(has("t\nV1 a 0 1\nR1 a lonely 1k\n", "floating-node"));
    assert!(!has(
        "t\n.subckt s a b\nR1 a b 1\nR2 a b 1\n.ends\nV1 x 0 1\nR1 x 0 1\n",
        "floating-node"
    ));
    assert!(has("t\nV1 a b 1\nR1 a b 1k\n", "no-ground"));
    assert!(!has(BASE, "no-ground"));
    let off = DiagnosticSettings {
        floating_nodes: false,
        ..Default::default()
    };
    assert!(run_with("t\nV1 a 0 1\nR1 a lonely 1k\n", true, true, &off).is_empty());
}

#[test]
fn command_rules() {
    assert!(has(&format!("{BASE}.foo\n"), "unknown-command"));
    assert!(has(&format!("{BASE}.func f(x) x*2\n"), "foreign-command"));
    assert!(has(
        &format!("{BASE}.model s1 sw vt=1\n"),
        "foreign-command"
    ));
    assert!(has(&format!("{BASE}.width out=80\n"), "obsolete"));
    assert!(has(
        &format!("{BASE}.option nonsense=1\n"),
        "unknown-option"
    ));
    assert!(!has(
        &format!("{BASE}.option post probe\n"),
        "unknown-option"
    ));
    let off = DiagnosticSettings {
        unknown_options: false,
        ..Default::default()
    };
    assert!(
        !run_with(&format!("{BASE}.option nonsense=1\n"), true, true, &off)
            .iter()
            .any(|(code, _)| *code == "unknown-option")
    );
}

#[test]
fn number_rules() {
    assert!(has(&format!("{BASE}R2 a 0 1e-6u\n"), "bad-number"));
    assert!(has(&format!("{BASE}R2 a 0 'w*1e-6u'\n"), "bad-number"));
    assert!(!has(&format!("{BASE}R2 a 0 1e-6\n"), "bad-number"));
    assert!(has(&format!("{BASE}R2 a 0 10M\n"), "meg-suffix"));
    assert!(has(&format!("{BASE}.ac dec 10 1 10M\n"), "meg-suffix"));
    assert!(!has(&format!("{BASE}R2 a 0 10MEG\n"), "meg-suffix"));
    assert!(!has(&format!("{BASE}R2 a 0 0.5m\n"), "meg-suffix"));
    let off = DiagnosticSettings {
        meg_suffix: false,
        ..Default::default()
    };
    assert!(
        !run_with(&format!("{BASE}R2 a 0 10M\n"), true, true, &off)
            .iter()
            .any(|(code, _)| *code == "meg-suffix")
    );
}

#[test]
fn meg_message_suggests_meg() {
    let text = format!("{BASE}R2 a 0 10M\n");
    let syntax = parse_document(&text, true);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            ..Default::default()
        },
    );
    let diagnostics = compute(&syntax, &analysis, &DiagnosticSettings::default(), &text);
    let message = diagnostics
        .iter()
        .find(|d| d.code == "meg-suffix")
        .map(|d| d.message.as_str());
    assert_eq!(
        message,
        Some("`10M` is 10 milli (1e-2); use `10MEG` for 10e6")
    );
}

#[test]
fn skipped_statements_are_never_reported() {
    assert!(only(
        &format!("{BASE}.variation\nY1 a b c\n.foo\nR9\n.end_variation\n"),
        &[]
    ));
}

#[test]
fn reserved_measure_names_are_errors() {
    assert!(has(
        &format!("{BASE}.meas tran tran avg v(a)\n"),
        "reserved-measure-name"
    ));
    assert!(!has(
        &format!("{BASE}.meas tran delay avg v(a)\n"),
        "reserved-measure-name"
    ));
}

#[test]
fn floating_nodes_are_not_reported_where_includes_may_connect_them() {
    let pex = format!("{BASE}.subckt inv a b\n.include 'missing.pxi'\nM1 a b x x nch\n.ends\n");
    assert!(!has(&pex, "floating-node"));
    let plain = format!("{BASE}.subckt inv a b\nR2 a lone 1k\nR3 a b 1k\n.ends\n");
    assert!(has(&plain, "floating-node"));
}

#[test]
fn parenthesised_nodes_are_nodes() {
    let text = "t\n.model nch nmos level=1\n.subckt inv a y\nM1 (y a 0 0) nch\n.ends\nV1 a 0 1\nX1 (a y) inv\nR1 (y 0) 1k\n.op\n.end\n";
    assert_eq!(run(text), vec![]);
}

#[test]
fn value_lists_and_wildcards_are_valid() {
    let text = format!(
        "{BASE}.ac dec 10 1 1meg\n.noise v(a) V1 10 listfreq=(1k 10k)\n.tran 1n 10n sweep monte=list(10 20:30)\n.probe tran v(*) i(*)\n.end\n"
    );
    assert_eq!(run(&text), vec![]);
}

#[test]
fn renamed_and_newly_listed_options_are_known() {
    let text = format!("{BASE}.option runlvl=3 absin=1 hb_maxiter=100 sim_la=pact\n");
    assert!(!has(&text, "unknown-option"));
}

#[test]
fn misspelled_options_are_warnings() {
    assert!(
        run(&format!("{BASE}.option run_lvl=3\n")).contains(&("unknown-option", Severity::Warning))
    );
    assert!(
        run(&format!(
            "{BASE}.mosra reltotaltime=1 bogus=1\n.tran 1n 10n\n"
        ))
        .contains(&("unknown-mosra-param", Severity::Warning))
    );
}

#[test]
fn mosra_parameters_are_checked() {
    let full = format!(
        "{BASE}.param tlife=6.3e8\n.mosra reltotaltime=tlife relstep=6.3e7 mosralife=delvth0 aginginst=\"x1.*\"\n.tran 1n 10n\n.end\n"
    );
    assert_eq!(run(&full), vec![]);
    assert!(has(&format!("{BASE}.mosra relstep=1\n"), "mosra-args"));
    assert!(has(
        &format!("{BASE}.mosra reltotaltime=1 bogus=1\n"),
        "unknown-mosra-param"
    ));
    assert!(has(
        &format!("{BASE}.mosra reltotaltime=nosuch\n"),
        "undefined-param"
    ));
}

#[test]
fn mosra_combination_rules() {
    let stress = format!("{BASE}.mosra reltotaltime=1e8\n");
    assert_eq!(
        run(&format!("{stress}.tran 1n 10n sweep V1 1 2 0.5\n.end\n")),
        vec![("mosra-sweep", Severity::Error)]
    );
    assert!(has(
        &format!("{stress}.tran 1n 10n sweep monte=10\n.end\n"),
        "mosra-sweep"
    ));
    assert!(only(
        &format!(
            "{BASE}.mosra reltotaltime=1e8 simmode=1\n.tran 1n 10n sweep monte=10\n.dc V1 0 1 0.1\n.end\n"
        ),
        &[]
    ));
    assert!(only(
        &format!(
            "{BASE}.param m=2\n.mosra reltotaltime=1e8 simmode=m\n.tran 1n 10n sweep monte=10\n.end\n"
        ),
        &[]
    ));
    assert_eq!(
        run(&format!("{stress}.tran 1n 10n\n.ac dec 10 1 1meg\n.end\n")),
        vec![("mosra-dc-ac", Severity::Warning)]
    );
    assert_eq!(
        run(&format!("{stress}.end\n")),
        vec![("mosra-without-tran", Severity::Warning)]
    );
    assert!(!has(
        &format!("{stress}.tran 1n 10n\n.end\n"),
        "mosra-without-tran"
    ));
    let alter = format!(
        "{stress}.tran 1n 10n\n.model nra mosra level=1\n.alter aged\n.model nra mosra level=1 tit0=1\n.appendmodel nra mosra nch nmos\n.end\n"
    );
    assert_eq!(
        run(&alter),
        vec![
            ("mosra-model-in-alter", Severity::Error),
            ("mosra-model-in-alter", Severity::Error)
        ]
    );
}

#[test]
fn output_variable_names_must_exist() {
    assert!(has(&format!("{BASE}.print v(nosuch)\n"), "undefined-node"));
    assert!(has(
        &format!("{BASE}.probe tran i(r9)\n"),
        "undefined-element"
    ));
    assert!(has(&format!("{BASE}.ic v(nosuch)=0\n"), "undefined-node"));
    assert!(has(
        &format!("{BASE}.meas tran m avg v(nosuch)\n"),
        "undefined-node"
    ));
    assert!(has(
        &format!("{BASE}.print par('v(nosuch)*2')\n"),
        "undefined-node"
    ));
    let valid =
        format!("{BASE}.global vdd\n.print v(a) v(a,0) i(r1) v1(r1) v(vdd) v(gnd) v(*) i(*)\n");
    assert!(only(&valid, &["floating-node"]));
}

#[test]
fn hierarchical_output_names_follow_local_subcircuits() {
    let text = "t\n.subckt s p\nR1 p q 1\nR2 q 0 1\n.ends\nV1 a 0 1\nX1 a s\n";
    let codes = |line: &str| {
        run(&format!("{text}{line}\n"))
            .into_iter()
            .map(|(code, _)| code)
            .filter(|code| code.starts_with("undefined-"))
            .collect::<Vec<_>>()
    };
    assert!(codes(".print v(x1.q) v(x1.p) i(x1.r1)").is_empty());
    assert_eq!(codes(".print v(x1.nosuch)"), vec!["undefined-node"]);
    assert_eq!(codes(".print i(x1.r9)"), vec!["undefined-element"]);
    assert_eq!(codes(".print v(x9.q)"), vec!["undefined-node"]);
    let external = "t\nV1 a 0 1\nX1 a b ext\n.print v(x1.anything)\n";
    assert!(!has(external, "undefined-node"));
}

#[test]
fn output_names_are_not_checked_in_include_files() {
    let found = run_with(
        "R1 a 0 1k\n.print v(nosuch)\n",
        false,
        true,
        &DiagnosticSettings::default(),
    );
    assert!(found.iter().all(|(code, _)| *code != "undefined-node"));
}

#[test]
fn enumerated_values_are_checked() {
    assert!(
        run(&format!("{BASE}.option method=gears\n"))
            .contains(&("invalid-value", Severity::Warning))
    );
    assert!(has(&format!("{BASE}.option runlvl=7\n"), "invalid-value"));
    assert!(has(
        &format!("{BASE}.mosra reltotaltime=1 relmode=3\n.tran 1n 10n\n"),
        "invalid-value"
    ));
    assert!(has(
        &format!("{BASE}.fft v(a) window=hat\n"),
        "invalid-value"
    ));
    assert!(!has(
        &format!("{BASE}.option method=GEAR post=2.0 runlvl=5 parhie=local converge=-1\n"),
        "invalid-value"
    ));
    assert!(!has(
        &format!("{BASE}.param lvl=5\n.option runlvl=lvl\n"),
        "invalid-value"
    ));
    assert!(!has(
        &format!("{BASE}.option reltol=1e-4\n"),
        "invalid-value"
    ));
}

#[test]
fn options_replaced_by_runlvl_are_hinted() {
    assert!(
        run(&format!("{BASE}.option trtol=7 itl3=4\n"))
            .contains(&("runlvl-ignored-option", Severity::Hint))
    );
    assert!(!has(
        &format!("{BASE}.option runlvl=0 trtol=7\n"),
        "runlvl-ignored-option"
    ));
    assert!(!has(
        &format!("{BASE}.option reltol=1e-4\n"),
        "runlvl-ignored-option"
    ));
}

#[test]
fn module_blocks_are_tracked() {
    assert!(has(
        &format!("{BASE}.modulevar mv\n.param q=1\n"),
        "unclosed-block"
    ));
    assert!(!has(
        &format!("{BASE}.modulevar mv\n.temp 25\n.endmodulevar mv\n"),
        "unclosed-block"
    ));
    assert!(!has(
        &format!("{BASE}.module m\n.endmodule\n.prune 'a.spf' 'b.sp'\n"),
        "unknown-command"
    ));
}

#[test]
fn number_plausibility_rules() {
    assert!(has(&format!("{BASE}.param x=1e-6u\n"), "bad-number"));
    assert!(!has(
        &format!("{BASE}.option mixed_num_format=1\n.param x=1e-6u\n"),
        "bad-number"
    ));
    assert!(has(
        &format!("{BASE}.option mixed_num_format=0\n.param x=1e-6u\n"),
        "bad-number"
    ));
    assert!(run(&format!("{BASE}C1 a 0 1\n")).contains(&("large-value", Severity::Hint)));
    assert!(has(&format!("{BASE}L1 a 0 l=0.5\n"), "large-value"));
    assert!(!has(
        &format!("{BASE}C1 a 0 1p\nL1 a 0 10n\n"),
        "large-value"
    ));
}

#[test]
fn element_connection_and_value_rules() {
    assert!(run(&format!("{BASE}R2 a a 1k\n")).contains(&("shorted-element", Severity::Warning)));
    assert!(has(&format!("{BASE}V2 0 gnd 1\n"), "shorted-element"));
    assert!(!has(&format!("{BASE}M1 a a 0 0 nch\n"), "shorted-element"));
    assert!(run(&format!("{BASE}R2 a 0\n")).contains(&("missing-value", Severity::Warning)));
    assert!(has(&format!("{BASE}C1 a 0\n"), "missing-value"));
    assert!(!has(
        &format!("{BASE}R2 a 0 r='1k'\nR3 a 0 rmod l=1u w=1u\nVm a 0\n"),
        "missing-value"
    ));
    assert!(run(&format!("{BASE}R2 a 0 1k m=0\n")).contains(&("bad-multiplier", Severity::Error)));
    assert!(!has(&format!("{BASE}R2 a 0 1k m=2\n"), "bad-multiplier"));
}

#[test]
fn data_and_temp_references_are_resolved() {
    let data = ".data d a\n1\n.enddata\n";
    assert!(
        run(&format!("{BASE}.tran 1n 10n sweep data=nosuch\n"))
            .contains(&("undefined-data", Severity::Error))
    );
    assert!(has(&format!("{BASE}.dc data=nosuch\n"), "undefined-data"));
    assert!(!has(
        &format!("{BASE}.param a=1\n{data}.tran 1n 10n sweep data=d\n"),
        "undefined-data"
    ));
    assert!(has(&format!("{BASE}.temp tnosuch\n"), "undefined-param"));
    assert!(!has(
        &format!("{BASE}.param t1=85\n.temp 25 t1\n"),
        "undefined-param"
    ));
}

#[test]
fn analysis_value_rules() {
    assert!(has(&format!("{BASE}.tran -0.1n 50n\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.tran 1n 0\n"), "analysis-args"));
    assert!(!has(&format!("{BASE}.tran 1n 1u 0 1p\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.ac lin 0 1 1g\n"), "analysis-args"));
    assert!(has(&format!("{BASE}.ac dec 10 10g 1g\n"), "analysis-args"));
    assert!(!has(
        &format!("{BASE}.ac dec 10 1 1g\n.ac poi 2 5k 1k\n"),
        "analysis-args"
    ));
    assert!(run(&format!("{BASE}.probe\n")).contains(&("empty-command", Severity::Warning)));
    assert!(has(&format!("{BASE}.ic\n"), "empty-command"));
    assert!(!has(&format!("{BASE}.print v(a)\n.op\n"), "empty-command"));
}

#[test]
fn output_signals_in_parameters_are_warned() {
    assert!(
        run(&format!("{BASE}.param p1='2*v(a)'\nR2 a 0 p1\n"))
            .contains(&("output-signal-param", Severity::Warning))
    );
    assert!(has(
        &format!("{BASE}.tran 1n 10n\n.meas tran m param='v(a)*2'\n"),
        "output-signal-param"
    ));
    assert!(!has(
        &format!("{BASE}.param f(x)='2*x' q='max(1,2)'\nR2 a 0 'f(v(a))+q'\n"),
        "output-signal-param"
    ));
}

#[test]
fn measure_keywords_cannot_be_parameters() {
    let delay = "trig v(a) val=0.5 rise=1 targ v(a) val=0.5 fall=last";
    assert!(
        run(&format!(
            "{BASE}.param td=1n\n.tran 1n 10n\n.meas tran d {delay} td=td\n"
        ))
        .contains(&("measure-keyword-param", Severity::Warning))
    );
    assert!(has(
        &format!("{BASE}.param td=1n\n.tran 1n 10n\n.meas tran m avg v(a) from 0 to td\n"),
        "measure-keyword-param"
    ));
    assert!(!has(
        &format!("{BASE}.param tdel=1n\n.tran 1n 10n\n.meas tran m avg v(a) from=0 to=tdel\n"),
        "measure-keyword-param"
    ));
    assert!(!has(
        &format!("{BASE}.param td=1n\n.tran 1n 10n\n.meas tran d {delay} td=trig\nR2 a 0 td\n"),
        "measure-keyword-param"
    ));
}

#[test]
fn source_function_rules() {
    let source = |text: &str| format!("t\nV1 a 0 {text}\nR1 a 0 1k\n");
    assert!(run(&source("PULSE(0)")).contains(&("source-args", Severity::Warning)));
    assert!(has(&source("PULSE(0 1 0 -1p 1p 5n 10n)"), "source-args"));
    assert!(has(&source("sin 0 1 -1k"), "source-args"));
    assert!(run(&source("PWL(0 0 1n 1 0.5n 0)")).contains(&("source-args", Severity::Error)));
    assert!(has(&source("pwl 0 0 5p 1 4.9p 0"), "source-args"));
    assert!(!has(&source("PULSE(0 1 0 1p 1p 5n 10n)"), "source-args"));
    assert!(!has(&source("PWL(0 0 1n 1 1n 0 2n 0) R=0"), "source-args"));
    assert!(!has(
        ".param vdd=1 td=1n\nV1 a 0 PULSE(0 vdd td)\nR1 a 0 1k\n",
        "source-args"
    ));
    assert!(!has(&source("dc 1 ac 1"), "source-args"));
}

#[test]
fn initial_conditions_and_branch_order() {
    assert!(
        run(&format!("{BASE}.ic v(a)=0\n.ic v(a)=1\n"))
            .contains(&("duplicate-ic", Severity::Warning))
    );
    assert!(!has(
        &format!("{BASE}.ic v(a)=0\n.nodeset v(a)=1\n"),
        "duplicate-ic"
    ));
    assert!(!has(
        &format!("{BASE}.if (1)\n.ic v(a)=0\n.else\n.ic v(a)=1\n.endif\n"),
        "duplicate-ic"
    ));
    assert!(
        run(&format!("{BASE}.if (1)\n.else\n.elseif (0)\n.endif\n"))
            .contains(&("misplaced-else", Severity::Error))
    );
    assert!(has(
        &format!("{BASE}.if (1)\n.else\n.else\n.endif\n"),
        "misplaced-else"
    ));
    assert!(!has(
        &format!("{BASE}.if (1)\n.elseif (0)\n.if (1)\n.else\n.endif\n.else\n.endif\n"),
        "misplaced-else"
    ));
}
