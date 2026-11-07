//! Assigns roles (nodes, model, subckt, controlling elements, values) to positional element arguments.

use crate::lang::elements::{ElementSpec, find_source_function};
use crate::syntax::ast::{Arg, AssignKey, Assignment, Call, Element, Ident, Value};

/// Role assignment of one element line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElementRoles {
    /// Terminal nodes in order.
    pub nodes: Vec<Ident>,
    /// Controlling node pairs of E/G sources (in order).
    pub control_nodes: Vec<Ident>,
    /// Model or subckt name.
    pub reference: Option<Ident>,
    /// Controlling voltage sources (F/H) or coupled inductors (K).
    pub controls: Vec<Ident>,
    /// Positional values that are neither nodes nor references (values, keywords, source functions).
    pub values: Vec<Value>,
}

/// Lookup callbacks used to break ties between nodes and model names.
pub struct NameKinds<'a> {
    /// Returns true if a lowercase name is a known model (or binned model).
    pub is_model: &'a dyn Fn(&str) -> bool,
    /// Returns true if a lowercase name is a known subcircuit.
    pub is_subckt: &'a dyn Fn(&str) -> bool,
}

/// Returns the positional (non-assignment) values of an element in order.
///
/// A group right after the name holds nodes: `M1 (d g s b) nch`.
///
/// # Arguments
/// * `element` - lowered element line
pub fn positional_values(element: &Element) -> Vec<&Value> {
    let leading: &[Arg] = match element.args.first() {
        Some(Arg::Group { args, .. }) => args,
        _ => &[],
    };
    leading
        .iter()
        .chain(&element.args)
        .filter_map(|arg| match arg {
            Arg::Value(value) => Some(value),
            _ => None,
        })
        .collect()
}

/// Returns the assignments of an element in order.
///
/// # Arguments
/// * `element` - lowered element line
pub fn assignments(element: &Element) -> impl Iterator<Item = &Assignment> {
    element.args.iter().filter_map(|arg| match arg {
        Arg::Assign(assignment) => Some(assignment),
        _ => None,
    })
}

/// Returns the identifier of a value usable as a node or name (words and plain numbers).
///
/// # Arguments
/// * `value` - positional value
fn name_like(value: &Value) -> Option<Ident> {
    match value {
        Value::Word(word) => Some(word.clone()),
        Value::Number(number)
            if number
                .text
                .chars()
                .all(|character| character.is_ascii_digit()) =>
        {
            Some(Ident::new(&number.text, number.range))
        }
        _ => None,
    }
}

/// Returns the word of a value, if it is a bare word.
///
/// # Arguments
/// * `value` - positional value
fn word(value: &Value) -> Option<&Ident> {
    match value {
        Value::Word(word) => Some(word),
        _ => None,
    }
}

/// Assigns roles to the positional arguments of an element.
///
/// # Arguments
/// * `element` - lowered element line
/// * `spec` - element type from the first letter, if known
/// * `names` - model/subckt lookups for tie-breaking
pub fn assign_roles(
    element: &Element,
    spec: Option<&ElementSpec>,
    names: &NameKinds,
) -> ElementRoles {
    let positionals = positional_values(element);
    let letter = spec.map_or('?', |spec| spec.letter);
    match letter {
        'X' => subckt_instance_roles(&positionals),
        'M' | 'Q' | 'J' => optional_fourth_node_roles(&positionals, names),
        'D' => fixed_nodes_roles(&positionals, 2, true),
        'R' | 'C' | 'L' => passive_roles(&positionals, names),
        'V' | 'I' | 'P' => fixed_nodes_roles(&positionals, 2, false),
        'E' | 'G' => voltage_controlled_roles(element, &positionals),
        'F' | 'H' => current_controlled_roles(&positionals),
        'K' => coupling_roles(&positionals),
        'T' => transmission_line_roles(&positionals),
        _ => variadic_roles(&positionals, names),
    }
}

/// Splits values into leading node names and the remaining values.
///
/// # Arguments
/// * `positionals` - positional values
/// * `count` - maximum number of nodes
fn take_nodes(positionals: &[&Value], count: usize) -> (Vec<Ident>, Vec<Value>) {
    let nodes: Vec<Ident> = positionals
        .iter()
        .take(count)
        .map_while(|value| name_like(value))
        .collect();
    let rest = positionals[nodes.len()..]
        .iter()
        .map(|value| (*value).clone())
        .collect();
    (nodes, rest)
}

