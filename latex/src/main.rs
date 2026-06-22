use clap::Parser;

mod auth;
mod cli_structure;
mod generated_pdfs;
mod handlers;
mod models;
mod services;

use cli_structure::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Auth check (no-op for now; future: payment/subscription gate).
    auth::check()?;

    match cli.command {
        Command::Convert(args)  => handlers::convert::run(args),
        Command::Validate(args) => handlers::validate::run(args),
    }
}
