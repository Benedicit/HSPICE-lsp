//! Constant evaluation of parameter expressions for hover.

use crate::syntax::ast::{BinaryOp, Expr, ExprKind, Ident, UnaryOp};
use crate::syntax::number::parse_number;

/// Looks up the value of a parameter by lowercase name.
type Lookup<'a> = &'a dyn Fn(&str) -> Option<f64>;

/// Constant-folds an expression.
///
/// # Arguments
/// * `expr` - the expression to evaluate
/// * `lookup` - returns the value of a known parameter given its lowercase name
///
/// # Returns
/// The finite value, or `None` if the expression involves unknown names, time-dependent
/// quantities, output functions, invalid arity or a non-finite result.
pub fn evaluate(expr: &Expr, lookup: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    evaluate_kind(&expr.kind, lookup).filter(|value| value.is_finite())
}

/// Evaluates one expression node.
///
/// # Arguments
/// * `kind` - the node
/// * `lookup` - parameter lookup
fn evaluate_kind(kind: &ExprKind, lookup: Lookup) -> Option<f64> {
    match kind {
        ExprKind::Number(text) => parse_number(text).map(|parsed| parsed.value),
        ExprKind::Ident(ident) => evaluate_ident(ident, lookup),
        ExprKind::Unary { op, operand } => {
            evaluate(operand, lookup).map(|value| apply_unary(*op, value))
        }
        ExprKind::Binary { op, left, right } => {
            apply_binary(*op, evaluate(left, lookup)?, evaluate(right, lookup)?)
        }
        ExprKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            let branch = if evaluate(condition, lookup)? != 0.0 {
                then_branch
            } else {
                else_branch
            };
            evaluate(branch, lookup)
        }
        ExprKind::Call { name, args } => evaluate_call(name, args, lookup),
        ExprKind::Error => None,
    }
}

/// Resolves an identifier through the lookup, rejecting simulator variables.
///
/// # Arguments
/// * `ident` - the identifier
/// * `lookup` - parameter lookup
fn evaluate_ident(ident: &Ident, lookup: Lookup) -> Option<f64> {
    match ident.key.as_str() {
        "time" | "temper" | "hertz" => None,
        key => lookup(key),
    }
}

/// Converts a boolean to HSPICE's 1.0 / 0.0.
///
/// # Arguments
/// * `condition` - the truth value
fn truth(condition: bool) -> f64 {
    if condition { 1.0 } else { 0.0 }
}

/// Applies a unary operator.
///
/// # Arguments
/// * `op` - operator
/// * `value` - operand value
fn apply_unary(op: UnaryOp, value: f64) -> f64 {
    match op {
        UnaryOp::Neg => -value,
        UnaryOp::Plus => value,
        UnaryOp::Not => truth(value == 0.0),
    }
}

/// Applies a binary operator.
///
/// # Arguments
/// * `op` - operator
/// * `left` - left value
/// * `right` - right value
///
/// # Returns
/// The result, or `None` for division by zero.
fn apply_binary(op: BinaryOp, left: f64, right: f64) -> Option<f64> {
    Some(match op {
        BinaryOp::Add => left + right,
        BinaryOp::Sub => left - right,
        BinaryOp::Mul => left * right,
        BinaryOp::Div if right == 0.0 => return None,
        BinaryOp::Div => left / right,
        BinaryOp::Pow => power(left, right),
        BinaryOp::Eq => truth(left == right),
        BinaryOp::Ne => truth(left != right),
        BinaryOp::Lt => truth(left < right),
        BinaryOp::Le => truth(left <= right),
        BinaryOp::Gt => truth(left > right),
        BinaryOp::Ge => truth(left >= right),
        BinaryOp::And => truth(left != 0.0 && right != 0.0),
        BinaryOp::Or => truth(left != 0.0 || right != 0.0),
    })
}

/// Computes `x ** y`, using the integer part of `y` when `x` is negative.
///
/// # Arguments
/// * `base` - the base `x`
/// * `exponent` - the exponent `y`
fn power(base: f64, exponent: f64) -> f64 {
    if base < 0.0 {
        base.powf(exponent.trunc())
    } else {
        base.powf(exponent)
    }
}

/// Returns the sign of `x` as -1.0 or 1.0 (zero counts as positive).
///
/// # Arguments
/// * `value` - the number
fn sign_of(value: f64) -> f64 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