/// X elements: the last positional word is the subckt name, everything before it are nodes.
///
/// # Arguments
/// * `positionals` - positional values
fn subckt_instance_roles(positionals: &[&Value]) -> ElementRoles {
    let Some(reference_index) = positionals.iter().rposition(|value| word(value).is_some()) else {
        let (nodes, values) = take_nodes(positionals, positionals.len());
        return ElementRoles {
            nodes,
            values,
            ..ElementRoles::default()
        };
    };
    let (nodes, _) = take_nodes(&positionals[..reference_index], reference_index);
    ElementRoles {
        nodes,
        reference: word(positionals[reference_index]).cloned(),
        values: positionals[reference_index + 1..]
            .iter()
            .map(|value| (*value).clone())
            .collect(),
        ..ElementRoles::default()
    }
}

/// M/Q/J elements: 3 nodes plus an optional 4th, then the model (`str(param)` allowed).
///
/// # Arguments
/// * `positionals` - positional values
/// * `names` - model lookups for tie-breaking
fn optional_fourth_node_roles(positionals: &[&Value], names: &NameKinds) -> ElementRoles {
    let is_reference = |index: usize| {
        positionals.get(index).is_some_and(|value| {
            matches!(value, Value::Word(_))
                || matches!(value, Value::Call(call) if call.name.key == "str")
        })
    };
    let is_known = |index: usize| {
        positionals
            .get(index)
            .and_then(|value| word(value))
            .is_some_and(|name| (names.is_model)(&name.key) || (names.is_subckt)(&name.key))
    };
    let model_index = if is_reference(4) && !(is_known(3) && !is_known(4)) {
        4
    } else {
        3
    };
    if positionals.len() <= model_index {
        let (nodes, values) = take_nodes(positionals, positionals.len().min(4));
        return ElementRoles {
            nodes,
            values,
            ..ElementRoles::default()
        };
    }
    let (nodes, _) = take_nodes(&positionals[..model_index], model_index);
    ElementRoles {
        nodes,
        reference: word(positionals[model_index]).cloned(),
        values: positionals[model_index..]
            .iter()
            .skip(usize::from(word(positionals[model_index]).is_some()))
            .map(|value| (*value).clone())
            .collect(),
        ..ElementRoles::default()
    }
}

/// Elements with a fixed node count followed by an optional or required model word.
///
/// # Arguments
/// * `positionals` - positional values
/// * `node_count` - number of nodes
/// * `has_model` - true if the value after the nodes is a model name
fn fixed_nodes_roles(positionals: &[&Value], node_count: usize, has_model: bool) -> ElementRoles {
    let (nodes, rest) = take_nodes(positionals, node_count);
    let reference = has_model
        .then(|| rest.first().and_then(word).cloned())
        .flatten();
    let values = rest
        .into_iter()
        .skip(usize::from(reference.is_some()))
        .collect();
    ElementRoles {
        nodes,
        reference,
        values,
        ..ElementRoles::default()
    }
}

/// R/C/L elements: an optional model when a value follows it or when the name is a known model.
///
/// # Arguments
/// * `positionals` - positional values
/// * `names` - model lookups
fn passive_roles(positionals: &[&Value], names: &NameKinds) -> ElementRoles {
    let (nodes, rest) = take_nodes(positionals, 2);
    let is_model = rest.first().and_then(word).is_some_and(|name| {
        (names.is_model)(&name.key)
            || (rest.len() > 1 && !rest[1..].iter().all(|value| word(value).is_some()))
    });
    let reference = is_model
        .then(|| rest.first().and_then(word).cloned())
        .flatten();
    let values = rest
        .into_iter()
        .skip(usize::from(reference.is_some()))
        .collect();
    ElementRoles {
        nodes,
        reference,
        values,
        ..ElementRoles::default()
    }
}

/// Returns the dimension of a `POLY(n)` call, if `value` is one.
///
/// # Arguments
/// * `value` - positional value
fn poly_dimension(value: &Value) -> Option<usize> {
    let Value::Call(Call { name, args, .. }) = value else {
        return None;
    };
    if name.key != "poly" {
        return None;
    }
    match args.first() {
        Some(Arg::Value(Value::Number(number))) => number.text.parse().ok(),
        _ => Some(1),
    }
}

/// Returns true if a value is a behavioural keyword word or call of E/G/F/H sources.
///
/// # Arguments
/// * `value` - positional value
fn is_source_keyword(value: &Value) -> bool {
    const KEYWORDS: &[&str] = &[
        "vcvs",
        "vccs",
        "cccs",
        "ccvs",
        "vcr",
        "vccap",
        "delay",
        "laplace",
        "pole",
        "freq",
        "foster",
        "ztrans",
        "opamp",
        "transformer",
        "and",
        "nand",
        "or",
        "nor",
        "pwl",
        "npwl",
        "ppwl",
        "noise",
    ];
    let key = match value {
        Value::Word(word) => &word.key,
        Value::Call(call) => &call.name.key,
        _ => return false,
    };
    KEYWORDS.contains(&key.as_str())
}

