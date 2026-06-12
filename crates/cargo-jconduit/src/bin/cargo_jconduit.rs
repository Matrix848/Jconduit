use anyhow::Result;
use cargo_jconduit::{Cli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    run_cli(cli)
}
