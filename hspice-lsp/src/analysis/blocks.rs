//! Rebuilds block structure (`.subckt`, `.lib`, `.if`, `.data` …) and scopes from flat statements.

use super::symbols::{ROOT_SCOPE, Scope, ScopeId};
use crate::syntax::ast::{Arg, Command, CommandKind, Ident, Netlist, StatementKind, TextRange};

/// Kinds of statement blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// `.subckt` / `.macro` … `.ends` / `.eom`.
    Subckt,
    /// `.lib name` … `.endl`.
    LibSection,
    /// `.if` … `.endif`.
    If,
    /// `.data` … `.enddata`.
    Data,
    /// Plain-text `.protect` … `.unprotect`.
    Protect,
    /// `.variation` … `.end_variation` (its own sub-language).
    Variation,
    /// `.design_exploration` … `.end_design_exploration`.
    DesignExploration,
    /// `.module` … `.endmodule` (3D-IC module).
    Module,
    /// `.modulevar` … `.endmodulevar` (3D-IC module properties).
    ModuleVar,
}

impl BlockKind {
    /// Returns the opening and closing keywords.
    pub fn keywords(self) -> (&'static str, &'static str) {
        match self {
            BlockKind::Subckt => (".subckt", ".ends"),
            BlockKind::LibSection => (".lib", ".endl"),
            BlockKind::If => (".if", ".endif"),
            BlockKind::Data => (".data", ".enddata"),
            BlockKind::Protect => (".protect", ".unprotect"),
            BlockKind::Variation => (".variation", ".end_variation"),
            BlockKind::DesignExploration => (".design_exploration", ".end_design_exploration"),
            BlockKind::Module => (".module", ".endmodule"),
            BlockKind::ModuleVar => (".modulevar", ".endmodulevar"),
        }
    }
}

/// One block with its opening and (optional) closing statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Block kind.
    pub kind: BlockKind,
    /// Statement index of the opener.
    pub open: usize,
    /// Statement index of the closer, if present.
    pub close: Option<usize>,
    /// Name given at the opener (subckt or section name).
    pub name: Option<Ident>,
    /// Name given at the closer (`.ends inv`).
    pub close_name: Option<Ident>,
    /// Range from the opener to the closer (or to the last statement if unclosed).
    pub range: TextRange,
}

/// Block structure and per-statement context of a document.
#[derive(Debug, Clone, PartialEq)]
pub struct Structure {
    /// All blocks in opening order.
    pub blocks: Vec<Block>,
    /// Scope tree; index 0 is the document top level.
    pub scopes: Vec<Scope>,
    /// Scope of each statement (a `.subckt` statement belongs to its parent scope).
    pub statement_scope: Vec<ScopeId>,
    /// Enclosing `.lib` section (lowercase) of each statement.
    pub statement_section: Vec<Option<String>>,
    /// `.alter` section of each statement (0 before the first `.alter`).
    pub statement_alter: Vec<usize>,
    /// True for statements whose semantics are skipped (opaque regions and unparsable lines).
    pub skipped: Vec<bool>,
    /// True inside `.variation` blocks and encrypted `.prot <method>` regions (no syntax errors reported).
    pub opaque: Vec<bool>,
    /// Closers (`.ends`, `.endl`, `.endif` …) and `.else`/`.elseif` without a matching opener.
    pub unmatched: Vec<usize>,
}

impl Structure {
    /// Returns true if the semantics of a statement are skipped.
    ///
    /// # Arguments
    /// * `statement` - statement index
    pub fn is_skipped(&self, statement: usize) -> bool {
        self.skipped.get(statement).copied().unwrap_or(false)
    }

    /// Returns the scope of a statement.
    ///
    /// # Arguments
    /// * `statement` - statement index
    pub fn scope_of(&self, statement: usize) -> ScopeId {
        self.statement_scope
            .get(statement)
            .copied()
            .unwrap_or(ROOT_SCOPE)
    }

