local M = {}

---@class hspice.Settings
---@field includePaths? string[] Extra directories searched for `.include`/`.lib` files.
---@field diagnostics? { floatingNodes?: boolean, unusedParams?: boolean, unknownOptions?: boolean, megSuffix?: boolean }
---@field inlayHints? { ports?: boolean, sourceArgs?: boolean }
---@field workspaceScan? { enabled?: boolean, maxFiles?: integer }
---@field folding? { comments?: boolean }
---@field maxIncludeFileSizeMb? integer

---@class hspice.Options
---@field settings hspice.Settings Server settings.
---@field inlay_hints boolean Enable inlay hints on attach.
---@field lsp_folding boolean Use LSP folding ranges on attach.
---@field cmd? string[] Override the server command.

---@type hspice.Options
M.defaults = {
  settings = {
    includePaths = {},
    diagnostics = { floatingNodes = true, unusedParams = true, unknownOptions = true, megSuffix = true },
    inlayHints = { ports = true, sourceArgs = true },
    workspaceScan = { enabled = true, maxFiles = 500 },
    folding = { comments = false },
    maxIncludeFileSizeMb = 64,
  },
  inlay_hints = true,
  lsp_folding = true,
}

---@type hspice.Options
M.config = vim.deepcopy(M.defaults)

---Apply buffer and window options for a freshly attached hspice client.
---@param bufnr integer Buffer number.
local function configure_buffer(bufnr)
  if M.config.inlay_hints then
    vim.lsp.inlay_hint.enable(true, { bufnr = bufnr })
  end
  if M.config.lsp_folding then
    vim.api.nvim_buf_call(bufnr, function()
      vim.wo[0][0].foldmethod = "expr"
      vim.wo[0][0].foldexpr = "v:lua.vim.lsp.foldexpr()"
    end)
  end
end

---Configure and enable the hspice language server.
---@param opts? hspice.Options|table User options merged over the defaults.
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", M.defaults, opts or {})

  local server_config = {
    init_options = M.config.settings,
    settings = { hspice = M.config.settings },
  }
  if M.config.cmd then
    server_config.cmd = M.config.cmd
  end
  vim.lsp.config("hspice", server_config)
  vim.lsp.enable("hspice")

  vim.api.nvim_create_autocmd("LspAttach", {
    group = vim.api.nvim_create_augroup("hspice_lsp_attach", { clear = true }),
    desc = "Configure hspice buffers",
    callback = function(ev)
      local client = vim.lsp.get_client_by_id(ev.data.client_id)
      if client and client.name == "hspice" then
        configure_buffer(ev.buf)
      end
    end,
  })
end

return M
