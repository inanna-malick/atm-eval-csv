use clap::Parser;
use payments_engine::{process_csv_file, write_accounts_csv};
use std::process;

/// A high-performance payments processing engine
#[derive(Parser, Debug)]
#[command(name = "payments-engine")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input CSV file containing transactions
    #[arg(value_name = "FILE")]
    input: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = run(args).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Process the CSV file
    let engine = process_csv_file(&args.input).await?;

    // Get accounts and write to stdout
    let accounts = engine.get_accounts();
    write_accounts_csv(&accounts).await?;

    Ok(())
}
