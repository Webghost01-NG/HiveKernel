mod display;
mod live_tcp;
mod simulation;
mod web;

use clap::{Parser, Subcommand};
use colored::*;
use hive_core::auditor::ContractAuditor;
use hive_core::keystore::KeystoreManager;
use hive_core::receipt::AgentKeypair;
use hive_p2p::server::SwarmTcpNode;
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "hive-cli")]
#[command(about = "HiveKernel: Autonomous P2P Agent Subcontracting & Optimistic Settlement Engine in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive Web Mission Control Dashboard UI
    Ui {
        /// Network port to bind web dashboard
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// Run full multi-agent simulation with cryptographic identity binding and optimistic settlement
    RunSwarm {
        /// Optional path to real Solidity/Rust contract file to audit
        #[arg(short, long)]
        file: Option<String>,

        /// Description or title of task
        #[arg(
            short,
            long,
            default_value = "Solidity Smart Contract Security & Gas Audit"
        )]
        task: String,

        /// Bounty amount in USDC
        #[arg(short, long, default_value_t = 150)]
        bounty: u64,
    },

    /// Run live multi-agent execution across real TCP socket ports (19101, 19102)
    LiveSwarm {
        /// Optional path to real contract file
        #[arg(short, long)]
        file: Option<String>,

        /// Bounty amount in USDC
        #[arg(short, long, default_value_t = 200)]
        bounty: u64,
    },

    /// Simulate an adversarial dispute scenario where a rogue worker attempts to forge an audit report
    SimulateDispute {
        /// Optional path to real contract file
        #[arg(short, long)]
        file: Option<String>,

        /// Bounty amount in USDC
        #[arg(short, long, default_value_t = 200)]
        bounty: u64,
    },

    /// Directly audit any Solidity or Rust source file using the HiveKernel analyzer
    Audit {
        /// Path to the source file to audit
        #[arg(short, long)]
        file: String,
    },

    /// Start a standalone P2P TCP Swarm Node daemon
    Daemon {
        /// Node role: worker, validator, or delegator
        #[arg(short, long, default_value = "worker")]
        role: String,

        /// Network port to bind
        #[arg(short, long, default_value_t = 9101)]
        port: u16,
    },

    /// Generate and export a cryptographic Ed25519 keypair with AES-256-GCM encryption
    Keygen {
        /// Agent identity name
        #[arg(short, long, default_value = "agent_node")]
        name: String,

        /// Directory path to save the keystore file
        #[arg(short, long, default_value = "./keystore")]
        out_dir: String,

        /// Optional passphrase for AES-256-GCM encryption
        #[arg(short, long, default_value = "")]
        passphrase: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ui { port } => {
            web::start_web_dashboard(port).await?;
        }
        Commands::RunSwarm { file, task, bounty } => {
            simulation::run_swarm_simulation(&task, file.as_deref(), bounty, false).await?;
        }
        Commands::LiveSwarm { file, bounty } => {
            let (target_name, code) = match file {
                Some(f) => {
                    let content = fs::read_to_string(&f)?;
                    let name = Path::new(&f).file_name().and_then(|n| n.to_str()).unwrap_or("Contract.sol").to_string();
                    (name, content)
                }
                None => (
                    "LiquidityVault.sol".to_string(),
                    "contract LiquidityVault { mapping(address => uint) balances; function withdraw() public { (bool s,) = msg.sender.call{value: balances[msg.sender]}(\"\"); require(s); balances[msg.sender] = 0; } }".to_string(),
                ),
            };
            live_tcp::run_live_tcp_swarm(&target_name, &code, bounty).await?;
        }
        Commands::SimulateDispute { file, bounty } => {
            simulation::run_swarm_simulation(
                "Adversarial Dispute Simulation",
                file.as_deref(),
                bounty,
                true,
            )
            .await?;
        }
        Commands::Audit { file } => {
            display::print_banner();
            let content = fs::read_to_string(&file)?;
            let file_name = Path::new(&file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Contract.sol");

            println!(
                "🔍 {} {}",
                "Auditing Source File:".bright_white().bold(),
                file.bright_cyan()
            );
            let report = ContractAuditor::audit_source(file_name, &content);

            println!(
                "\n{}",
                "────────────────────────────── AUDIT SUMMARY ──────────────────────────────"
                    .yellow()
            );
            println!("  • Target File: {}", report.target_name.bold());
            println!("  • Lines Analyzed: {}", report.total_lines);
            println!(
                "  • Total Vulnerabilities: {}",
                report.total_vulnerabilities.to_string().bright_red()
            );
            println!(
                "  • Security Score: {}/100",
                report.security_score.to_string().bright_green()
            );
            println!(
                "  • Gas Efficiency Grade: {}",
                report.gas_efficiency_grade.bright_yellow()
            );

            if !report.findings.is_empty() {
                println!(
                    "\n{}",
                    "────────────────────────── DETAILED FINDINGS ──────────────────────────".red()
                );
                for (i, finding) in report.findings.iter().enumerate() {
                    println!(
                        "\n[{}] {} - Line {}",
                        i + 1,
                        finding.title.bright_red().bold(),
                        finding.line_number.to_string().bright_yellow()
                    );
                    println!("    Severity: {}", finding.severity.bold());
                    println!("    Snippet:  {}", finding.code_snippet.dimmed());
                    println!("    Action:   {}", finding.recommendation.bright_green());
                }
            }
            println!(
                "\n{}",
                "================================================================================"
                    .yellow()
            );
        }
        Commands::Daemon { role, port } => {
            display::print_banner();
            let bind_addr = format!("127.0.0.1:{}", port);
            println!(
                "🚀 Starting HiveKernel [{}] daemon on {}...",
                role.bright_green().bold(),
                bind_addr.bright_cyan()
            );
            let node = SwarmTcpNode::new(&bind_addr, 100);
            node.start().await.map_err(|e| anyhow::anyhow!("{}", e))?;
            println!(
                "⚡ Node listening for incoming P2P RFQs and Task Receipts. Press Ctrl+C to stop."
            );
            tokio::signal::ctrl_c().await?;
        }
        Commands::Keygen {
            name,
            out_dir,
            passphrase,
        } => {
            display::print_banner();
            let keypair = AgentKeypair::generate();
            let path = if passphrase.is_empty() {
                KeystoreManager::save_keypair(Path::new(&out_dir), &name, &keypair)?
            } else {
                KeystoreManager::save_encrypted_keypair(
                    Path::new(&out_dir),
                    &name,
                    &keypair,
                    &passphrase,
                )?
            };
            println!(
                "✔ Generated Ed25519 Keystore for [{}]",
                name.bright_green().bold()
            );
            println!("  • Public Key: {}", keypair.public_key_hex().bright_cyan());
            println!(
                "  • Encrypted: {}",
                if passphrase.is_empty() {
                    "No (Plaintext JSON with 0600 mode)".yellow()
                } else {
                    "Yes (AES-256-GCM + PBKDF2-SHA256)".bright_green()
                }
            );
            println!(
                "  • Keystore File: {}",
                path.display().to_string().bright_yellow()
            );
        }
    }

    Ok(())
}
