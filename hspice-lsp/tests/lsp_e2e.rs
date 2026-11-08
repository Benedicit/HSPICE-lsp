//! End-to-end test: spawns the language server binary and talks JSON-RPC over stdio.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(20);

/// A minimal LSP client driving the server process.
struct Client {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    notifications: Vec<Value>,
    next_id: i64,
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads one framed JSON-RPC message from a stream.
fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0u8; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

impl Client {
    /// Starts the server binary.
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_hspice-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn server");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (sender, messages) = channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Client {
            child,
            stdin,
            messages,
            notifications: Vec::new(),
            next_id: 1,
        }
    }

    /// Sends a framed message.
    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("write");
        self.stdin.flush().expect("flush");
    }

    /// Receives the next message or fails after the timeout.
    fn receive(&mut self, what: &str) -> Value {
        self.messages
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
    }

    /// Sends a request and returns the full response, stashing notifications.
    fn request_raw(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let message = self.receive(method);
            if message.get("id") == Some(&json!(id)) && message.get("method").is_none() {
                return message;
            }
            if message.get("method").is_some() && message.get("id").is_none() {
                self.notifications.push(message);
            }
        }
    }

    /// Sends a request and returns its `result`, failing on an error response.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let response = self.request_raw(method, params);
        assert!(
            response.get("error").is_none(),
            "{method} failed: {response}"
        );
        response["result"].clone()
    }

    /// Sends a notification.
    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Waits for the next `publishDiagnostics` of a document.
    fn diagnostics_for(&mut self, uri: &str) -> Vec<Value> {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(index) = self.notifications.iter().position(|n| {
                n["method"] == "textDocument/publishDiagnostics" && n["params"]["uri"] == uri
            }) {
                let notification = self.notifications.remove(index);
                return notification["params"]["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
            }
            assert!(Instant::now() < deadline, "no diagnostics for {uri}");
            let message = self.receive("publishDiagnostics");
            self.notifications.push(message);
        }
    }

    /// Performs the initialize handshake and returns the result.
    fn initialize(&mut self, root: &Path, encodings: &[&str]) -> Value {
        let result = self.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": file_uri(root),
                "capabilities": {"general": {"positionEncodings": encodings}},
            }),
        );
        self.notify("initialized", json!({}));
        result
    }

    /// Opens a document.
    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "hspice", "version": 1, "text": text}}),
        );
    }

    /// Replaces the full text of a document.
    fn change(&mut self, uri: &str, version: i64, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]}),
        );
    }

    /// Shuts the server down and checks the exit code.
    fn shutdown(&mut self) {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match self.child.try_wait().expect("wait") {
                Some(status) => {
                    assert!(status.success(), "server exit status {status}");
                    return;
                }
                None => {
                    assert!(Instant::now() < deadline, "server did not exit");
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

/// Builds a `file:` URI from a path.
fn file_uri(path: &Path) -> String {
    let encoded: String = path
        .to_string_lossy()
        .bytes()
        .map(|byte| match byte {
            b'/' | b'-' | b'_' | b'.' | b'~' => char::from(byte).to_string(),
            b if b.is_ascii_alphanumeric() => char::from(b).to_string(),
            b => format!("%{b:02X}"),
        })
        .collect();
    format!("file://{encoded}")
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Returns the LSP position of the `nth` occurrence of `needle` plus `delta` columns.
fn position(text: &str, needle: &str, nth: usize, delta: usize, utf8: bool) -> Value {
    let index = text
        .match_indices(needle)
        .nth(nth)
        .map(|(index, _)| index)
        .unwrap_or_else(|| panic!("`{needle}` not found"));
    let line = text[..index].matches('\n').count();
    let line_start = text[..index].rfind('\n').map_or(0, |newline| newline + 1);
    let prefix = &text[line_start..index];
    let column = if utf8 {
        prefix.len()
    } else {
        prefix.encode_utf16().count()
    };
    json!({"line": line, "character": column + delta})
}

fn doc(uri: &str) -> Value {
    json!({"uri": uri})
}

fn location_uris(result: &Value) -> Vec<String> {
    let items: Vec<&Value> = match result {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![result],
        _ => Vec::new(),
    };
    items
        .iter()
        .filter_map(|item| item["uri"].as_str().or_else(|| item["targetUri"].as_str()))
        .map(str::to_string)
        .collect()
}

#[test]
fn full_session_utf16() {
    let mut client = Client::start();
    let init = client.initialize(&fixtures(), &["utf-16"]);
    assert_eq!(init["capabilities"]["positionEncoding"], "utf-16");
    for capability in [
        "definitionProvider",
        "referencesProvider",
        "renameProvider",
        "hoverProvider",
        "codeActionProvider",
    ] {
        assert!(
            !init["capabilities"][capability].is_null(),
            "{capability} missing"
        );
    }

    let path = fixtures().join("inverter_tb.sp");
    let text = std::fs::read_to_string(&path).expect("fixture");
    let uri = file_uri(&path);
    client.open(&uri, &text);
    let diagnostics = client.diagnostics_for(&uri);
    let errors: Vec<&Value> = diagnostics.iter().filter(|d| d["severity"] == 1).collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");

    // completion after typing `.tr`
    let typed = text.replace("\n.end", "\n.tr\n.end");
    client.change(&uri, 2, &typed);
    let completion = client.request(
        "textDocument/completion",
        json!({"textDocument": doc(&uri), "position": position(&typed, ".tr\n.end", 0, 3, false)}),
    );
    let items = completion["items"]
        .as_array()
        .or_else(|| completion.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        items.iter().any(|item| item["label"]
            .as_str()
            .is_some_and(|label| label.trim_start_matches('.') == "TRAN")),
        "no .TRAN completion in {} items",
        items.len()
    );
    client.change(&uri, 3, &text);

    // hover
    let wn = position(&text, "wn=200n", 0, 1, false);
    let hover = client.request(
        "textDocument/hover",
        json!({"textDocument": doc(&uri), "position": wn}),
    );
    assert!(!hover.is_null(), "hover on wn is null");
    let contents = hover["contents"].to_string();
    assert!(contents.len() > 4, "hover contents empty: {contents}");

    // definition of the subcircuit in the included file
    let definition = client.request(
        "textDocument/definition",
        json!({"textDocument": doc(&uri), "position": position(&text, " inv wn", 0, 2, false)}),
    );
    assert!(
        location_uris(&definition)
            .iter()
            .any(|u| u.ends_with("cells/inv.inc")),
        "definition: {definition}"
    );

    // references and rename of wn
    let reference_position = position(&text, "wn=wn", 0, 4, false);
    let references = client.request(
        "textDocument/references",
        json!({"textDocument": doc(&uri), "position": reference_position, "context": {"includeDeclaration": true}}),
    );
    assert!(
        references.as_array().is_some_and(|found| found.len() >= 3),
        "references: {references}"
    );
    let prepared = client.request(
        "textDocument/prepareRename",
        json!({"textDocument": doc(&uri), "position": reference_position}),
    );
    assert_eq!(prepared["placeholder"], "wn", "prepareRename: {prepared}");
    let renamed = client.request(
        "textDocument/rename",
        json!({"textDocument": doc(&uri), "position": reference_position, "newName": "wid"}),
    );
    let edits = renamed["changes"][&uri]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(edits.len() >= 3, "rename: {renamed}");
    assert!(edits.iter().all(|edit| edit["newText"] == "wid"));
    let bad = client.request_raw(
        "textDocument/rename",
        json!({"textDocument": doc(&uri), "position": reference_position, "newName": "1bad"}),
    );
    assert!(bad.get("error").is_some(), "invalid name must fail: {bad}");

    // outline, tokens, hints
    let symbols = client.request(
        "textDocument/documentSymbol",
        json!({"textDocument": doc(&uri)}),
    );
    assert!(
        symbols.as_array().is_some_and(|items| !items.is_empty()),
        "documentSymbol: {symbols}"
    );
    let tokens = client.request(
        "textDocument/semanticTokens/full",
        json!({"textDocument": doc(&uri)}),
    );
    assert!(
        tokens["data"]
            .as_array()
            .is_some_and(|data| !data.is_empty()),
        "semanticTokens: {tokens}"
    );
    let hints = client.request(
        "textDocument/inlayHint",
        json!({"textDocument": doc(&uri), "range": {"start": {"line": 0, "character": 0}, "end": {"line": 100, "character": 0}}}),
    );
    assert!(
        hints.as_array().is_some_and(|items| !items.is_empty()),
        "inlayHint: {hints}"
    );

    // folding on a file with blocks
    let ring_path = fixtures().join("ring_osc.sp");
    let ring_uri = file_uri(&ring_path);
    client.open(
        &ring_uri,
        &std::fs::read_to_string(&ring_path).expect("fixture"),
    );
    let folding = client.request(
        "textDocument/foldingRange",
        json!({"textDocument": doc(&ring_uri)}),
    );
    assert!(
        folding.as_array().is_some_and(|items| !items.is_empty()),
        "foldingRange: {folding}"
    );

    // quick fix for a meg-suffix diagnostic
    let meg_uri = "file:///tmp/hspice_e2e_meg.sp";
    client.open(meg_uri, "title\nR1 a 0 10M\nV1 a 0 1\n.end\n");
    let meg = client.diagnostics_for(meg_uri);
    let diagnostic = meg
        .iter()
        .find(|d| d["code"] == "meg-suffix")
        .unwrap_or_else(|| panic!("no meg-suffix diagnostic in {meg:?}"));
    let actions = client.request(
        "textDocument/codeAction",
        json!({"textDocument": doc(meg_uri), "range": diagnostic["range"], "context": {"diagnostics": [diagnostic]}}),
    );
    let action = actions
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|a| a["title"].as_str().is_some_and(|t| t.contains("MEG")))
        })
        .unwrap_or_else(|| panic!("no MEG quick fix in {actions}"));
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(action["edit"]["changes"][meg_uri][0]["newText"], "MEG");

    client.shutdown();
}

#[test]
fn utf8_session_uses_byte_columns() {
    let mut client = Client::start();
    let init = client.initialize(&fixtures(), &["utf-8", "utf-16"]);
    assert_eq!(init["capabilities"]["positionEncoding"], "utf-8");
    let uri = "file:///tmp/hspice_e2e_utf8.sp";
    let text = "title\n* \u{b5}\u{3a9} note\n.param wn=1u\nR1 a 0 'wn'\n.end\n";
    client.open(uri, text);
    let _ = client.diagnostics_for(uri);
    let usage = position(text, "'wn'", 0, 1, true);
    let definition = client.request(
        "textDocument/definition",
        json!({"textDocument": doc(uri), "position": usage}),
    );
    let start = &definition["range"]["start"];
    assert_eq!(start["line"], 2, "definition: {definition}");
    assert_eq!(start["character"], 7);
    client.shutdown();
}
