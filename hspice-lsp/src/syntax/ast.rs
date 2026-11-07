//! Typed syntax tree produced by lowering the tree-sitter CST; all positions are byte offsets.

/// Half-open byte range `[start, end)` into the document text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct TextRange {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl TextRange {
    /// Creates a range from two byte offsets.
    ///
    /// # Arguments
    /// * `start` - first byte
    /// * `end` - one past the last byte
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns true if `offset` lies inside the range or directly at its end (cursor semantics).
    ///
    /// # Arguments
    /// * `offset` - byte offset of the cursor
    pub fn touches(&self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }

    /// Returns true if `other` lies completely inside this range.
    ///
    /// # Arguments
    /// * `other` - range to test
    pub fn contains_range(&self, other: TextRange) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Returns the smallest range covering both ranges.
    ///
    /// # Arguments
    /// * `other` - range to merge with
    pub fn cover(self, other: TextRange) -> TextRange {
        TextRange::new(self.start.min(other.start), self.end.max(other.end))
    }

    /// Returns the number of bytes in the range.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns true if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A name as written plus its case-folded lookup key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// Original spelling.
    pub text: String,
    /// Lowercase key used for case-insensitive lookup.
    pub key: String,
    /// Location of the name.
    pub range: TextRange,
}

impl Ident {
    /// Creates an identifier and derives its lowercase key.
    ///
    /// # Arguments
    /// * `text` - original spelling
    /// * `range` - location in the document
    pub fn new(text: &str, range: TextRange) -> Self {
        Self {
            text: text.to_string(),
            key: text.to_ascii_lowercase(),
            range,
        }
    }
}

/// A lowered document: a flat list of statements in source order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Netlist {
    /// Statements in source order (block structure is rebuilt by `analysis::blocks`).
    pub statements: Vec<Statement>,
}

/// One logical line (including `+` continuation lines).
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    /// What kind of statement this is.
    pub kind: StatementKind,
    /// Range of the whole logical line, excluding the trailing newline.
    pub range: TextRange,
}

/// The kinds of statements HSPICE netlists contain.
#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    /// A `*` comment line.
    Comment,
    /// An element (instance) line such as `M1 d g s b nch W=1u`.
    Element(Element),
    /// A dot command.
    Command(Command),
    /// A line tree-sitter could not parse into any statement.
    Error,
}

/// An element line: name plus raw arguments; roles are assigned in `analysis::roles`.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Element name; its first letter selects the element type.
    pub name: Ident,
    /// Arguments in source order.
    pub args: Vec<Arg>,
}

/// A dot command: the keyword as written plus its kind-specific payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Command keyword including the dot, e.g. `.TRAN` (key `.tran`).
    pub keyword: Ident,
    /// Command payload.
    pub kind: CommandKind,
}

/// Kind-specific payload of a dot command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    /// `.include` / `.inc` / `.incl`.
    Include {
        /// File path, if present.
        path: Option<PathArg>,
    },
    /// `.lib 'file' section` (call form, `path` is set) or `.lib section` (definition form).
    Lib {
        /// File path of the call form.
        path: Option<PathArg>,
        /// Library section (entry) name.
        section: Option<Ident>,
    },
    /// `.param` / `.parameter` / `.parameters`.
    Param {
        /// Definitions in source order.
        definitions: Vec<ParamDef>,
    },
    /// `.model name type args…`.
    Model {
        /// Model name, e.g. `nch.1`.
        name: Option<Ident>,
        /// Model type, e.g. `nmos`.
        model_type: Option<Ident>,
        /// Model parameters (assignments, possibly inside a parenthesized group).
        args: Vec<Arg>,
    },
    /// `.subckt` / `.macro name ports… params…`.
    Subckt {
        /// Subcircuit name.
        name: Option<Ident>,
        /// Port nodes in order.
        ports: Vec<Ident>,
        /// Header parameters with default values.
        params: Vec<ParamDef>,
    },
    /// `.ends` / `.eom [name]`.
    Ends {
        /// Optional subcircuit name.
        name: Option<Ident>,
    },
    /// `.endl [name]`.
    Endl {
        /// Optional section name.
        name: Option<Ident>,
    },
    /// `.end`.
    End,
    /// `.global nodes…`.
    Global {
        /// Global node names.
        nodes: Vec<Ident>,
    },
    /// `.option` / `.options` / `.opt`.
    Option {
        /// Option arguments (bare names and assignments).
        args: Vec<Arg>,
    },
    /// `.if (condition)`.
    If {
        /// Condition expression.
        condition: Option<Expr>,
    },
    /// `.elseif (condition)`.
    ElseIf {
        /// Condition expression.
        condition: Option<Expr>,
    },
    /// `.else`.
    Else,
    /// `.endif`.
    EndIf,
    /// `.title text`.
    Title {
        /// Title text.
        text: Option<String>,
    },
    /// `.alter [title]`.
    Alter {
        /// Optional title text.
        text: Option<String>,
    },
    /// `.hdl "file" [module] [alias]`.
    Hdl {
        /// Verilog-A file path.
        path: Option<PathArg>,
        /// Remaining arguments.
        args: Vec<Arg>,
    },
    /// Any other dot command (`.tran`, `.meas`, `.print` …).
    Generic {
        /// Arguments in source order.
        args: Vec<Arg>,
    },
}

