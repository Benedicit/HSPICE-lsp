// Runs test/smoke.js inside a downloaded VS Code (or VSCODE_EXECUTABLE) with the fixtures as workspace.
// Usage (from vscode/): npm test   (needs server/hspice-lsp[.exe] or hspice-lsp on PATH)
const path = require("node:path");
const { runTests } = require("@vscode/test-electron");

const fixtures = path.resolve(__dirname, "../../hspice-lsp/tests/fixtures");

runTests({
  extensionDevelopmentPath: path.resolve(__dirname, ".."),
  extensionTestsPath: path.join(__dirname, "smoke.js"),
  launchArgs: [fixtures, "--disable-extensions", "--disable-workspace-trust"],
  vscodeExecutablePath: process.env.VSCODE_EXECUTABLE,
}).catch((error) => {
  console.error(error);
  process.exit(1);
});