    /// Returns the scope opened by a `.subckt` statement.
    ///
    /// # Arguments
    /// * `statement` - statement index of the `.subckt`
    pub fn subckt_scope(&self, statement: usize) -> Option<ScopeId> {
        self.scopes
            .iter()
            .position(|scope| scope.subckt_statement == Some(statement))
    }
}

/// Role of a statement in the block structure.
enum BlockRole {
    Open(BlockKind, Option<Ident>),
    Close(BlockKind, Option<Ident>),
    Branch,
    Alter,
    EncryptedProtect,
    None,
}

/// Classifies a command by its effect on the block structure.
///
/// # Arguments
/// * `command` - lowered dot command
fn block_role(command: &Command) -> BlockRole {
    match &command.kind {
        CommandKind::Subckt { name, .. } => BlockRole::Open(BlockKind::Subckt, name.clone()),
        CommandKind::Ends { name } => BlockRole::Close(BlockKind::Subckt, name.clone()),
        CommandKind::Lib {
            path: None,
            section,
        } => BlockRole::Open(BlockKind::LibSection, section.clone()),
        CommandKind::Endl { name } => BlockRole::Close(BlockKind::LibSection, name.clone()),
        CommandKind::If { .. } => BlockRole::Open(BlockKind::If, None),
        CommandKind::ElseIf { .. } | CommandKind::Else => BlockRole::Branch,
        CommandKind::EndIf => BlockRole::Close(BlockKind::If, None),
        CommandKind::Alter { .. } => BlockRole::Alter,
        CommandKind::Generic { args } => generic_block_role(&command.keyword.key, args),
        _ => BlockRole::None,
    }
}

/// Returns the block kind a command closes or continues (`.ends`, `.else` ...).
///
/// # Arguments
/// * `command` - lowered dot command
pub fn closed_kind(command: &Command) -> Option<BlockKind> {
    match block_role(command) {
        BlockRole::Close(kind, _) => Some(kind),
        BlockRole::Branch => Some(BlockKind::If),
        _ => None,
    }
}

/// Classifies generic commands that open or close blocks.
///
/// # Arguments
/// * `keyword` - lowercase command keyword
/// * `args` - command arguments
fn generic_block_role(keyword: &str, args: &[Arg]) -> BlockRole {
    match keyword {
        ".data" => BlockRole::Open(BlockKind::Data, first_word(args)),
        ".enddata" => BlockRole::Close(BlockKind::Data, None),
        ".prot" | ".protect" if args.is_empty() => BlockRole::Open(BlockKind::Protect, None),
        ".prot" | ".protect" => BlockRole::EncryptedProtect,
        ".unprot" | ".unprotect" => BlockRole::Close(BlockKind::Protect, None),
        ".variation" => BlockRole::Open(BlockKind::Variation, None),
        ".end_variation" => BlockRole::Close(BlockKind::Variation, None),
        ".design_exploration" => BlockRole::Open(BlockKind::DesignExploration, None),
        ".end_design_exploration" => BlockRole::Close(BlockKind::DesignExploration, None),
        ".module" => BlockRole::Open(BlockKind::Module, first_word(args)),
        ".endmodule" => BlockRole::Close(BlockKind::Module, None),
        ".modulevar" => BlockRole::Open(BlockKind::ModuleVar, first_word(args)),
        ".endmodulevar" => BlockRole::Close(BlockKind::ModuleVar, None),
        _ => BlockRole::None,
    }
}

/// Returns the first argument if it is a bare word.
///
/// # Arguments
/// * `args` - command arguments
fn first_word(args: &[Arg]) -> Option<Ident> {
    match args.first() {
        Some(Arg::Value(crate::syntax::ast::Value::Word(word))) => Some(word.clone()),
        _ => None,
    }
}

/// Mutable state threaded through the block-building fold.
struct Builder {
    structure: Structure,
    open_blocks: Vec<usize>,
    scope_stack: Vec<ScopeId>,
    alter: usize,
    in_encrypted_region: bool,
}

