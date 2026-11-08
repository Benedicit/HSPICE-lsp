-- Headless end-to-end test for the hspice plugin and server.
-- Usage (from the repo root):
--   XDG_STATE_HOME=$TMPDIR XDG_DATA_HOME=$TMPDIR XDG_CACHE_HOME=$TMPDIR \
--     nvim --headless --clean -u NONE -l scripts/nvim-smoke.lua [fixture]
local TIMEOUT_MS = 10000
local REQUEST_TIMEOUT_MS = 5000
local DEFAULT_FIXTURE = "hspice-lsp/tests/fixtures/inverter_tb.sp"

local failures = 0

---Print a PASS/FAIL line and count failures.
---@param ok boolean Whether the check passed.
---@param message string Check description.
local function report(ok, message)
  print((ok and "PASS: " or "FAIL: ") .. message)
  if not ok then
    failures = failures + 1
  end
end

---Return the repo root, derived from this script's path.
---@return string root Absolute repo root.
local function repo_root()
  local source = debug.getinfo(1, "S").source:sub(2)
  return vim.fs.dirname(vim.fs.dirname(vim.fn.fnamemodify(source, ":p")))
end

---Find the first buffer line matching a Lua pattern.
---@param bufnr integer Buffer number.
---@param pattern string Lua pattern.
---@return integer? row Zero-based line index.
---@return string? line Line text.
local function find_line(bufnr, pattern)
  for row, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
    if line:match(pattern) then
      return row - 1, line
    end
  end
end

---Build an LSP position from a zero-based row and byte column.
---@param line string Line text.
---@param row integer Zero-based row.
---@param byte_col integer Zero-based byte column.
---@param encoding string Client position encoding.
---@return table position LSP Position.
local function make_position(line, row, byte_col, encoding)
  return { line = row, character = vim.str_utfindex(line, encoding, byte_col, false) }
end

---Send a synchronous request and return its result.
---@param client vim.lsp.Client LSP client.
---@param bufnr integer Buffer number.
---@param method string LSP method.
---@param params table Request params.
---@return any? result Result payload, nil on error.
local function request(client, bufnr, method, params)
  local response = client:request_sync(method, params, REQUEST_TIMEOUT_MS, bufnr)
  if not response or response.err then
    return nil
  end
  return response.result
end

---Return the first location URI from a definition result.
---@param result any Result of textDocument/definition.
---@return string? uri Target URI.
local function first_uri(result)
  local first = result and (result.uri and result or result[1])
  return first and (first.uri or first.targetUri)
end

---Flatten hover contents to a string.
---@param contents any LSP hover contents.
---@return string text Plain text.
local function hover_text(contents)
  if type(contents) == "string" then
    return contents
  end
  if type(contents) ~= "table" then
    return ""
  end
  if contents.value then
    return contents.value
  end
  return table.concat(vim.tbl_map(hover_text, contents), "\n")
end

local root = repo_root()
vim.opt.runtimepath:prepend(root)
vim.cmd.source(vim.fs.joinpath(root, "plugin/hspice.lua"))
require("hspice").setup({})

local diagnostics_published = false
local original_publish = vim.lsp.handlers["textDocument/publishDiagnostics"]
vim.lsp.handlers["textDocument/publishDiagnostics"] = function(...)
  diagnostics_published = true
  return original_publish(...)
end

local fixture = vim.fn.fnamemodify(arg and arg[1] or DEFAULT_FIXTURE, ":p")
vim.cmd.edit(vim.fn.fnameescape(fixture))
local bufnr = vim.api.nvim_get_current_buf()
vim.cmd("filetype detect")

local attached = vim.wait(TIMEOUT_MS, function()
  return #vim.lsp.get_clients({ bufnr = bufnr, name = "hspice" }) > 0
end, 50)
report(attached, "hspice client attached")
if not attached then
  os.exit(1)
end
local client = vim.lsp.get_clients({ bufnr = bufnr, name = "hspice" })[1]
local encoding = client.offset_encoding

local published = vim.wait(TIMEOUT_MS, function()
  return diagnostics_published
end, 50)
report(published, "diagnostics published")

report(vim.treesitter.highlighter.active[bufnr] ~= nil, "tree-sitter parser hspice active")

local errors = vim.diagnostic.get(bufnr, { severity = vim.diagnostic.severity.ERROR })
for _, diagnostic in ipairs(errors) do
  print(("  error %d:%d %s"):format(diagnostic.lnum + 1, diagnostic.col + 1, diagnostic.message))
end
report(#errors == 0, "no error diagnostics in clean fixture")

-- Completion on a temporary `.tr` line inserted before the last line (`.end`); a line after
-- `.end` would be the title of a new deck, where HSPICE ignores everything.
local line_count = vim.api.nvim_buf_line_count(bufnr) - 1
vim.api.nvim_buf_set_lines(bufnr, line_count, line_count, false, { ".tr" })
local completion = request(client, bufnr, "textDocument/completion", {
  textDocument = { uri = vim.uri_from_bufnr(bufnr) },
  position = make_position(".tr", line_count, 3, encoding),
})
local items = completion and (completion.items or completion) or {}
local has_tran = vim.iter(items):any(function(item)
  return item.label == ".TRAN"
end)
report(has_tran, "completion offers .TRAN")
vim.api.nvim_buf_set_lines(bufnr, line_count, line_count + 1, false, {})

-- Hover on the first .param name.
local param_row, param_line = find_line(bufnr, "^%s*%.[Pp][Aa][Rr][Aa][Mm]%s+%S")
if param_row and param_line then
  local name_start = param_line:match("^%s*%S+%s+()")
  local hover = request(client, bufnr, "textDocument/hover", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = make_position(param_line, param_row, name_start - 1, encoding),
  })
  report(hover ~= nil and vim.trim(hover_text(hover.contents)) ~= "", "hover on first .param is non-empty")
else
  report(false, "fixture has a .param line")
end

-- Definition on the subckt name of the first X instance.
local x_row, x_line = find_line(bufnr, "^%s*[Xx]")
if x_row and x_line then
  local subckt_start
  for start, word in x_line:gmatch("()(%S+)") do
    if word:find("=", 1, true) then
      break
    end
    subckt_start = start
  end
  local definition = request(client, bufnr, "textDocument/definition", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = make_position(x_line, x_row, subckt_start - 1, encoding),
  })
  local uri = first_uri(definition)
  report(uri ~= nil and uri:match("inv%.inc$") ~= nil, "definition of subckt resolves to inv.inc")
else
  report(false, "fixture has an X instance line")
end

os.exit(failures > 0 and 1 or 0)
