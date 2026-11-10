//! User settings received via `initializationOptions` and `workspace/didChangeConfiguration`.

use serde::Deserialize;

/// All server settings; every field has a default so partial JSON objects are accepted.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Extra directories searched for `.include` / `.lib` files.
    pub include_paths: Vec<String>,
    /// Toggles for individual diagnostics.
    pub diagnostics: DiagnosticSettings,
    /// Toggles for inlay hints.
    pub inlay_hints: InlayHintSettings,
    /// Workspace scan used to find netlists that include an open file.
    pub workspace_scan: WorkspaceScanSettings,
    /// Toggles for folding ranges.
    pub folding: FoldingSettings,
    /// Included files larger than this are skipped.
    pub max_include_file_size_mb: u64,
}

/// Toggles for diagnostics that may be noisy in some projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiagnosticSettings {
    /// Warn about nodes with a single connection.
    pub floating_nodes: bool,
    /// Hint about parameters that are never used.
    pub unused_params: bool,
    /// Warn about unknown `.option` names, `.mosra` parameters and undocumented `key=value` values.
    pub unknown_options: bool,
    /// Hint when `M` (milli) is probably meant as `MEG`.
    pub meg_suffix: bool,
}

/// Toggles for inlay hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InlayHintSettings {
    /// Port / terminal names before element nodes.
    pub ports: bool,
    /// Argument names inside `pulse(…)`, `sin(…)` and similar source functions.
    pub source_args: bool,
}

/// Toggles for folding ranges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FoldingSettings {
    /// Fold runs of consecutive comment lines.
    pub comments: bool,
    /// Fold statements continued over several lines with `+`.
    pub continuations: bool,
}

/// Bounds for the workspace scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkspaceScanSettings {
    /// Whether to scan the workspace at all.
    pub enabled: bool,
    /// Maximum number of netlist files to read.
    pub max_files: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            include_paths: Vec::new(),
            diagnostics: DiagnosticSettings::default(),
            inlay_hints: InlayHintSettings::default(),
            workspace_scan: WorkspaceScanSettings::default(),
            folding: FoldingSettings::default(),
            max_include_file_size_mb: 64,
        }
    }
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        Self {
            floating_nodes: true,
            unused_params: true,
            unknown_options: true,
            meg_suffix: true,
        }
    }
}

impl Default for InlayHintSettings {
    fn default() -> Self {
        Self {
            ports: true,
            source_args: true,
        }
    }
}

impl Default for WorkspaceScanSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_files: 500,
        }
    }
}

impl Settings {
    /// Parses settings from a JSON value, accepting both the bare object and `{ "hspice": {...} }`.
    ///
    /// # Arguments
    /// * `value` - JSON from the client
    ///
    /// # Returns
    /// Defaults for missing or malformed input.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let object = value.get("hspice").unwrap_or(value);
        serde_json::from_value(object.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_settings_keep_defaults() {
        let settings = Settings::from_json(&serde_json::json!({
            "hspice": { "includePaths": ["/pdk"], "diagnostics": { "floatingNodes": false } }
        }));
        assert_eq!(settings.include_paths, vec!["/pdk".to_string()]);
        assert!(!settings.diagnostics.floating_nodes);
        assert!(settings.diagnostics.meg_suffix);
        assert_eq!(settings.workspace_scan.max_files, 500);
        assert!(!settings.folding.comments);
        assert!(!settings.folding.continuations);
    }

    #[test]
    fn malformed_settings_fall_back_to_defaults() {
        assert_eq!(
            Settings::from_json(&serde_json::json!("nonsense")),
            Settings::default()
        );
    }
}
