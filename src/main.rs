use anyhow::Result;
use clap::Parser;

use depot::cli::{Cli, Command, HubAction, ScanArgs};
use depot::{hub, report, run, tui};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Scan { args, json }) => {
            let items = report::collect(args.to_options());
            if json {
                report::print_json(&items);
            } else {
                report::print_grouped(&items);
            }
            Ok(())
        }
        Some(Command::Tui(args)) => tui::run(args.to_options()),
        Some(Command::Run { args }) => run::run(args),
        Some(Command::Ensure) => run::ensure(),
        Some(Command::Link) => hub::run(HubAction::Link),
        Some(Command::Hub { action }) => hub::run(action),
        // No subcommand → launch the TUI with default options.
        None => tui::run(ScanArgs::default().to_options()),
    }
}
