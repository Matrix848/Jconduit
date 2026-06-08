use anyhow::Result;
use cargo_jconduit::{Cli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    run_cli(cli)
}
