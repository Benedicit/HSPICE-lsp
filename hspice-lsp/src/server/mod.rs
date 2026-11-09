//! LSP server: initialization handshake, capabilities and the main message loop.

pub mod dispatch;

use std::error::Error;
use std::path::PathBuf;

use ls_types::notification::{Notification as _, PublishDiagnostics};
use ls_types::{
    CodeActionProviderCapability, CompletionOptions, FoldingRangeProviderCapability,
    HoverProviderCapability, InitializeParams, InitializeResult, OneOf, PositionEncodingKind,
    PublishDiagnosticsParams, RenameOptions, SaveOptions, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, Uri,
};
use lsp_server::{Connection, Message, Notification};

use crate::config::Settings;
use crate::features::{diagnostics::to_lsp_diagnostics, semantic_tokens};
use crate::state::line_index::PositionEncoding;
use crate::state::workspace::{Workspace, file_path};

/// Result type of the server entry points.
pub type ServerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Runs the language server on an established connection until `shutdown`/`exit`.
///
/// # Arguments
/// * `connection` - client connection (stdio in production, in-memory in tests)
pub fn run(connection: &Connection) -> ServerResult<()> {
    let (id, params) = connection.initialize_start()?;
    let init: InitializeParams = serde_json::from_value(params)?;
    let encoding = negotiate_encoding(&init);
    let settings = init
        .initialization_options
        .as_ref()
        .map(Settings::from_json)
        .unwrap_or_default();
    let result = InitializeResult {
        capabilities: capabilities(encoding),
        server_info: Some(ServerInfo {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        offset_encoding: None,
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;
    let mut workspace = Workspace::new(workspace_root(&init), encoding, settings);
    main_loop(connection, &mut workspace)
}

/// Processes messages until the client requests shutdown.
///
/// # Arguments
/// * `connection` - client connection
/// * `workspace` - server state
fn main_loop(connection: &Connection, workspace: &mut Workspace) -> ServerResult<()> {
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                let response = dispatch::handle_request(workspace, request);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                let changed = dispatch::handle_notification(workspace, notification);
                publish_diagnostics(connection, workspace, changed)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// Publishes diagnostics for the given documents (an empty list for closed ones).
///
/// # Arguments
/// * `connection` - client connection
/// * `workspace` - server state
/// * `uris` - documents whose diagnostics changed
fn publish_diagnostics(
    connection: &Connection,
    workspace: &Workspace,
    uris: Vec<Uri>,
) -> ServerResult<()> {
    uris.into_iter().try_for_each(|uri| {
        let document = workspace.document(&uri);
        let params = PublishDiagnosticsParams {
            diagnostics: document
                .map(|document| to_lsp_diagnostics(workspace, document))
                .unwrap_or_default(),
            version: document.map(|document| document.version),
            uri,
        };
        let notification = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
        connection
            .sender
            .send(Message::Notification(notification))?;
        Ok(())
    })
}

/// Picks UTF-8 positions if the client supports them, otherwise the LSP default UTF-16.
///
/// # Arguments
/// * `init` - initialize parameters
fn negotiate_encoding(init: &InitializeParams) -> PositionEncoding {
    let supports_utf8 = init
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
        .is_some_and(|encodings| encodings.contains(&PositionEncodingKind::UTF8));
    if supports_utf8 {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    }
}

/// Returns the workspace root from the workspace folders or the root URI.
///
/// # Arguments
/// * `init` - initialize parameters
#[allow(deprecated)]
fn workspace_root(init: &InitializeParams) -> Option<PathBuf> {
    let folder = init
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .map(|folder| folder.uri.clone());
    folder
        .or_else(|| init.root_uri.clone())
        .and_then(|uri| file_path(&uri))
}

/// Returns the capabilities advertised to the client.
///
/// # Arguments
/// * `encoding` - negotiated position encoding
fn capabilities(encoding: PositionEncoding) -> ServerCapabilities {
    let to_strings = |characters: &[&str]| {
        characters
            .iter()
            .map(|character| (*character).to_string())
            .collect()
    };
    ServerCapabilities {
        position_encoding: Some(match encoding {
            PositionEncoding::Utf8 => PositionEncodingKind::UTF8,
            PositionEncoding::Utf16 => PositionEncodingKind::UTF16,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(false),
                })),
                ..Default::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(to_strings(&[".", "(", "=", "'", "/", "+"])),
            resolve_provider: Some(false),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(to_strings(&["(", " ", ","])),
            retrigger_characters: None,
            work_done_progress_options: Default::default(),
        }),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: semantic_tokens::legend(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: None,
                work_done_progress_options: Default::default(),
            },
        )),
        inlay_hint_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}
