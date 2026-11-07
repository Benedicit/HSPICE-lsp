; Tree-sitter highlights for HSPICE netlists (Neovim capture names).
; The language server refines these with semantic tokens (nodes, params, models, subckts).

(comment) @comment @spell

(inline_comment) @comment @spell

[
  (line_continuation)
  (backslash_continuation)
] @punctuation.special

(command_name) @keyword.directive

(title_command
  text: (text) @markup.heading)

(alter_command
  text: (text) @markup.heading)

; Elements: the key letter selects the type.
(element
  name: (element_name) @constant)

((element
  name: (element_name) @function.call)
  (#match? @function.call "^[xX]"))

; Definitions.
(subckt_command
  subckt: (word) @module)

(subckt_command
  port: (word) @variable)

(ends_command
  subckt: (word) @module)

(model_command
  model: (word) @type.definition)

(model_command
  type: (word) @type.builtin)

(param_definition
  name: (identifier) @constant)

(parameter_list
  (identifier) @variable.parameter)

(lib_command
  section: (word) @label)

(endl_command
  section: (word) @label)

(global_command
  node: (word) @variable.builtin)

; Arguments.
(assignment
  key: (word) @property)

(file_assignment
  key: (word) @property)

(call
  function: (word) @function.call)

(call_expression
  function: (identifier) @function.call)

((call
  function: (word) @function.builtin)
  (#match? @function.builtin "\\c\\v^(pulse|pu|sin|exp|pwl|pl|pwlz|sffm|am|pat|lfsr|poly|v|vr|vi|vm|vp|vdb|vt|i|i[1-4]|ir|ii|im|ip|idb|it|p|par|str|lv[0-9]+|lx[0-9]+)$"))

((call_expression
  function: (identifier) @function.builtin)
  (#match? @function.builtin "\\c\\v^(sin|cos|tan|asin|acos|atan|sinh|cosh|tanh|abs|sqrt|log|log10|exp|db|int|nint|sgn|def|pow|pwr|sign|min|max|unif|aunif|gauss|agauss|limit|floor|ceil|val|valm|valp|str|v|i|p|par|lv[0-9]+|lx[0-9]+)$"))

((word) @keyword
  (#match? @keyword "\\c\\v^(dc|ac|pulse|pu|sin|exp|pwl|pl|sffm|am|pat|vcvs|vccs|cccs|ccvs|poly|dec|oct|lin|poi|sweep|uic|tran|noise|disto|trig|targ|when|find|avg|rms|min|max|pp|integ|integral|deriv|derivative|off)$"))

((word) @constant.builtin
  (#match? @constant.builtin "\\c\\v^(gnd|gnd!|ground|!gnd)$"))

((identifier) @variable.builtin
  (#match? @variable.builtin "\\c\\v^(time|temper|hertz)$"))

(number) @number

[
  (string)
  (path_word)
] @string.special.path

(quoted_expression
  [
    "'"
    "\""
  ] @punctuation.special)

[
  "("
  ")"
] @punctuation.bracket

"," @punctuation.delimiter

[
  "="
  "+"
  "-"
  "*"
  "/"
  "**"
  "^"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "?"
  ":"
] @operator
