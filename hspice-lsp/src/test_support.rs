//! Helpers shared by the unit tests.

use std::path::PathBuf;
use std::str::FromStr;

use ls_types::{Position, Uri};

use crate::config::Settings;
use crate::state::line_index::PositionEncoding;
use crate::state::workspace::Workspace;

/// An opened document and the marker position.
pub struct Opened {
    /// Workspace holding the document.
    pub workspace: Workspace,
    /// Document URI.
    pub uri: Uri,
    /// Position of the removed `|` marker.
    pub position: Position,
}

/// Opens a document whose text contains one `|` cursor marker.
///
/// # Arguments
/// * `marked` - document text with a `|` marker
pub fn open(marked: &str) -> Opened {
    open_at(None, "file:///tmp/t.sp", marked)
}

/// Opens a document with a marker in a workspace with an optional root.
///
/// # Arguments
/// * `root` - workspace root directory
/// * `uri` - document URI
/// * `marked` - document text with a `|` marker
pub fn open_at(root: Option<PathBuf>, uri: &str, marked: &str) -> Opened {
    let offset = marked.find('|').unwrap_or_default();
    let text = marked.replacen('|', "", 1);
    let mut workspace = Workspace::new(root, PositionEncoding::Utf16, Settings::default());
    let uri = Uri::from_str(uri).expect("uri");
    workspace.update(uri.clone(), 1, text.clone());
    let position = workspace
        .document(&uri)
        .map(|document| {
            document
                .line_index
                .position(&text, offset, PositionEncoding::Utf16)
        })
        .unwrap_or_default();
    Opened {
        workspace,
        uri,
        position,
    }
}

/// Returns the directory of the test fixtures.
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Opens a fixture file in a workspace.
///
/// # Arguments
/// * `workspace` - workspace to open the file in
/// * `relative` - path below `tests/fixtures`
pub fn open_fixture(workspace: &mut Workspace, relative: &str) -> Uri {
    let path = fixtures().join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let uri = Uri::from_file_path(&path).expect("file uri");
    workspace.update(uri.clone(), 1, text);
    uri
}

/// Opens a marked document located inside the fixtures directory.
///
/// # Arguments
/// * `marked` - document text with a `|` marker
pub fn open_in_fixtures(marked: &str) -> Opened {
    let path = fixtures().join("scratch_tb.sp");
    let uri = Uri::from_file_path(&path)
        .expect("file uri")
        .as_str()
        .to_string();
    open_at(Some(fixtures()), &uri, marked)
}
