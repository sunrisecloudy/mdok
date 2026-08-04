use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mdok")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Files accepted as an alias for `mdok test`.
    paths: Vec<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    Test { paths: Vec<std::path::PathBuf> },
    Lint { paths: Vec<std::path::PathBuf> },
    Plan { paths: Vec<std::path::PathBuf> },
    List { paths: Vec<std::path::PathBuf> },
    Version,
}

fn main() -> Result<()> {
    let _cli = Cli::parse();
    anyhow::bail!("implementation skeleton: follow docs/15-roadmap-and-acceptance.md")
}