/// Evaluates a function call, handling `def` and the statistical functions specially.
///
/// # Arguments
/// * `name` - function name
/// * `args` - argument expressions
/// * `lookup` - parameter lookup
fn evaluate_call(name: &Ident, args: &[Expr], lookup: Lookup) -> Option<f64> {
    let key = name.key.as_str();
    match (key, args) {
        (
            "def",
            [
                Expr {
                    kind: ExprKind::Ident(target),
                    ..
                },
            ],
        ) => Some(truth(lookup(&target.key).is_some())),
        ("def", _) => None,
        ("unif" | "aunif", [nominal, _] | [nominal, _, _]) => evaluate(nominal, lookup),
        ("gauss" | "agauss", [nominal, _, _] | [nominal, _, _, _]) => evaluate(nominal, lookup),
        ("limit", [nominal, _]) => evaluate(nominal, lookup),
        _ => {
            let values = args
                .iter()
                .map(|arg| evaluate(arg, lookup))
                .collect::<Option<Vec<f64>>>()?;
            apply_builtin(key, &values)
        }
    }
}

/// Applies a built-in function to evaluated arguments.
///
/// # Arguments
/// * `key` - lowercase function name
/// * `values` - evaluated arguments
///
/// # Returns
/// The result, or `None` for unknown functions or wrong arity.
fn apply_builtin(key: &str, values: &[f64]) -> Option<f64> {
    match (key, values) {
        (_, [x]) => apply_unary_function(key, *x),
        ("pow", [x, y]) => Some(x.powf(y.trunc())),
        ("pwr", [x, y]) => Some(sign_of(*x) * x.abs().powf(*y)),
        ("sign", [x, y]) => Some(x.abs() * sign_of(*y)),
        ("min", [x, y]) => Some(x.min(*y)),
        ("max", [x, y]) => Some(x.max(*y)),
        _ => None,
    }
}

