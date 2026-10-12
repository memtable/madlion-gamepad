use clap::Parser;

mod cli;
mod config;
mod gamepad;
mod ipc;
mod keyboard;
mod ui;

use cli::benchmark::cmd_benchmark;
use cli::daemon::cmd_run;
use cli::inspect::cmd_inspect;
use cli::profile::cmd_profile;
use cli::{Cli, Commands, cmd_list, cmd_status, cmd_toggle_gamepad, cmd_toggle_override};
use ui::run_gui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Gui) {
        Commands::Gui => {
            run_gui();
        }
        Commands::List => cmd_list()?,
        Commands::Profile(args) => cmd_profile(args)?,
        Commands::Inspect(args) => cmd_inspect(args)?,
        Commands::Run(args) => cmd_run(args)?,
        Commands::Benchmark(args) => cmd_benchmark(args)?,
        Commands::ToggleOverride => cmd_toggle_override()?,
        Commands::ToggleGamepad => cmd_toggle_gamepad()?,
        Commands::Status => cmd_status()?,
    }

    Ok(())
}
