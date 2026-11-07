//! Routing of LSP requests and notifications to feature functions.

use ls_types::Uri;
use ls_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Notification as LspNotification,
};
use ls_types::request::{
    CodeActionRequest, Completion, DocumentHighlightRequest, DocumentSymbolRequest,
    FoldingRangeRequest, GotoDefinition, HoverRequest, InlayHintRequest, PrepareRenameRequest,
    References, Rename, Request as LspRequest, SemanticTokensFullRequest, SignatureHelpRequest,
    WorkspaceSymbolRequest,
};
use lsp_server::{ErrorCode, Notification, Request, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::config::Settings;
use crate::features::{
    code_actions, completion, folding, hover, inlay_hints, navigation, semantic_tokens,
    signature_help, symbols,
};
use crate::state::document::Document;
use crate::state::workspace::Workspace;

/// Answers a request; unknown methods get a `MethodNotFound` error.
///
/// # Arguments
/// * `workspace` - server state
/// * `request` - incoming request
pub fn handle_request(workspace: &Workspace, request: Request) -> Response {
    let ws = workspace;
    respond::<Completion>(&request, |params| {
        let position = params.text_document_position;
        Ok(on_document(ws, &position.text_document.uri, |document| {
            completion::completion(ws, document, position.position)
        }))
    })
    .or_else(|| {
        respond::<HoverRequest>(&request, |params| {
            let position = params.text_document_position_params;
            Ok(on_document(ws, &position.text_document.uri, |document| {
                hover::hover(ws, document, position.position)
            }))
        })
    })
    .or_else(|| {
        respond::<SignatureHelpRequest>(&request, |params| {
            let position = params.text_document_position_params;
            Ok(on_document(ws, &position.text_document.uri, |document| {
                signature_help::signature_help(ws, document, position.position)
            }))
        })
    })
    .or_else(|| {
        respond::<GotoDefinition>(&request, |params| {
            let position = params.text_document_position_params;
            Ok(on_document(ws, &position.text_document.uri, |document| {
                navigation::goto_definition(ws, document, position.position)
            }))
        })
    })
    .or_else(|| {
        respond::<References>(&request, |params| {
            let position = params.text_document_position;
            let include_declaration = params.context.include_declaration;
            Ok(on_document(ws, &position.text_document.uri, |document| {
                navigation::references(ws, document, position.position, include_declaration)
            }))
        })
    })
    .or_else(|| {
        respond::<DocumentHighlightRequest>(&request, |params| {
            let position = params.text_document_position_params;
            Ok(on_document(ws, &position.text_document.uri, |document| {
                navigation::document_highlight(ws, document, position.position)
            }))
        })
    })
    .or_else(|| {
        respond::<PrepareRenameRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                navigation::prepare_rename(ws, document, params.position)
            }))
        })
    })
    .or_else(|| {
        respond::<Rename>(&request, |params| {
            let position = params.text_document_position;
            match ws.document(&position.text_document.uri) {
                Some(document) => {
                    navigation::rename(ws, document, position.position, &params.new_name)
                }
                None => Ok(None),
            }
        })
    })
    .or_else(|| {
        respond::<DocumentSymbolRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                symbols::document_symbols(ws, document)
            }))
        })
    })
    .or_else(|| {
        respond::<WorkspaceSymbolRequest>(&request, |params| {
            Ok(symbols::workspace_symbols(ws, &params.query))
        })
    })
    .or_else(|| {
        respond::<SemanticTokensFullRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                semantic_tokens::semantic_tokens(ws, document)
            }))
        })
    })
    .or_else(|| {
        respond::<FoldingRangeRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                folding::folding_ranges(ws, document)
            }))
        })
    })
    .or_else(|| {
        respond::<InlayHintRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                inlay_hints::inlay_hints(ws, document, params.range)
            }))
        })
    })
    .or_else(|| {
        respond::<CodeActionRequest>(&request, |params| {
            Ok(on_document(ws, &params.text_document.uri, |document| {
                code_actions::code_actions(ws, document, &params)
            }))
        })
    })
    .unwrap_or_else(|| {
        Response::new_err(
            request.id.clone(),
            ErrorCode::MethodNotFound as i32,
            format!("unsupported request {}", request.method),
        )
    })
}

/// Runs a feature on an open document; unknown documents yield `None`.
///
/// # Arguments
/// * `workspace` - server state
/// * `uri` - document URI
/// * `feature` - feature to run
fn on_document<T>(
    workspace: &Workspace,
    uri: &Uri,
    feature: impl FnOnce(&Document) -> Option<T>,
) -> Option<T> {
    workspace.document(uri).and_then(feature)
}

/// Answers `request` with `handler` if its method matches `R`; otherwise returns `None`.
///
/// # Arguments
/// * `request` - incoming request
/// * `handler` - feature call; `Err` becomes a `RequestFailed` error response
fn respond<R>(
    request: &Request,
    handler: impl FnOnce(R::Params) -> Result<R::Result, String>,
) -> Option<Response>
where
    R: LspRequest,
    R::Params: DeserializeOwned,
    R::Result: Serialize,
{
    if request.method != R::METHOD {
        return None;
    }
    let response = match serde_json::from_value::<R::Params>(request.params.clone()) {
        Ok(params) => match handler(params) {
            Ok(result) => Response::new_ok(request.id.clone(), result),
            Err(message) => {
                Response::new_err(request.id.clone(), ErrorCode::RequestFailed as i32, message)
            }
        },
        Err(error) => Response::new_err(
            request.id.clone(),
            ErrorCode::InvalidParams as i32,
            error.to_string(),
        ),
    };
    Some(response)
}

/// Applies a notification to the workspace.
///
/// # Arguments
/// * `workspace` - server state
/// * `notification` - incoming notification
///
/// # Returns
/// URIs whose diagnostics must be republished.
pub fn handle_notification(workspace: &mut Workspace, notification: Notification) -> Vec<Uri> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => parse::<DidOpenTextDocument>(notification)
            .map(|params| {
                workspace.update(
                    params.text_document.uri,
                    params.text_document.version,
                    params.text_document.text,
                )
            })
            .unwrap_or_default(),
        DidChangeTextDocument::METHOD => parse::<DidChangeTextDocument>(notification)
            .and_then(|params| {
                let text = params.content_changes.into_iter().last()?.text;
                Some(workspace.update(params.text_document.uri, params.text_document.version, text))
            })
            .unwrap_or_default(),
        DidSaveTextDocument::METHOD => parse::<DidSaveTextDocument>(notification)
            .map(|params| workspace.saved(&params.text_document.uri))
            .unwrap_or_default(),
        DidCloseTextDocument::METHOD => parse::<DidCloseTextDocument>(notification)
            .map(|params| {
                let uri = params.text_document.uri;
                std::iter::once(uri.clone())
                    .chain(workspace.close(&uri))
                    .collect()
            })
            .unwrap_or_default(),
        DidChangeConfiguration::METHOD => parse::<DidChangeConfiguration>(notification)
            .map(|params| workspace.set_settings(Settings::from_json(&params.settings)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Deserializes notification parameters, logging malformed input to stderr.
///
/// # Arguments
/// * `notification` - incoming notification
fn parse<N>(notification: Notification) -> Option<N::Params>
where
    N: LspNotification,
    N::Params: DeserializeOwned,
{
    serde_json::from_value(notification.params)
        .map_err(|error| eprintln!("hspice-lsp: invalid {} params: {error}", N::METHOD))
        .ok()
}
