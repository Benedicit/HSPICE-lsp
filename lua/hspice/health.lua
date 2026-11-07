local M = {}

---Return the plugin root directory.
---@return string root Absolute plugin root path.
local function plugin_root()
  local source = debug.getinfo(1, "S").source:sub(2)
  return vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(source)))
end

---Report whether the server binary is found and runnable.
local function check_binary()
  local cmd = require("hspice").config.cmd or vim.lsp.config.hspice.cmd
  if type(cmd) ~= "table" then
    vim.health.warn("Server command is not a plain command list; skipping binary check")
    return
  end
  local executable = vim.fn.exepath(cmd[1])
  if executable == "" then
    vim.health.error(
      ("Server binary not found: %s"):format(cmd[1]),
      { "Run scripts/build-local.sh, or install a prebuilt package (see README)" }
    )
    return
  end
  vim.health.ok(("Server binary: %s"):format(executable))
  local ok, result = pcall(function()
    return vim.system({ executable, "--version" }, { text = true }):wait(5000)
  end)
  if ok and result.code == 0 then
    vim.health.ok(vim.trim(result.stdout))
  else
    vim.health.error("`hspice-lsp --version` failed", { ok and vim.trim(result.stderr or "") or tostring(result) })
  end
end

---Report whether the tree-sitter parser loads.
local function check_parser()
  local ok, err = pcall(vim.treesitter.language.add, "hspice")
  if ok and err ~= false then
    vim.health.ok("tree-sitter parser `hspice` loaded")
  else
    vim.health.error("tree-sitter parser `hspice` not loadable", { "Expected parser/hspice.so (hspice.dll on Windows) in the plugin", tostring(err) })
  end
end

---Report whether the tree-sitter queries are present.
local function check_queries()
  local files = vim.api.nvim_get_runtime_file("queries/hspice/*.scm", true)
  if #files > 0 then
    vim.health.ok(("%d query file(s) found"):format(#files))
  else
    vim.health.error("No queries/hspice/*.scm found on runtimepath")
  end
end

---Report filetype and parser registration.
local function check_filetype()
  local lang = vim.treesitter.language.get_lang("spice")
  if lang == "hspice" then
    vim.health.ok("filetype `spice` uses parser `hspice`")
  else
    vim.health.error(("filetype `spice` maps to parser `%s`, expected `hspice`"):format(tostring(lang)))
  end
  if vim.g.loaded_hspice_plugin then
    vim.health.ok("plugin/hspice.lua loaded (.inc/.pm detection active)")
  else
    vim.health.error("plugin/hspice.lua not loaded")
  end
end

---Run all checks for `:checkhealth hspice`.
function M.check()
  vim.health.start("hspice")
  vim.health.info(("Plugin root: %s"):format(plugin_root()))
  if vim.fn.has("nvim-0.11") == 1 then
    vim.health.ok("Neovim >= 0.11")
  else
    vim.health.error("Neovim >= 0.11 is required (vim.lsp.config / vim.lsp.enable)")
  end
  check_binary()
  check_parser()
  check_queries()
  check_filetype()
end

return M
