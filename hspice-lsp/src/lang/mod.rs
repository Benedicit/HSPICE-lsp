//! Static HSPICE knowledge base used for completion, hover and validation.

pub mod commands;
pub mod elements;
pub mod functions;
pub mod measure;
pub mod models;
pub mod mosra;
pub mod mosra_model;
pub mod options;
pub mod output_vars;

/// How common a construct is in course netlists; used to rank completion items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Usage {
    /// Used in nearly every analog course netlist.
    Common,
    /// Used now and then.
    Occasional,
    /// Specialised (RF, signal integrity, reliability …).
    Rare,
}

/// Whether HSPICE supports a construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Supported by current HSPICE releases.
    Supported,
    /// Still accepted but documented as obsolete.
    Obsolete,
    /// Belongs to another SPICE dialect (ngspice, LTspice, PSpice) and is not HSPICE.
    Foreign,
}

/// A keyword with a short description, e.g. a `.measure` or `.ac` keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordSpec {
    /// Lowercase keyword.
    pub name: &'static str,
    /// Accepted alternative spellings (lowercase).
    pub aliases: &'static [&'static str],
    /// True if the keyword is written as `key=value`.
    pub takes_value: bool,
    /// One-line description.
    pub description: &'static str,
}

/// Table entries that can be looked up by a case-insensitive name or alias.
pub trait Named {
    /// Canonical lowercase name.
    fn name(&self) -> &'static str;

    /// Alternative lowercase spellings.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
}

impl Named for KeywordSpec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }
}

/// Finds the entry whose name or alias equals `name`, ignoring ASCII case.
///
/// # Arguments
/// * `table` - static table to search
/// * `name` - name as written in the netlist
pub fn find_named<T: Named>(table: &'static [T], name: &str) -> Option<&'static T> {
    table.iter().find(|entry| {
        std::iter::once(entry.name())
            .chain(entry.aliases().iter().copied())
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
    })
}
