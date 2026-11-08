// Headless end-to-end test of the extension (the counterpart of scripts/nvim-smoke.lua).
const path = require("node:path");
const vscode = require("vscode");

const TIMEOUT_MS = 20000;
const fixtures = path.resolve(__dirname, "../../hspice-lsp/tests/fixtures");

/** Polls `check` until it returns a truthy value or the timeout expires. */
async function waitFor(message, check) {
  const end = Date.now() + TIMEOUT_MS;
  while (Date.now() < end) {
    const value = await check();
    if (value) {
      console.log(`PASS: ${message}`);
      return value;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`FAIL: ${message}`);
}

/** Opens a fixture in an editor (by URI, since a language switch replaces the document object). */
async function open(relative) {
  const editor = await vscode.window.showTextDocument(vscode.Uri.file(path.join(fixtures, relative)));
  return editor.document;
}

exports.run = async function run() {
  for (const name of ["inverter_tb.sp", "cells/inv.inc", "models/ptm45.pm"]) {
    const { uri } = await open(name);
    await waitFor(`${name} has language hspice`, () =>
      vscode.workspace.textDocuments.some(
        (document) => document.uri.toString() === uri.toString() && document.languageId === "hspice",
      ),
    );
  }

  const errors = await open("errors.sp");
  await waitFor("diagnostics published for errors.sp", () => vscode.languages.getDiagnostics(errors.uri).length > 0);

  const testbench = await open("inverter_tb.sp");
  const text = testbench.getText();
  const paramName = testbench.positionAt(text.search(/^\.param\s/m) + ".param ".length);
  await waitFor("hover on first .param is non-empty", async () => {
    const hovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", testbench.uri, paramName);
    return hovers.some((hover) => hover.contents.length > 0);
  });
  const subcktName = testbench.positionAt(text.indexOf(" inv ", text.search(/^X1 /m)) + 1);
  await waitFor("definition of subckt resolves to inv.inc", async () => {
    const locations = await vscode.commands.executeCommand(
      "vscode.executeDefinitionProvider",
      testbench.uri,
      subcktName,
    );
    return locations.some((location) => (location.targetUri ?? location.uri).path.endsWith("/cells/inv.inc"));
  });
  const errorsInTestbench = vscode.languages
    .getDiagnostics(testbench.uri)
    .filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error);
  if (errorsInTestbench.length > 0) {
    throw new Error(`FAIL: inverter_tb.sp has errors: ${errorsInTestbench.map((d) => d.message).join("; ")}`);
  }

  const untitled = await vscode.workspace.openTextDocument({ language: "hspice", content: "* t\n.tr" });
  await waitFor("completion offers .TRAN", async () => {
    const list = await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      untitled.uri,
      new vscode.Position(1, 3),
    );
    return list.items.some((item) => (item.label.label ?? item.label) === ".TRAN");
  });
};
