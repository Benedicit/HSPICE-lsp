//! Built-in expression functions and special variables.

use super::{Named, find_named};

/// Static description of a built-in expression function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionSpec {
    /// Lowercase name.
    pub name: &'static str,
    /// Alternative spellings.
    pub aliases: &'static [&'static str],
    /// Minimum number of arguments.
    pub min_args: usize,
    /// Maximum number of arguments.
    pub max_args: usize,
    /// Signature for hover and signature help, e.g. `pow(x, y)`.
    pub signature: &'static str,
    /// Parameter names in order, used by signature help.
    pub parameters: &'static [&'static str],
    /// One-line description.
    pub description: &'static str,
}

impl Named for FunctionSpec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }
}

/// A predefined variable usable in expressions, such as `time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariableSpec {
    /// Lowercase name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// False if HSPICE forbids redefining it with `.param`.
    pub redefinable: bool,
}

impl Named for VariableSpec {
    fn name(&self) -> &'static str {
        self.name
    }
}

/// Built-in math, statistical and element-access functions.
pub static FUNCTIONS: &[FunctionSpec] = &[
    FunctionSpec {
        name: "sin",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "sin(x)",
        parameters: &["x"],
        description: "Sine (radians)",
    },
    FunctionSpec {
        name: "cos",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "cos(x)",
        parameters: &["x"],
        description: "Cosine (radians)",
    },
    FunctionSpec {
        name: "tan",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "tan(x)",
        parameters: &["x"],
        description: "Tangent (radians)",
    },
    FunctionSpec {
        name: "asin",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "asin(x)",
        parameters: &["x"],
        description: "Arc sine",
    },
    FunctionSpec {
        name: "acos",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "acos(x)",
        parameters: &["x"],
        description: "Arc cosine",
    },
    FunctionSpec {
        name: "atan",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "atan(x)",
        parameters: &["x"],
        description: "Arc tangent",
    },
    FunctionSpec {
        name: "sinh",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "sinh(x)",
        parameters: &["x"],
        description: "Hyperbolic sine",
    },
    FunctionSpec {
        name: "cosh",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "cosh(x)",
        parameters: &["x"],
        description: "Hyperbolic cosine",
    },
    FunctionSpec {
        name: "tanh",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "tanh(x)",
        parameters: &["x"],
        description: "Hyperbolic tangent",
    },
    FunctionSpec {
        name: "abs",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "abs(x)",
        parameters: &["x"],
        description: "Absolute value",
    },
    FunctionSpec {
        name: "sqrt",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "sqrt(x)",
        parameters: &["x"],
        description: "Square root; keeps the sign of the argument",
    },
    FunctionSpec {
        name: "log",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "log(x)",
        parameters: &["x"],
        description: "Natural logarithm; keeps the sign of the argument",
    },
    FunctionSpec {
        name: "log10",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "log10(x)",
        parameters: &["x"],
        description: "Base-10 logarithm; keeps the sign of the argument",
    },
    FunctionSpec {
        name: "exp",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "exp(x)",
        parameters: &["x"],
        description: "Exponential e^x",
    },
    FunctionSpec {
        name: "db",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "db(x)",
        parameters: &["x"],
        description: "20*log10(|x|); keeps the sign of the argument",
    },
    FunctionSpec {
        name: "int",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "int(x)",
        parameters: &["x"],
        description: "Integer part (truncation)",
    },
    FunctionSpec {
        name: "nint",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "nint(x)",
        parameters: &["x"],
        description: "Nearest integer",
    },
    FunctionSpec {
        name: "sgn",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "sgn(x)",
        parameters: &["x"],
        description: "Sign of x (-1, 0, 1)",
    },
    FunctionSpec {
        name: "def",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "def(x)",
        parameters: &["x"],
        description: "1 if parameter x is defined, else 0",
    },
    FunctionSpec {
        name: "floor",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "floor(x)",
        parameters: &["x"],
        description: "Largest integer not above x (unverified, accepted)",
    },
    FunctionSpec {
        name: "ceil",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "ceil(x)",
        parameters: &["x"],
        description: "Smallest integer not below x (unverified, accepted)",
    },
    FunctionSpec {
        name: "pow",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "pow(x, y)",
        parameters: &["x", "y"],
        description: "x raised to the integer part of y",
    },
    FunctionSpec {
        name: "pwr",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "pwr(x, y)",
        parameters: &["x", "y"],
        description: "sign(x)*|x|^y",
    },
    FunctionSpec {
        name: "sign",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "sign(x, y)",
        parameters: &["x", "y"],
        description: "|x| with the sign of y",
    },
    FunctionSpec {
        name: "min",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "min(x, y)",
        parameters: &["x", "y"],
        description: "Smaller of x and y",
    },
    FunctionSpec {
        name: "max",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "max(x, y)",
        parameters: &["x", "y"],
        description: "Larger of x and y",
    },
    FunctionSpec {
        name: "unif",
        aliases: &[],
        min_args: 2,
        max_args: 3,
        signature: "unif(nominal, rel_variation[, multiplier])",
        parameters: &["nominal", "rel_variation", "multiplier"],
        description: "Uniform distribution with relative variation around nominal",
    },
    FunctionSpec {
        name: "aunif",
        aliases: &[],
        min_args: 2,
        max_args: 3,
        signature: "aunif(nominal, abs_variation[, multiplier])",
        parameters: &["nominal", "abs_variation", "multiplier"],
        description: "Uniform distribution with absolute variation around nominal",
    },
    FunctionSpec {
        name: "gauss",
        aliases: &[],
        min_args: 3,
        max_args: 4,
        signature: "gauss(nominal, rel_variation, num_sigma[, multiplier])",
        parameters: &["nominal", "rel_variation", "num_sigma", "multiplier"],
        description: "Gaussian distribution with relative variation",
    },
    FunctionSpec {
        name: "agauss",
        aliases: &[],
        min_args: 3,
        max_args: 4,
        signature: "agauss(nominal, abs_variation, num_sigma[, multiplier])",
        parameters: &["nominal", "abs_variation", "num_sigma", "multiplier"],
        description: "Gaussian distribution with absolute variation",
    },
    FunctionSpec {
        name: "limit",
        aliases: &[],
        min_args: 2,
        max_args: 2,
        signature: "limit(nominal, abs_variation)",
        parameters: &["nominal", "abs_variation"],
        description: "Nominal plus or minus the absolute variation, chosen randomly",
    },
    FunctionSpec {
        name: "val",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "val(element)",
        parameters: &["element"],
        description: "Value of an element or element parameter, e.g. val(r1)",
    },
    FunctionSpec {
        name: "valm",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "valm(element.modelparam)",
        parameters: &["element.modelparam"],
        description: "Value of an element's model parameter",
    },
    FunctionSpec {
        name: "valp",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "valp(param)",
        parameters: &["param"],
        description: "Value of a parameter, including subcircuit parameter paths",
    },
    FunctionSpec {
        name: "str",
        aliases: &[],
        min_args: 1,
        max_args: 1,
        signature: "str(string)",
        parameters: &["string"],
        description: "String parameter value, e.g. str('nch')",
    },
];

