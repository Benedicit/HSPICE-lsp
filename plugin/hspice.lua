-- HSPICE support: tree-sitter language registration and .inc/.pm filetype detection.
if vim.g.loaded_hspice_plugin then
  return
end
vim.g.loaded_hspice_plugin = true

local SCAN_LINE_COUNT = 100

-- Dot commands that mark a file as an HSPICE netlist (C/Pascal/Perl files never start lines with these).
local SPICE_COMMANDS = {
  subckt = true, macro = true, ends = true, eom = true, param = true, parameter = true,
  parameters = true, model = true, include = true, inc = true, incl = true, lib = true,
  endl = true, global = true, option = true, options = true, opt = true, tran = true,
  ac = true, dc = true, op = true, ["end"] = true, data = true, measure = true, meas = true,
  print = true, probe = true, temp = true, ic = true, nodeset = true, hdl = true, prot = true,
  protect = true, unprot = true, unprotect = true, title = true, alter = true,
}

vim.treesitter.language.register("hspice", "spice")

---Check whether a line starts with a known HSPICE dot command.
---@param line string Buffer line.
---@return boolean looks_like_spice True if the line starts with e.g. `.subckt` or `.MODEL`.
local function is_spice_line(line)
  local command = line:match("^%s*%.(%a+)")
  return command ~= nil and SPICE_COMMANDS[command:lower()] == true
end

---Check whether the buffer's first lines contain SPICE syntax.
---@param bufnr integer Buffer number.
---@return boolean looks_like_spice True if any scanned line looks like SPICE.
local function buffer_looks_like_spice(bufnr)
  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, SCAN_LINE_COUNT, false)
  return vim.iter(lines):any(is_spice_line)
end

---Build a filetype detection function for an extension.
---A function-valued rule replaces the built-in one, so the fallback is called explicitly.
---@param fallback fun(path: string, bufnr: integer): string? Built-in detection for the extension.
---@return fun(path: string, bufnr: integer): string? detect
local function make_detector(fallback)
  return function(path, bufnr)
    if bufnr < 1 then
      return nil
    end
    if buffer_looks_like_spice(bufnr) then
      return "spice"
    end
    return fallback(path, bufnr)
  end
end

local detect = require("vim.filetype.detect")

vim.filetype.add({
  extension = {
    inc = make_detector(detect.inc),
    pm = make_detector(detect.pm),
  },
})
