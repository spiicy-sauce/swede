//! Zed extension for Swede: highlighting comes from the bundled queries; this
//! code only locates and launches the `swede-lsp` language server on PATH.

use zed_extension_api::{self as zed, LanguageServerId, Result};

struct SwedeExtension;

impl zed::Extension for SwedeExtension {
    fn new() -> Self {
        SwedeExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree.which("swede-lsp").ok_or_else(|| {
            "`swede-lsp` was not found in PATH. Install it with \
             `cargo install --path crates/swede-lsp`."
                .to_string()
        })?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(SwedeExtension);
