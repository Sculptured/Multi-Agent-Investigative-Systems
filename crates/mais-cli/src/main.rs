use clap::Parser;

/// Report where a multi-agent run's tokens and context went.
#[derive(Parser)]
#[command(name = "mais", version)]
struct Cli {}

fn main() {
    let _ = Cli::parse();
}
