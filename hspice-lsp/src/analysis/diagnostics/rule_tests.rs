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
fn compute_without_text_skips_text_rules() {
    let text = ".tran 1n 1u\nV1 a 0 1\nR1 a 0 1k\n.end\n";
    let syntax = parse_document(text, true);
    let analysis = analyze(
        &syntax.netlist,
        ExternalContext {
            complete: true,
            ..Default::default()
        },
    );
    let settings = DiagnosticSettings::default();
    assert!(
        compute(&syntax, &analysis, &settings, text)
            .iter()
            .any(|d| d.code == "title-is-statement")
    );
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
