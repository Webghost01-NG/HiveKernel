mod display;
mod simulation;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hive-cli")]
#[command(about = "HiveKernel: Autonomous P2P Agent Subcontracting & Optimistic Settlement Engine in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the full multi-agent swarm simulation (RFQ -> Auction -> Receipt -> Optimistic Settlement)
    RunSwarm {
        /// Description or title of the task to outsource
        #[arg(short, long, default_value = "Solidity Smart Contract Security & Gas Optimization Audit")]
        task: String,

        /// Bounty amount in USDC
        #[arg(short, long, default_value_t = 150)]
        bounty: u64,
    },

    /// Simulate an adversarial dispute scenario with malicious worker output
    SimulateDispute {
        #[arg(short, long, default_value_t = 200)]
        bounty: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::RunSwarm { task, bounty } => {
            simulation::run_swarm_simulation(&task, bounty, false).await?;
        }
        Commands::SimulateDispute { bounty } => {
            simulation::run_swarm_simulation("Malicious Inject Task Simulation", bounty, true).await?;
        }
    }

    Ok(())
}
