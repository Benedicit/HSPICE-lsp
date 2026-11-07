import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

const SCAN_LINE_COUNT = 100;

/** Dot commands that mark a `.inc`/`.pm` file as an HSPICE netlist (same list as plugin/hspice.lua). */
const SPICE_COMMANDS = new Set([
  "subckt", "macro", "ends", "eom", "param", "parameter", "parameters", "model", "include", "inc", "incl",
  "lib", "endl", "global", "option", "options", "opt", "tran", "ac", "dc", "op", "end", "data", "measure",
  "meas", "print", "probe", "temp", "ic", "nodeset", "hdl", "prot", "protect", "unprot", "unprotect",
  "title", "alter",
]);

let client: LanguageClient | undefined;
const checked = new Set<string>();

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const outputChannel = vscode.window.createOutputChannel("HSPICE", { log: true });
  const restart = async () => {
    if (client?.isRunning()) {
      await client.stop();
    }
    await startClient(context, outputChannel);
  };
  context.subscriptions.push(
    outputChannel,
    vscode.workspace.onDidOpenTextDocument(detectLanguage),
    vscode.commands.registerCommand("hspice.restartServer", restart),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("hspice.server.path")) {
        void restart();
      }
    }),
  );
  vscode.workspace.textDocuments.forEach(detectLanguage);
  await startClient(context, outputChannel).catch((error) => outputChannel.error(String(error)));
}

export function deactivate(): Thenable<void> | undefined {
  return client?.isRunning() ? client.stop() : undefined;
}

/** Starts a new language client; settings are sent on start and on every change. */
async function startClient(context: vscode.ExtensionContext, outputChannel: vscode.LogOutputChannel): Promise<void> {
  client = new LanguageClient(
    "hspice",
    "HSPICE",
    { command: serverCommand(context) },
    {
      documentSelector: [
        { scheme: "file", language: "hspice" },
        { scheme: "untitled", language: "hspice" },
      ],
      initializationOptions: () => JSON.parse(JSON.stringify(vscode.workspace.getConfiguration("hspice"))),
      synchronize: { configurationSection: "hspice" },
      outputChannel,
    },
  );
  await client.start();
}

/** Returns `hspice.server.path`, else the bundled binary, else `hspice-lsp` from PATH. */
function serverCommand(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration("hspice").get<string>("server.path");
  if (configured) {
    return configured;
  }
  const binary = process.platform === "win32" ? "hspice-lsp.exe" : "hspice-lsp";
  const bundled = context.asAbsolutePath(path.join("server", binary));
  return fs.existsSync(bundled) ? bundled : "hspice-lsp";
}

/** Switches a `.inc`/`.pm` file to HSPICE once if one of its first lines is an HSPICE dot command. */
function detectLanguage(document: vscode.TextDocument): void {
  const key = document.uri.toString();
  if (!/\.(inc|pm)$/i.test(document.fileName) || checked.has(key)) {
    return;
  }
  checked.add(key);
  const lineCount = Math.min(document.lineCount, SCAN_LINE_COUNT);
  for (let line = 0; line < lineCount; line++) {
    const command = /^\s*\.([a-z]+)/i.exec(document.lineAt(line).text)?.[1];
    if (command && SPICE_COMMANDS.has(command.toLowerCase())) {
      if (document.languageId !== "hspice") {
        void vscode.languages.setTextDocumentLanguage(document, "hspice");
      }
      return;
    }
  }
}