/// E/G elements: output nodes, then linear / POLY(n) / keyword control nodes; VOL=/CUR= forms have none.
///
/// # Arguments
/// * `element` - element line (assignments are inspected for `vol`/`cur`/`poly`)
/// * `positionals` - positional values
fn voltage_controlled_roles(element: &Element, positionals: &[&Value]) -> ElementRoles {
    let (nodes, rest) = take_nodes(positionals, 2);
    let behavioural = assignments(element).any(|assignment| {
        matches!(&assignment.key, AssignKey::Name(key) if matches!(key.key.as_str(), "vol" | "cur" | "value"))
    });
    let poly_assignment =
        assignments(element).find_map(|assignment| match (&assignment.key, &assignment.value) {
            (AssignKey::Name(key), Some(Value::Expr(expr))) if key.key == "poly" => {
                match &expr.kind {
                    crate::syntax::ast::ExprKind::Number(text) => text.parse::<usize>().ok(),
                    _ => None,
                }
            }
            _ => None,
        });
    if behavioural {
        return ElementRoles {
            nodes,
            values: rest,
            ..ElementRoles::default()
        };
    }
    let keyword_count = rest
        .iter()
        .take_while(|value| is_source_keyword(value) && poly_dimension(value).is_none())
        .count();
    let after_keywords = &rest[keyword_count..];
    let (dimension, skip) = match after_keywords.first().and_then(poly_dimension) {
        Some(dimension) => (dimension, 1),
        None => (poly_assignment.unwrap_or(1), 0),
    };
    let candidates: Vec<&Value> = after_keywords[skip..].iter().collect();
    let (control_nodes, _) = take_nodes(&candidates, 2 * dimension);
    let consumed = keyword_count + skip + control_nodes.len();
    ElementRoles {
        nodes,
        control_nodes,
        values: rest
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index < keyword_count + skip || *index >= consumed)
            .map(|(_, value)| value)
            .collect(),
        ..ElementRoles::default()
    }
}

/// F/H elements: output nodes, then one (or POLY(n)) controlling voltage-source names.
///
/// # Arguments
/// * `positionals` - positional values
fn current_controlled_roles(positionals: &[&Value]) -> ElementRoles {
    let (nodes, rest) = take_nodes(positionals, 2);
    let keyword_count = rest
        .iter()
        .take_while(|value| is_source_keyword(value) && poly_dimension(value).is_none())
        .count();
    let after_keywords = &rest[keyword_count..];
    let (dimension, skip) = after_keywords
        .first()
        .and_then(poly_dimension)
        .map_or((1, 0), |dimension| (dimension, 1));
    let controls: Vec<Ident> = after_keywords[skip..]
        .iter()
        .take(dimension)
        .map_while(|value| word(value).cloned())
        .collect();
    let consumed = keyword_count + skip + controls.len();
    ElementRoles {
        nodes,
        controls,
        values: rest
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index < keyword_count + skip || *index >= consumed)
            .map(|(_, value)| value)
            .collect(),
        ..ElementRoles::default()
    }
}

/// K elements: inductor names, optionally followed by a core model name, then the coupling value.
///
/// # Arguments
/// * `positionals` - positional values
fn coupling_roles(positionals: &[&Value]) -> ElementRoles {
    let words: Vec<Ident> = positionals
        .iter()
        .map_while(|value| word(value).cloned())
        .collect();
    let (controls, reference): (Vec<Ident>, Option<Ident>) = match words.split_last() {
        Some((last, inductors)) if inductors.len() >= 2 && !last.key.starts_with('l') => {
            (inductors.to_vec(), Some(last.clone()))
        }
        _ => (words.clone(), None),
    };
    ElementRoles {
        controls,
        reference,
        values: positionals[words.len()..]
            .iter()
            .map(|value| (*value).clone())
            .collect(),
        ..ElementRoles::default()
    }
}

/// T elements: four nodes and an optional U model.
///
/// # Arguments
/// * `positionals` - positional values
fn transmission_line_roles(positionals: &[&Value]) -> ElementRoles {
    fixed_nodes_roles(positionals, 4, true)
}

/// Variadic elements (B S U W and unknown letters): leading name-like values are nodes unless known models.
///
/// # Arguments
/// * `positionals` - positional values
/// * `names` - model lookups
fn variadic_roles(positionals: &[&Value], names: &NameKinds) -> ElementRoles {
    let (mut nodes, values) = take_nodes(positionals, positionals.len());
    let reference = match nodes.last() {
        Some(last) if (names.is_model)(&last.key) => nodes.pop(),
        _ => None,
    };
    ElementRoles {
        nodes,
        reference,
        values,
        ..ElementRoles::default()
    }
}