impl Builder {
    /// Returns the innermost scope.
    fn current_scope(&self) -> ScopeId {
        self.scope_stack.last().copied().unwrap_or(ROOT_SCOPE)
    }

    /// Returns true if an open block of the given kind encloses the current statement.
    ///
    /// # Arguments
    /// * `kind` - block kind
    fn inside(&self, kind: BlockKind) -> bool {
        self.open_blocks
            .iter()
            .any(|index| self.structure.blocks[*index].kind == kind)
    }

    /// Returns the innermost enclosing `.lib` section name.
    fn current_section(&self) -> Option<String> {
        self.open_blocks
            .iter()
            .rev()
            .map(|index| &self.structure.blocks[*index])
            .find(|block| block.kind == BlockKind::LibSection)
            .and_then(|block| block.name.as_ref().map(|name| name.key.clone()))
    }

    /// Opens a block at statement `index`, creating a scope for subcircuits.
    ///
    /// # Arguments
    /// * `kind` - block kind
    /// * `name` - optional block name
    /// * `index` - statement index
    /// * `range` - statement range
    fn open(&mut self, kind: BlockKind, name: Option<Ident>, index: usize, range: TextRange) {
        self.structure.blocks.push(Block {
            kind,
            open: index,
            close: None,
            name,
            close_name: None,
            range,
        });
        self.open_blocks.push(self.structure.blocks.len() - 1);
        if kind == BlockKind::Subckt {
            self.structure.scopes.push(Scope {
                parent: Some(self.current_scope()),
                subckt_statement: Some(index),
                range,
            });
            self.scope_stack.push(self.structure.scopes.len() - 1);
        }
    }

    /// Closes the innermost open block of `kind`; inner unclosed blocks stay unclosed.
    ///
    /// # Arguments
    /// * `kind` - block kind
    /// * `name` - name given at the closer
    /// * `index` - statement index of the closer
    /// * `range` - range of the closer
    fn close(&mut self, kind: BlockKind, name: Option<Ident>, index: usize, range: TextRange) {
        let Some(position) = self
            .open_blocks
            .iter()
            .rposition(|block| self.structure.blocks[*block].kind == kind)
        else {
            self.structure.unmatched.push(index);
            return;
        };
        let closed: Vec<usize> = self.open_blocks.split_off(position);
        closed.iter().for_each(|block_index| {
            let block_kind = self.structure.blocks[*block_index].kind;
            if block_kind == BlockKind::Subckt {
                self.scope_stack.pop();
            }
        });
        let block = &mut self.structure.blocks[closed[0]];
        block.close = Some(index);
        block.close_name = name;
        block.range = block.range.cover(range);
        if kind == BlockKind::Subckt
            && let Some(scope) = self
                .structure
                .scopes
                .iter_mut()
                .rev()
                .find(|scope| scope.subckt_statement == Some(block.open))
        {
            scope.range = block.range;
        }
    }

    /// Extends the ranges of all open blocks (and their scopes) to cover a statement.
    ///
    /// # Arguments
    /// * `range` - statement range
    fn extend_open(&mut self, range: TextRange) {
        let open: Vec<usize> = self.open_blocks.clone();
        open.into_iter().for_each(|index| {
            let block = &mut self.structure.blocks[index];
            block.range = block.range.cover(range);
            let opener = block.open;
            if block.kind == BlockKind::Subckt
                && let Some(scope) = self
                    .structure
                    .scopes
                    .iter_mut()
                    .find(|scope| scope.subckt_statement == Some(opener))
            {
                scope.range = scope.range.cover(range);
            }
        });
    }
}

