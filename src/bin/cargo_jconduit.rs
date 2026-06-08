use anyhow::Result;
use cargo_jconduit::{run_cli, Cli};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    run_cli(cli)
}
