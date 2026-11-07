vim.bo.commentstring = "* %s"
pcall(vim.treesitter.start, 0, "hspice")

vim.b.undo_ftplugin = (vim.b.undo_ftplugin and vim.b.undo_ftplugin .. "|" or "") .. "setlocal commentstring<"
