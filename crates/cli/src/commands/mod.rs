//! CLI commands

use tokio::process::Command;

pub mod build;
pub mod dev;
pub mod init;
pub mod prebuild;

pub(super) fn bun_command() -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/D", "/S", "/C", "bun"]);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("bun")
    }
}
