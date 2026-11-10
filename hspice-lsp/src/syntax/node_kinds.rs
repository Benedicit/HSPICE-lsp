//! Node-kind and field-name constants of the tree-sitter-hspice grammar (checked against it by the tests below).

/// `*` comment line.
pub const COMMENT: &str = "comment";
/// `$` comment.
pub const INLINE_COMMENT: &str = "inline_comment";
/// `+` continuation (extra).
pub const LINE_CONTINUATION: &str = "line_continuation";
/// `.include` statement.
pub const INCLUDE_COMMAND: &str = "include_command";
/// `.lib` statement.
pub const LIB_COMMAND: &str = "lib_command";
/// `.param` statement.
pub const PARAM_COMMAND: &str = "param_command";
/// `.model` statement.
pub const MODEL_COMMAND: &str = "model_command";
/// `.subckt` statement.
pub const SUBCKT_COMMAND: &str = "subckt_command";
/// `.ends` statement.
pub const ENDS_COMMAND: &str = "ends_command";
/// `.endl` statement.
pub const ENDL_COMMAND: &str = "endl_command";
/// `.end` statement.
pub const END_COMMAND: &str = "end_command";
/// `.global` statement.
pub const GLOBAL_COMMAND: &str = "global_command";
/// `.option` statement.
pub const OPTION_COMMAND: &str = "option_command";
/// `.if` statement.
pub const IF_COMMAND: &str = "if_command";
/// `.elseif` statement.
pub const ELSEIF_COMMAND: &str = "elseif_command";
/// `.else` statement.
pub const ELSE_COMMAND: &str = "else_command";
/// `.endif` statement.
pub const ENDIF_COMMAND: &str = "endif_command";
/// `.title` statement.
pub const TITLE_COMMAND: &str = "title_command";
/// `.alter` statement.
pub const ALTER_COMMAND: &str = "alter_command";
/// `.hdl` statement.
pub const HDL_COMMAND: &str = "hdl_command";
/// Any other dot command.
pub const GENERIC_COMMAND: &str = "generic_command";
/// Element line.
pub const ELEMENT: &str = "element";
/// `key=value` argument.
pub const ASSIGNMENT: &str = "assignment";
/// `file=path` argument.
pub const FILE_ASSIGNMENT: &str = "file_assignment";
/// `name(args)` argument.
pub const CALL: &str = "call";
/// `( … )` argument group.
pub const PARENTHESIZED: &str = "parenthesized";
/// Numeric literal.
pub const NUMBER: &str = "number";
/// Bare word (node, model, keyword …).
pub const WORD: &str = "word";
/// Unquoted file path.
pub const PATH_WORD: &str = "path_word";
/// Quoted file path.
pub const STRING: &str = "string";
/// `name=value` inside `.param`.
pub const PARAM_DEFINITION: &str = "param_definition";
/// `'…'` or `"…"` expression.
pub const QUOTED_EXPRESSION: &str = "quoted_expression";
/// Binary operation.
pub const BINARY_EXPRESSION: &str = "binary_expression";
/// Unary operation.
pub const UNARY_EXPRESSION: &str = "unary_expression";
/// `?:` operation.
pub const TERNARY_EXPRESSION: &str = "ternary_expression";
/// Function call inside an expression.
pub const CALL_EXPRESSION: &str = "call_expression";
/// `( … )` inside an expression.
pub const PARENTHESIZED_EXPRESSION: &str = "parenthesized_expression";
/// Expression identifier.
pub const IDENTIFIER: &str = "identifier";
/// `key=(v1 v2 …)` or `key=list(…)` value.
pub const VALUE_LIST: &str = "value_list";

