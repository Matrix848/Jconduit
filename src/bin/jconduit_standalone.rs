use anyhow::Result;
use cargo_jconduit::{run_cli, Cli};
use clap::Parser;
use std::env;

fn main() -> Result<()> {
    let args_iter = env::args();

    let sanitized_args = args_iter.enumerate().filter_map(|(i, arg)| {
        if i == 1 && arg == "jconduit" {
            None
        } else {
            Some(arg)
        }
    });

    let cli = Cli::parse_from(sanitized_args);
    run_cli(cli)
}
