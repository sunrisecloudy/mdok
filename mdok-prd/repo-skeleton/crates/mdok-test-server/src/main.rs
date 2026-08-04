use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: String,
    #[arg(long, default_value = "127.0.0.1:0")]
    tls_listen: String,
    #[arg(long)]
    json_ready: bool,
}

fn main() -> Result<()> {
    let _ = Args::parse();
    anyhow::bail!("implement endpoint contract from docs/17-fixture-server.md")
}