/// Returns true if a word is a positional keyword of the element type or a source function name.
///
/// # Arguments
/// * `spec` - element type
/// * `key` - lowercase word
pub fn is_element_keyword(spec: Option<&ElementSpec>, key: &str) -> bool {
    spec.is_some_and(|spec| spec.keywords.contains(&key)) || find_source_function(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::elements::find_element;
    use crate::syntax::ast::StatementKind;
    use crate::syntax::{lower::lower, parse::parse_tree};

    fn roles_of(text: &str, models: &[&str]) -> ElementRoles {
        let tree = parse_tree(text).expect("parser");
        let netlist = lower(&tree, text);
        let Some(StatementKind::Element(element)) = netlist
            .statements
            .first()
            .map(|statement| statement.kind.clone())
        else {
            panic!("expected element");
        };
        let is_model = |key: &str| models.contains(&key);
        let is_subckt = |_: &str| false;
        let names = NameKinds {
            is_model: &is_model,
            is_subckt: &is_subckt,
        };
        assign_roles(&element, find_element(&element.name.text), &names)
    }

    fn keys(idents: &[Ident]) -> Vec<&str> {
        idents.iter().map(|ident| ident.key.as_str()).collect()
    }

    #[test]
    fn leading_group_holds_nodes() {
        let mosfet = roles_of("M1 (d g s b) nch W=1u\n", &[]);
        assert_eq!(keys(&mosfet.nodes), vec!["d", "g", "s", "b"]);
        assert_eq!(mosfet.reference.map(|name| name.key), Some("nch".into()));
        let instance = roles_of("X1 (a b) inv\n", &[]);
        assert_eq!(keys(&instance.nodes), vec!["a", "b"]);
        assert_eq!(instance.reference.map(|name| name.key), Some("inv".into()));
    }

    #[test]
    fn mosfet_with_and_without_bulk() {
        let four = roles_of("M1 d g s b nch W=1u\n", &[]);
        assert_eq!(keys(&four.nodes), vec!["d", "g", "s", "b"]);
        assert_eq!(four.reference.map(|name| name.key), Some("nch".into()));
        let three = roles_of("M1 d g s nch 1u 0.1u\n", &[]);
        assert_eq!(keys(&three.nodes), vec!["d", "g", "s"]);
        assert_eq!(three.reference.map(|name| name.key), Some("nch".into()));
        assert_eq!(three.values.len(), 2);
    }

    #[test]
    fn subckt_instance_uses_last_word() {
        let roles = roles_of("X1 in out vdd 0 inv wn=1u\n", &[]);
        assert_eq!(keys(&roles.nodes), vec!["in", "out", "vdd", "0"]);
        assert_eq!(roles.reference.map(|name| name.key), Some("inv".into()));
    }

    #[test]
    fn resistor_value_word_is_param_unless_model() {
        let value = roles_of("R1 a b rval\n", &[]);
        assert!(value.reference.is_none());
        assert_eq!(value.values.len(), 1);
        let model = roles_of("R1 a b rmod\n", &["rmod"]);
        assert_eq!(model.reference.map(|name| name.key), Some("rmod".into()));
        let model_then_value = roles_of("R1 a b rpoly 1k\n", &[]);
        assert_eq!(
            model_then_value.reference.map(|name| name.key),
            Some("rpoly".into())
        );
    }

    #[test]
    fn controlled_sources() {
        let linear = roles_of("E1 out 0 in 0 10\n", &[]);
        assert_eq!(keys(&linear.control_nodes), vec!["in", "0"]);
        let poly = roles_of("E2 o 0 POLY(2) a 0 b 0 0 1 1\n", &[]);
        assert_eq!(keys(&poly.control_nodes), vec!["a", "0", "b", "0"]);
        assert_eq!(poly.values.len(), 4);
        let behavioural = roles_of("E3 o 0 VOL='v(a)*2'\n", &[]);
        assert!(behavioural.control_nodes.is_empty());
        let keyword = roles_of("G1 b 0 VCCS out 0 1m\n", &[]);
        assert_eq!(keys(&keyword.control_nodes), vec!["out", "0"]);
        let current = roles_of("F1 c 0 vsense 2\n", &[]);
        assert_eq!(keys(&current.controls), vec!["vsense"]);
    }

    #[test]
    fn coupling_and_diode() {
        let coupling = roles_of("K1 L1 L2 0.9\n", &[]);
        assert_eq!(keys(&coupling.controls), vec!["l1", "l2"]);
        let diode = roles_of("D1 a k dmod 2\n", &[]);
        assert_eq!(keys(&diode.nodes), vec!["a", "k"]);
        assert_eq!(diode.reference.map(|name| name.key), Some("dmod".into()));
    }
}