/// Predefined variables (`time`, `temper`, `hertz`, and leniently accepted names such as `pi`).
pub static SPECIAL_VARIABLES: &[VariableSpec] = &[
    VariableSpec {
        name: "time",
        description: "Current simulation time in transient analysis",
        redefinable: false,
    },
    VariableSpec {
        name: "temper",
        description: "Current circuit temperature in degrees Celsius",
        redefinable: false,
    },
    VariableSpec {
        name: "hertz",
        description: "Current frequency in AC analysis",
        redefinable: false,
    },
    VariableSpec {
        name: "pi",
        description: "Not built into HSPICE (manuals define it via .PARAM); accepted leniently and may be redefined",
        redefinable: true,
    },
];

/// Looks up a built-in function by name (case-insensitive).
///
/// # Arguments
/// * `name` - function name as written
pub fn find_function(name: &str) -> Option<&'static FunctionSpec> {
    find_named(FUNCTIONS, name)
}

/// Looks up a predefined variable by name (case-insensitive).
///
/// # Arguments
/// * `name` - variable name as written
pub fn find_special_variable(name: &str) -> Option<&'static VariableSpec> {
    find_named(SPECIAL_VARIABLES, name)
}

/// Returns true for optimization-style calls such as `OPT1(init, lo, hi)`, whose names are user-chosen.
///
/// # Arguments
/// * `name` - function name as written
pub fn is_optimization_function(name: &str) -> bool {
    name.len() > 3
        && name
            .get(..3)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("opt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::assert_unique_lowercase;

    #[test]
    fn finds_functions_any_case() {
        assert_eq!(find_function("SQRT").map(|f| f.name), Some("sqrt"));
        assert_eq!(
            find_function("AGauss").map(|f| (f.min_args, f.max_args)),
            Some((3, 4))
        );
        assert!(find_function("bogus").is_none());
    }

    #[test]
    fn finds_special_variables() {
        assert_eq!(
            find_special_variable("TIME").map(|v| v.redefinable),
            Some(false)
        );
        assert_eq!(
            find_special_variable("Pi").map(|v| v.redefinable),
            Some(true)
        );
        assert!(find_special_variable("hertz").is_some_and(|v| !v.redefinable));
    }

    #[test]
    fn table_invariants() {
        assert_unique_lowercase(FUNCTIONS);
        for function in FUNCTIONS {
            assert!(function.min_args <= function.max_args);
            assert!(function.parameters.len() >= function.max_args);
            assert!(function.signature.starts_with(function.name));
            assert!(!function.description.is_empty());
        }
    }

    #[test]
    fn optimization_function_detection() {
        assert!(is_optimization_function("opt1"));
        assert!(is_optimization_function("OPTxyz"));
        assert!(!is_optimization_function("opt"));
        assert!(!is_optimization_function("op"));
        assert!(!is_optimization_function("sin"));
    }
}
