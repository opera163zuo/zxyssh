//! zxySSH MCP service CLI.

mod mcp_hostkey;
mod mcp_serve;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zxyssh", author, version)]
#[command(about = "zxySSH Linux MCP service")]
#[command(
    long_about = "zxySSH exposes configured SSH sites through its MCP HTTP service.\n\nThe MCP service owns SSH credentials and sessions; remote Agents connect through `zxyssh mcp serve` and do not need a local SSH client or credential copy."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version information
    Version,
    /// Run or administer the MCP service
    Mcp {
        #[command(subcommand)]
        command: mcp_serve::McpCommand,
    },
    /// Show GPLv3 terms and notices
    License,
}

fn main() -> Result<()> {
    main_entry()
}

pub fn main_entry() -> Result<()> {
    dispatch(Cli::parse().command)
}

fn dispatch(command: Option<Commands>) -> Result<()> {
    match command {
        Some(Commands::Mcp { command }) => mcp_serve::run(command),
        Some(Commands::License) => {
            println!(
                "{}\n{}",
                include_str!("../../NOTICE"),
                include_str!("../../LICENSE")
            );
            Ok(())
        }
        Some(Commands::Version) => {
            println!("zxyssh {}", zxyssh_core::version());
            Ok(())
        }
        None => {
            println!("zxySSH MCP service");
            println!("Run `zxyssh mcp serve` to start the Linux MCP endpoint.");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn exposes_only_mcp_management_commands() {
        assert!(matches!(
            Cli::try_parse_from(["zxyssh", "mcp", "serve"])
                .unwrap()
                .command,
            Some(Commands::Mcp { .. })
        ));
        assert!(Cli::try_parse_from(["zxyssh", "ssh", "server"]).is_err());
        assert!(Cli::try_parse_from(["zxyssh", "plugins", "list"]).is_err());
        assert!(Cli::try_parse_from(["zxyssh", "sftp", "server"]).is_err());
    }
}