/// Applies a one-argument built-in function.
///
/// # Arguments
/// * `key` - lowercase function name
/// * `x` - argument value
fn apply_unary_function(key: &str, x: f64) -> Option<f64> {
    Some(match key {
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "asin" => x.asin(),
        "acos" => x.acos(),
        "atan" => x.atan(),
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "abs" => x.abs(),
        "exp" => x.exp(),
        "int" => x.trunc(),
        "nint" => x.round(),
        "sgn" if x == 0.0 => 0.0,
        "sgn" => sign_of(x),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "sqrt" => sign_of(x) * x.abs().sqrt(),
        "log" => sign_of(x) * x.abs().ln(),
        "log10" => sign_of(x) * x.abs().log10(),
        "db" => sign_of(x) * 20.0 * x.abs().log10(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ast::TextRange;

    fn expr(kind: ExprKind) -> Expr {
        Expr {
            kind,
            range: TextRange::default(),
        }
    }

    fn num(text: &str) -> Expr {
        expr(ExprKind::Number(text.to_string()))
    }

    fn ident(name: &str) -> Expr {
        expr(ExprKind::Ident(Ident::new(name, TextRange::default())))
    }

    fn bin(op: BinaryOp, left: Expr, right: Expr) -> Expr {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn un(op: UnaryOp, operand: Expr) -> Expr {
        expr(ExprKind::Unary {
            op,
            operand: Box::new(operand),
        })
    }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        expr(ExprKind::Call {
            name: Ident::new(name, TextRange::default()),
            args,
        })
    }

    fn lookup(name: &str) -> Option<f64> {
        match name {
            "wn" => Some(200e-9),
            "supply" => Some(1.0),
            "neg" => Some(-4.0),
            _ => None,
        }
    }

    fn eval(expression: &Expr) -> Option<f64> {
        evaluate(expression, &lookup)
    }

    fn close(actual: Option<f64>, expected: f64) -> bool {
        actual.is_some_and(|value| (value - expected).abs() <= 1e-12 * expected.abs().max(1.0))
    }

    #[test]
    fn numbers_with_scale() {
        assert!(close(eval(&num("2k")), 2000.0));
        assert!(close(eval(&num("10pF")), 10e-12));
        assert_eq!(eval(&num("1.2.3")), None);
    }

    #[test]
    fn identifiers() {
        assert!(close(eval(&ident("WN")), 200e-9));
        assert_eq!(eval(&ident("unknown")), None);
        for special in ["time", "temper", "hertz", "TIME"] {
            assert_eq!(eval(&ident(special)), None);
        }
    }

    #[test]
    fn arithmetic() {
        let sum = bin(BinaryOp::Add, num("1"), num("2"));
        assert!(close(eval(&sum), 3.0));
        let product = bin(BinaryOp::Mul, ident("wn"), num("2"));
        assert!(close(eval(&product), 400e-9));
        assert!(close(eval(&bin(BinaryOp::Sub, num("1"), num("3"))), -2.0));
        assert!(close(eval(&bin(BinaryOp::Div, num("1"), num("4"))), 0.25));
    }

    #[test]
    fn division_by_zero_and_non_finite() {
        assert_eq!(eval(&bin(BinaryOp::Div, num("1"), num("0"))), None);
        assert_eq!(eval(&call("exp", vec![num("1000")])), None);
        assert_eq!(eval(&call("log", vec![num("0")])), None);
    }

    #[test]
    fn unary_operators() {
        assert!(close(eval(&un(UnaryOp::Neg, num("5"))), -5.0));
        assert!(close(eval(&un(UnaryOp::Plus, num("5"))), 5.0));
        assert!(close(eval(&un(UnaryOp::Not, num("0"))), 1.0));
        assert!(close(eval(&un(UnaryOp::Not, num("3"))), 0.0));
    }

    #[test]
    fn power_operator() {
        assert!(close(
            eval(&bin(BinaryOp::Pow, num("2"), num("10"))),
            1024.0
        ));
        let negative_base = bin(BinaryOp::Pow, un(UnaryOp::Neg, num("2")), num("2.7"));
        assert!(close(eval(&negative_base), 4.0));
        assert!(close(eval(&bin(BinaryOp::Pow, num("4"), num("0.5"))), 2.0));
    }

    #[test]
    fn comparisons_and_logic() {
        let cases = [
            (BinaryOp::Lt, "1", "2", 1.0),
            (BinaryOp::Le, "2", "2", 1.0),
            (BinaryOp::Gt, "1", "2", 0.0),
            (BinaryOp::Ge, "1", "2", 0.0),
            (BinaryOp::Eq, "2", "2", 1.0),
            (BinaryOp::Ne, "2", "2", 0.0),
            (BinaryOp::And, "3", "0", 0.0),
            (BinaryOp::And, "3", "-1", 1.0),
            (BinaryOp::Or, "0", "5", 1.0),
            (BinaryOp::Or, "0", "0", 0.0),
        ];
        for (op, left, right, expected) in cases {
            assert!(
                close(eval(&bin(op, num(left), num(right))), expected),
                "{op:?}"
            );
        }
    }

    #[test]
    fn ternary_picks_branch() {
        let make = |condition: &str| {
            expr(ExprKind::Ternary {
                condition: Box::new(num(condition)),
                then_branch: Box::new(num("10")),
                else_branch: Box::new(num("20")),
            })
        };
        assert!(close(eval(&make("1")), 10.0));
        assert!(close(eval(&make("0")), 20.0));
    }

    #[test]
    fn ternary_ignores_unknown_untaken_branch() {
        let expression = expr(ExprKind::Ternary {
            condition: Box::new(num("1")),
            then_branch: Box::new(num("10")),
            else_branch: Box::new(ident("unknown")),
        });
        assert!(close(eval(&expression), 10.0));
    }

    #[test]
    fn trigonometry_and_exp() {
        assert!(close(eval(&call("sin", vec![num("0")])), 0.0));
        assert!(close(eval(&call("COS", vec![num("0")])), 1.0));
        assert!(close(
            eval(&call("atan", vec![num("1")])),
            std::f64::consts::FRAC_PI_4
        ));
        assert!(close(
            eval(&call("exp", vec![num("1")])),
            std::f64::consts::E
        ));
        assert!(close(eval(&call("tanh", vec![num("0")])), 0.0));
    }

    #[test]
    fn rounding_functions() {
        assert!(close(
            eval(&call("int", vec![un(UnaryOp::Neg, num("2.7"))])),
            -2.0
        ));
        assert!(close(eval(&call("nint", vec![num("2.5")])), 3.0));
        assert!(close(eval(&call("nint", vec![num("-2.5")])), -3.0));
        assert!(close(eval(&call("floor", vec![num("-2.1")])), -3.0));
        assert!(close(eval(&call("ceil", vec![num("2.1")])), 3.0));
        assert!(close(eval(&call("sgn", vec![num("-7")])), -1.0));
        assert!(close(eval(&call("sgn", vec![num("0")])), 0.0));
        assert!(close(eval(&call("abs", vec![num("-7")])), 7.0));
    }

    #[test]
    fn sign_preserving_functions() {
        assert!(close(eval(&call("sqrt", vec![num("16")])), 4.0));
        assert!(close(eval(&call("sqrt", vec![ident("neg")])), -2.0));
        assert!(close(eval(&call("log10", vec![num("1000")])), 3.0));
        assert!(close(
            eval(&call("log10", vec![un(UnaryOp::Neg, num("100"))])),
            -2.0
        ));
        assert!(close(eval(&call("log", vec![num("1")])), 0.0));
        assert!(close(eval(&call("db", vec![num("10")])), 20.0));
        assert!(close(
            eval(&call("db", vec![un(UnaryOp::Neg, num("10"))])),
            -20.0
        ));
    }

    #[test]
    fn two_argument_functions() {
        assert!(close(eval(&call("pow", vec![num("2"), num("3.9")])), 8.0));
        assert!(close(
            eval(&call("pow", vec![ident("neg"), num("2.9")])),
            16.0
        ));
        assert!(close(
            eval(&call("pwr", vec![ident("neg"), num("2")])),
            -16.0
        ));
        assert!(close(eval(&call("pwr", vec![num("3"), num("2")])), 9.0));
        assert!(close(eval(&call("sign", vec![num("5"), num("-1")])), -5.0));
        assert!(close(eval(&call("sign", vec![num("-5"), num("2")])), 5.0));
        assert!(close(eval(&call("min", vec![num("1"), num("2")])), 1.0));
        assert!(close(eval(&call("MAX", vec![num("1"), num("2")])), 2.0));
    }

    #[test]
    fn wrong_arity_is_none() {
        assert_eq!(eval(&call("max", vec![num("1")])), None);
        assert_eq!(eval(&call("sin", vec![num("1"), num("2")])), None);
        assert_eq!(eval(&call("sin", vec![])), None);
        assert_eq!(eval(&call("unif", vec![num("1")])), None);
        assert_eq!(eval(&call("limit", vec![num("1")])), None);
        assert_eq!(eval(&call("def", vec![])), None);
    }

    #[test]
    fn def_checks_lookup() {
        assert!(close(eval(&call("def", vec![ident("wn")])), 1.0));
        assert!(close(eval(&call("DEF", vec![ident("nope")])), 0.0));
        assert_eq!(eval(&call("def", vec![num("1")])), None);
    }

    #[test]
    fn statistical_functions_use_nominal() {
        assert!(close(eval(&call("unif", vec![num("5"), num("0.1")])), 5.0));
        assert!(close(
            eval(&call("aunif", vec![num("5"), num("1"), num("2")])),
            5.0
        ));
        assert!(close(
            eval(&call("gauss", vec![ident("supply"), num("0.1"), num("3")])),
            1.0
        ));
        let agauss = call("agauss", vec![num("1u"), num("1n"), num("3"), num("2")]);
        assert!(close(eval(&agauss), 1e-6));
        assert!(close(eval(&call("limit", vec![num("2"), num("0.5")])), 2.0));
    }

    #[test]
    fn unsupported_functions_are_none() {
        for name in [
            "v", "i", "p", "lv9", "lx7", "par", "val", "valp", "str", "myfunc",
        ] {
            assert_eq!(eval(&call(name, vec![num("1")])), None, "{name}");
        }
    }

    #[test]
    fn unknown_operand_and_error_nodes() {
        assert_eq!(eval(&bin(BinaryOp::Add, num("1"), ident("missing"))), None);
        assert_eq!(eval(&expr(ExprKind::Error)), None);
        assert_eq!(
            eval(&bin(BinaryOp::Add, num("1"), expr(ExprKind::Error))),
            None
        );
    }

    #[test]
    fn nested_expression_from_netlist() {
        let expression = bin(
            BinaryOp::Div,
            ident("supply"),
            bin(BinaryOp::Add, num("1"), num("1")),
        );
        assert!(close(eval(&expression), 0.5));
    }
}