/// Builds the block structure and scopes of a lowered document.
///
/// # Arguments
/// * `netlist` - lowered document
pub fn build_structure(netlist: &Netlist) -> Structure {
    let document_range = netlist
        .statements
        .iter()
        .map(|statement| statement.range)
        .reduce(TextRange::cover)
        .unwrap_or_default();
    let mut builder = Builder {
        structure: Structure {
            blocks: Vec::new(),
            scopes: vec![Scope {
                parent: None,
                subckt_statement: None,
                range: document_range,
            }],
            statement_scope: Vec::with_capacity(netlist.statements.len()),
            statement_section: Vec::with_capacity(netlist.statements.len()),
            statement_alter: Vec::with_capacity(netlist.statements.len()),
            skipped: Vec::with_capacity(netlist.statements.len()),
            opaque: Vec::with_capacity(netlist.statements.len()),
            unmatched: Vec::new(),
        },
        open_blocks: Vec::new(),
        scope_stack: vec![ROOT_SCOPE],
        alter: 0,
        in_encrypted_region: false,
    };
    netlist
        .statements
        .iter()
        .enumerate()
        .for_each(|(index, statement)| {
            let role = match &statement.kind {
                StatementKind::Command(command) => block_role(command),
                _ => BlockRole::None,
            };
            if matches!(statement.kind, StatementKind::Command(_)) {
                builder.in_encrypted_region = false;
            }
            builder.extend_open(statement.range);
            let scope_before = builder.current_scope();
            let opaque = builder.inside(BlockKind::Variation) || builder.in_encrypted_region;
            let skipped = opaque || matches!(statement.kind, StatementKind::Error);
            let section = builder.current_section();
            match role {
                BlockRole::Open(kind, name) => builder.open(kind, name, index, statement.range),
                BlockRole::Close(kind, name) => builder.close(kind, name, index, statement.range),
                BlockRole::Branch if !builder.inside(BlockKind::If) => {
                    builder.structure.unmatched.push(index)
                }
                BlockRole::Alter => builder.alter += 1,
                BlockRole::EncryptedProtect => builder.in_encrypted_region = true,
                BlockRole::Branch | BlockRole::None => {}
            }
            builder.structure.statement_scope.push(scope_before);
            builder.structure.statement_section.push(section);
            builder.structure.statement_alter.push(builder.alter);
            builder.structure.skipped.push(skipped);
            builder.structure.opaque.push(opaque);
        });
    builder.structure
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{lower::lower, parse::parse_tree};

    fn structure_of(text: &str) -> Structure {
        let tree = parse_tree(text).expect("parser");
        build_structure(&lower(&tree, text))
    }

    #[test]
    fn subckt_creates_a_scope() {
        let structure = structure_of(".subckt inv a b\nM1 a b 0 0 nch\n.ends inv\nR1 x y 1k\n");
        assert_eq!(structure.scopes.len(), 2);
        assert_eq!(structure.statement_scope, vec![0, 1, 1, 0]);
        assert_eq!(structure.blocks[0].close, Some(2));
        assert_eq!(
            structure.blocks[0]
                .close_name
                .as_ref()
                .map(|name| name.key.as_str()),
            Some("inv")
        );
    }

    #[test]
    fn unclosed_and_unmatched_blocks_are_reported() {
        let structure = structure_of(".endl\n.subckt a x\nR1 x 0 1\n.else\n");
        assert_eq!(structure.unmatched, vec![0, 3]);
        assert_eq!(structure.blocks[0].close, None);
    }

    #[test]
    fn lib_sections_and_alters_are_tracked() {
        let structure = structure_of(".lib tt\n.param a=1\n.endl tt\n.alter\n.param a=2\n");
        assert_eq!(structure.statement_section[1].as_deref(), Some("tt"));
        assert_eq!(structure.statement_section[4], None);
        assert_eq!(structure.statement_alter, vec![0, 0, 0, 1, 1]);
    }

    #[test]
    fn variation_and_encrypted_regions_are_skipped() {
        let structure = structure_of(
            ".variation\noption x=1\n.end_variation\n.prot FREELIB\n5f8Kx#!@^Qz((\n.ends\n",
        );
        assert!(structure.opaque[1]);
        assert!(!structure.opaque[3]);
        assert!(structure.opaque[4]);
        assert!(!structure.opaque[5]);
    }
}
