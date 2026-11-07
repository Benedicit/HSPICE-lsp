/**
 * @file Tree-sitter grammar for Synopsys HSPICE netlists.
 * @license MIT
 *
 * The grammar is line-oriented and flat: every logical line (including `+` continuation lines) is one
 * statement. Block structure (.subckt/.ends, .lib/.endl, .if/.endif …) and the meaning of positional
 * element arguments are resolved by the language server, which also handles the title line.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

/**
 * Builds a case-insensitive token for one dot command and its aliases.
 *
 * @param {...string} names command names without the leading dot, longest alias first
 * @returns {RegExp}
 */
const dotKeyword = (...names) => new RegExp(`\\.(${names.join('|')})`, 'i');

/**
 * Builds a comma-separated list with at least one element.
 *
 * @param {RuleOrLiteral} rule list element
 * @returns {SeqRule}
 */
const commaSep1 = (rule) => seq(rule, repeat(seq(',', rule)));

const PREC = {
  ternary: 1,
  or: 2,
  and: 3,
  equality: 4,
  comparison: 5,
  additive: 6,
  multiplicative: 7,
  unary: 8,
  power: 9,
  call: 10,
};

export default grammar({
  name: 'hspice',

  extras: $ => [/[ \t]/, $.inline_comment, $.line_continuation, $.backslash_continuation],

  conflicts: $ => [],

  rules: {
    source_file: $ => seq(
      repeat(choice(seq($._statement, $._newline), $._newline)),
      optional($._statement),
    ),

    _statement: $ => choice(
      $.comment,
      $.include_command,
      $.lib_command,
      $.param_command,
      $.model_command,
      $.subckt_command,
      $.ends_command,
      $.endl_command,
      $.end_command,
      $.global_command,
      $.option_command,
      $.if_command,
      $.elseif_command,
      $.else_command,
      $.endif_command,
      $.title_command,
      $.alter_command,
      $.hdl_command,
      $.generic_command,
      $.element,
    ),

    // ---------------------------------------------------------------- lexical helpers

    _newline: _ => /\r?\n/,

    comment: _ => token(seq('*', /[^\r\n]*/)),

    inline_comment: _ => token(seq('$', /[^\r\n]*/)),

    // A newline followed by optional comment lines and a `+` as the first non-blank character.
    line_continuation: _ => token(seq(/\r?\n/, repeat(/[ \t]*[*$][^\r\n]*\r?\n/), /[ \t]*\+/)),

    backslash_continuation: _ => token(seq(/\\\\?/, /\r?\n/)),

    raw_text: _ => /[^\r\n]+/,

    // ---------------------------------------------------------------- dot commands

    include_command: $ => seq(
      field('name', alias(dotKeyword('include', 'incl', 'inc'), $.command_name)),
      optional(field('path', $._path)),
    ),

    lib_command: $ => seq(
      field('name', alias(dotKeyword('lib'), $.command_name)),
      choice(
        seq(field('path', $._path), field('section', $._section_name)),
        field('section', alias($.path_word, $.word)),
      ),
    ),

    param_command: $ => seq(
      field('name', alias(dotKeyword('parameters', 'parameter', 'param', 'para'), $.command_name)),
      repeat(choice($.param_definition, ',')),
    ),

    model_command: $ => seq(
      field('name', alias(dotKeyword('model'), $.command_name)),
      field('model', $._word),
      optional(seq(field('type', $._word), repeat($._argument))),
    ),

    subckt_command: $ => seq(
      field('name', alias(dotKeyword('subckt', 'macro'), $.command_name)),
      field('subckt', $._word),
      optional(seq('(', repeat(choice(field('port', choice($._word, $.number)), ',')), ')')),
      repeat(choice(field('port', choice($._word, $.number)), field('parameter', $.assignment), ',')),
    ),

    ends_command: $ => seq(
      field('name', alias(dotKeyword('ends', 'eom'), $.command_name)),
      optional(field('subckt', $._word)),
    ),

    endl_command: $ => seq(
      field('name', alias(dotKeyword('endl'), $.command_name)),
      optional(field('section', $._word)),
    ),

    end_command: $ => seq(
      field('name', alias(dotKeyword('end'), $.command_name)),
      optional(field('comment', alias($.raw_text, $.comment))),
    ),

    global_command: $ => seq(
      field('name', alias(dotKeyword('global'), $.command_name)),
      repeat(choice(field('node', choice($._word, $.number)), ',')),
    ),

    option_command: $ => seq(
      field('name', alias(dotKeyword('options', 'option', 'opt'), $.command_name)),
      repeat($._argument),
    ),

    if_command: $ => seq(
      field('name', alias(dotKeyword('if'), $.command_name)),
      optional(field('condition', $._value_expression)),
    ),

    elseif_command: $ => seq(
      field('name', alias(dotKeyword('elseif'), $.command_name)),
      optional(field('condition', $._value_expression)),
    ),

    else_command: $ => field('name', alias(dotKeyword('else'), $.command_name)),

    endif_command: $ => field('name', alias(dotKeyword('endif'), $.command_name)),

    title_command: $ => seq(
      field('name', alias(dotKeyword('title'), $.command_name)),
      optional(field('text', alias($.raw_text, $.text))),
    ),

    alter_command: $ => seq(
      field('name', alias(dotKeyword('alter'), $.command_name)),
      optional(field('text', alias($.raw_text, $.text))),
    ),

    hdl_command: $ => seq(
      field('name', alias(dotKeyword('hdl'), $.command_name)),
      optional(seq(field('path', $._path), repeat($._argument))),
    ),

    generic_command: $ => seq(
      field('name', $.command_name),
      repeat($._argument),
    ),

    // ---------------------------------------------------------------- elements

    element: $ => seq(
      field('name', $.element_name),
      repeat($._argument),
    ),

    element_name: _ => /[a-zA-Z][^\s,=()'"\\]*/,

    // ---------------------------------------------------------------- arguments

    _argument: $ => choice(
      $.assignment,
      $.file_assignment,
      $._value,
      $.parenthesized,
      ',',
    ),

    _value: $ => choice(
      $.number,
      $._word,
      $.quoted_expression,
      $.call,
    ),

    assignment: $ => seq(
      field('key', choice($.word, $.call)),
      '=',
      field('value', choice($._value_expression, $.value_list)),
    ),

    // Two or more values: `listfreq=(1k 10k)`, `layer=(m1,0.5u)`, `monte=list(10 20:30)`.
    value_list: $ => choice(
      prec(PREC.call, seq(field('function', $.identifier), '(', $._list_item, repeat1($._list_item), ')')),
      seq('(', $._list_item, repeat1(seq(optional(','), $._list_item)), ')'),
    ),

    _list_item: $ => choice($.number, $.identifier, $.quoted_expression, seq($.number, ':', $.number)),

    file_assignment: $ => seq(
      field('key', alias($.file_key, $.word)),
      '=',
      field('value', $._path),
    ),

    call: $ => prec(PREC.call, seq(
      field('function', $._word),
      '(',
      repeat(field('argument', choice($._argument, alias($._wildcard, $.word)))),
      ')',
    )),

    // `v(*)`, `i(*)`: a word may not start with `*` elsewhere (comment lines).
    _wildcard: _ => /\*[^\s,=()'"\\]*/,

    parenthesized: $ => seq('(', repeat($._argument), ')'),

    number: $ => seq(optional(choice('-', '+')), $._number_literal),

    _number_literal: _ => /(\d+\.?\d*|\.\d+)([eEdD][+-]?\d+)?[a-zA-Z]*/,

    // Keys whose value is a file path rather than an expression; also usable as plain words.
    file_key: _ => /(file|rlgcfile|tablefile|pwlfile|filename|search)/i,

    _word: $ => choice($.word, alias($.file_key, $.word)),

    word: _ => /[^\s,=()'"$*+\-.\\][^\s,=()'"\\]*/,

    _path: $ => choice($.string, $.path_word),

    _section_name: $ => choice($._word, $.string),

    path_word: _ => /[^\s,=()'"$]+/,

    string: _ => choice(/'[^'\r\n]*'/, /"[^"\r\n]*"/),

    // ---------------------------------------------------------------- expressions

    param_definition: $ => seq(
      field('name', $.identifier),
      optional(field('parameters', $.parameter_list)),
      '=',
      field('value', $._value_expression),
    ),

    parameter_list: $ => seq('(', optional(commaSep1($.identifier)), ')'),

    quoted_expression: $ => choice(
      seq("'", $._expression, "'"),
      seq('"', $._expression, '"'),
    ),

    _value_expression: $ => choice($._expression, $.quoted_expression),

    _expression: $ => choice(
      $.binary_expression,
      $.unary_expression,
      $.ternary_expression,
      $.call_expression,
      $.parenthesized_expression,
      // Beats a list item: `(a -1)` is a subtraction, not two values.
      prec(1, alias($._number_literal, $.number)),
      prec(1, $.identifier),
    ),

    binary_expression: $ => choice(
      ...[
        ['||', PREC.or],
        ['&&', PREC.and],
        ['==', PREC.equality],
        ['!=', PREC.equality],
        ['<', PREC.comparison],
        ['<=', PREC.comparison],
        ['>', PREC.comparison],
        ['>=', PREC.comparison],
        ['+', PREC.additive],
        ['-', PREC.additive],
        ['*', PREC.multiplicative],
        ['/', PREC.multiplicative],
      ].map(([operator, precedence]) => prec.left(precedence, seq(
        field('left', $._expression),
        field('operator', operator),
        field('right', $._expression),
      ))),
      prec.right(PREC.power, seq(
        field('left', $._expression),
        field('operator', choice('**', '^')),
        field('right', $._expression),
      )),
    ),

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '+', '!')),
      field('operand', $._expression),
    )),

    ternary_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._expression),
      '?',
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    call_expression: $ => prec(PREC.call, seq(
      field('function', $.identifier),
      '(',
      optional(commaSep1(field('argument', $._value_expression))),
      ')',
    )),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    identifier: _ => /[a-zA-Z_][a-zA-Z0-9_.#]*/,

    // Declared last so that every specific dot-command keyword wins ties against it.
    command_name: _ => /\.[a-zA-Z_][a-zA-Z0-9_]*/,
  },
});