/// Field holding a statement keyword or element name.
pub const FIELD_NAME: &str = "name";
/// Field holding a file path.
pub const FIELD_PATH: &str = "path";
/// Field holding a `.lib` section name.
pub const FIELD_SECTION: &str = "section";
/// Field holding a `.model` name.
pub const FIELD_MODEL: &str = "model";
/// Field holding a `.model` type.
pub const FIELD_TYPE: &str = "type";
/// Field holding a `.subckt`/`.ends` name.
pub const FIELD_SUBCKT: &str = "subckt";
/// Field holding a `.subckt` port.
pub const FIELD_PORT: &str = "port";
/// Field holding a `.subckt` header parameter.
pub const FIELD_PARAMETER: &str = "parameter";
/// Field holding a user-function parameter list.
pub const FIELD_PARAMETERS: &str = "parameters";
/// Field holding a `.global` node.
pub const FIELD_NODE: &str = "node";
/// Field holding an `.if` condition.
pub const FIELD_CONDITION: &str = "condition";
/// Field holding `.title`/`.alter` text.
pub const FIELD_TEXT: &str = "text";
/// Field holding an assignment key.
pub const FIELD_KEY: &str = "key";
/// Field holding an assignment or definition value.
pub const FIELD_VALUE: &str = "value";
/// Field holding a call's function name.
pub const FIELD_FUNCTION: &str = "function";
/// Field holding a call argument.
pub const FIELD_ARGUMENT: &str = "argument";
/// Field holding a binary operator.
pub const FIELD_OPERATOR: &str = "operator";
/// Field holding a binary left operand.
pub const FIELD_LEFT: &str = "left";
/// Field holding a binary right operand.
pub const FIELD_RIGHT: &str = "right";
/// Field holding a unary operand.
pub const FIELD_OPERAND: &str = "operand";
/// Field holding a ternary true branch.
pub const FIELD_CONSEQUENCE: &str = "consequence";
/// Field holding a ternary false branch.
pub const FIELD_ALTERNATIVE: &str = "alternative";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_kind_exists_in_the_grammar() {
        let language: tree_sitter::Language = tree_sitter_hspice::LANGUAGE.into();
        let kinds = [
            COMMENT,
            INLINE_COMMENT,
            LINE_CONTINUATION,
            INCLUDE_COMMAND,
            LIB_COMMAND,
            PARAM_COMMAND,
            MODEL_COMMAND,
            SUBCKT_COMMAND,
            ENDS_COMMAND,
            ENDL_COMMAND,
            END_COMMAND,
            GLOBAL_COMMAND,
            OPTION_COMMAND,
            IF_COMMAND,
            ELSEIF_COMMAND,
            ELSE_COMMAND,
            ENDIF_COMMAND,
            TITLE_COMMAND,
            ALTER_COMMAND,
            HDL_COMMAND,
            GENERIC_COMMAND,
            ELEMENT,
            ASSIGNMENT,
            FILE_ASSIGNMENT,
            CALL,
            PARENTHESIZED,
            NUMBER,
            WORD,
            PATH_WORD,
            STRING,
            PARAM_DEFINITION,
            QUOTED_EXPRESSION,
            BINARY_EXPRESSION,
            UNARY_EXPRESSION,
            TERNARY_EXPRESSION,
            CALL_EXPRESSION,
            PARENTHESIZED_EXPRESSION,
            IDENTIFIER,
            VALUE_LIST,
        ];
        let missing: Vec<&str> = kinds
            .into_iter()
            .filter(|kind| language.id_for_node_kind(kind, true) == 0)
            .collect();
        assert!(missing.is_empty(), "unknown node kinds: {missing:?}");
    }

    #[test]
    fn every_field_exists_in_the_grammar() {
        let language: tree_sitter::Language = tree_sitter_hspice::LANGUAGE.into();
        let fields = [
            FIELD_NAME,
            FIELD_PATH,
            FIELD_SECTION,
            FIELD_MODEL,
            FIELD_TYPE,
            FIELD_SUBCKT,
            FIELD_PORT,
            FIELD_PARAMETER,
            FIELD_PARAMETERS,
            FIELD_NODE,
            FIELD_CONDITION,
            FIELD_TEXT,
            FIELD_KEY,
            FIELD_VALUE,
            FIELD_FUNCTION,
            FIELD_ARGUMENT,
            FIELD_OPERATOR,
            FIELD_LEFT,
            FIELD_RIGHT,
            FIELD_OPERAND,
            FIELD_CONSEQUENCE,
            FIELD_ALTERNATIVE,
        ];
        let missing: Vec<&str> = fields
            .into_iter()
            .filter(|field| language.field_id_for_name(field).is_none())
            .collect();
        assert!(missing.is_empty(), "unknown fields: {missing:?}");
    }
}