/// A file path argument with its quotes removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathArg {
    /// Path text without quotes.
    pub text: String,
    /// Range of the path text (inside the quotes).
    pub range: TextRange,
}

/// One parameter definition: `name=value` or a user function `name(a, b)=value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    /// Parameter or function name.
    pub name: Ident,
    /// Formal arguments of a user function; empty for plain parameters.
    pub arguments: Vec<Ident>,
    /// Value expression, if present.
    pub value: Option<Expr>,
    /// True if the value was written inside quotes.
    pub quoted: bool,
    /// Range of the whole definition.
    pub range: TextRange,
}

/// An argument on an element or command line.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// A positional value.
    Value(Value),
    /// A `key=value` assignment.
    Assign(Assignment),
    /// A parenthesized group of arguments, e.g. a model card `( … )`.
    Group {
        /// Arguments inside the parentheses.
        args: Vec<Arg>,
        /// Range including the parentheses.
        range: TextRange,
    },
}

/// A `key=value` assignment; comma lists such as `AC=1,90` keep the extra values.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// The key: a name (`W`) or an output variable (`v(out)` in `.ic`).
    pub key: AssignKey,
    /// The value, if present.
    pub value: Option<Value>,
    /// Further comma-separated values (`AC=1,90` → `[90]`).
    pub extra_values: Vec<Value>,
    /// Range of the whole assignment.
    pub range: TextRange,
}

/// The left-hand side of an assignment.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignKey {
    /// A plain key such as `W`.
    Name(Ident),
    /// A call such as `v(out)` in `.ic v(out)=0`.
    Call(Call),
}

/// A positional or assigned value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A numeric literal with optional scale factor and unit, e.g. `1.5meg`.
    Number(NumberLit),
    /// A bare word: node, model, subckt, param reference or keyword.
    Word(Ident),
    /// An expression (quoted, or an unquoted assignment value).
    Expr(Expr),
    /// A file path string.
    Str(PathArg),
    /// A call such as `pulse(…)`, `v(out)`, `poly(2)` or `str(x)`.
    Call(Call),
}

/// A numeric literal as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLit {
    /// Literal text including sign, scale factor and unit letters.
    pub text: String,
    /// Location of the literal.
    pub range: TextRange,
}

/// A call-like argument `name(args…)` on an element or command line.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// Function or keyword name.
    pub name: Ident,
    /// Arguments inside the parentheses.
    pub args: Vec<Arg>,
    /// Range from the name to the closing parenthesis.
    pub range: TextRange,
}

/// An expression node.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Expression kind.
    pub kind: ExprKind,
    /// Location of the expression.
    pub range: TextRange,
}

/// Expression kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// Numeric literal text, e.g. `1u`.
    Number(String),
    /// A parameter, variable or node reference.
    Ident(Ident),
    /// A unary operation.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        operand: Box<Expr>,
    },
    /// A binary operation.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `condition ? then_branch : else_branch`.
    Ternary {
        /// Condition.
        condition: Box<Expr>,
        /// Value if true.
        then_branch: Box<Expr>,
        /// Value if false.
        else_branch: Box<Expr>,
    },
    /// A function call such as `max(a, b)` or `v(out)`.
    Call {
        /// Function name.
        name: Ident,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// A part tree-sitter could not parse.
    Error,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-x`
    Neg,
    /// `+x`
    Plus,
    /// `!x`
    Not,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `**` or `^`
    Pow,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

impl Value {
    /// Returns the source range of the value.
    pub fn range(&self) -> TextRange {
        match self {
            Value::Number(number) => number.range,
            Value::Word(ident) => ident.range,
            Value::Expr(expr) => expr.range,
            Value::Str(path) => path.range,
            Value::Call(call) => call.range,
        }
    }
}
