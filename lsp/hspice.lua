---Return the plugin root directory derived from this file's location.
---@return string root Absolute path of the plugin root.
local function plugin_root()
  local source = debug.getinfo(1, "S").source:sub(2)
  return vim.fs.dirname(vim.fs.dirname(source))
end

---Return the server command, preferring the bundled binary.
---@return string[] cmd Command line used to start the server.
local function server_cmd()
  local name = vim.fn.has("win32") == 1 and "hspice-lsp.exe" or "hspice-lsp"
  local bundled = vim.fs.joinpath(plugin_root(), "bin", name)
  if vim.fn.executable(bundled) == 1 then
    return { bundled }
  end
  return { "hspice-lsp" }
end

local settings = {}

---@type vim.lsp.Config
return {
  cmd = server_cmd(),
  filetypes = { "spice" },
  root_markers = { ".git" },
  init_options = settings,
  settings = { hspice = settings },
}
