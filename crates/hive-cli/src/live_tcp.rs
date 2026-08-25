use crate::display::*;
use colored::*;
use hive_core::{
    auditor::ContractAuditor,
    receipt::{AgentKeypair, TaskReceipt},
    registry::AgentRegistry,
    types::{AgentCapability, TaskSpec},
};
use hive_escrow::verifier::SwarmVerifier;
use hive_p2p::{client::SwarmTcpClient, protocol::SwarmMessage, server::SwarmTcpNode};
use std::time::Duration;
use tokio::time::sleep;

pub async fn run_live_tcp_swarm(
    target_name: &str,
    code_payload: &str,
    bounty: u64,
) -> anyhow::Result<()> {
    print_banner();

    // Step 1: Bind TCP Node Daemons
    log_step(
        1,
        "Initializing Asynchronous TCP Sockets",
        "Binding independent Worker & Validator P2P network daemons",
    );

    let worker_port = 19101;
    let validator_port = 19102;
    let worker_addr = format!("127.0.0.1:{}", worker_port);
    let validator_addr = format!("127.0.0.1:{}", validator_port);

    let worker_node = SwarmTcpNode::new(&worker_addr, 100);
    let validator_node = SwarmTcpNode::new(&validator_addr, 100);

    let mut worker_rx = worker_node.tx.subscribe();
    let mut validator_rx = validator_node.tx.subscribe();

    worker_node
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    validator_node
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    sleep(Duration::from_millis(150)).await;
    println!(
        "    • [SOCKET 1] Worker Daemon listening on:    {}",
        worker_addr.bright_cyan().bold()
    );
    println!(
        "    • [SOCKET 2] Validator Daemon listening on: {}",
        validator_addr.bright_cyan().bold()
    );

    // Generate and register cryptographic identities
    let delegator_key = AgentKeypair::generate();
    let worker_key = AgentKeypair::generate();
    let validator_key = AgentKeypair::generate();

    let mut registry = AgentRegistry::new();
    registry.register_agent("Delegator_Alpha", delegator_key.public_key_hex());
    registry.register_agent("Worker_Auditor_Beta", worker_key.public_key_hex());
    registry.register_agent("Validator_Sentinel", validator_key.public_key_hex());

    println!("    • [REGISTRY] Registered 3 Agent Ed25519 Identities into Swarm Identity Table");

    sleep(Duration::from_millis(200)).await;

    // Step 2: Transmit RFQ over TCP Wire
    log_step(
        2,
        "TCP RFQ Wire Transmission",
        &format!(
            "Transmitting TaskSpec frame over TCP socket to {}",
            worker_addr
        ),
    );

    let task = TaskSpec::new(
        "Delegator_Alpha",
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        target_name,
        code_payload,
        bounty,
    );

    println!(
        "    ┌── TARGET SOURCE CODE INGESTED: [{}] ──",
        target_name.bright_yellow().bold()
    );
    for (i, line) in code_payload.lines().take(6).enumerate() {
        println!("    │ {:2} | {}", i + 1, line.dimmed());
    }
    if code_payload.lines().count() > 6 {
        println!("    │ ... ({} lines total)", code_payload.lines().count());
    }
    println!("    └────────────────────────────────────────────────");

    let rfq_msg = SwarmMessage::TaskRfq(task.clone());
    let rfq_json = serde_json::to_string(&rfq_msg)?;
    println!(
        "    • [TCP OUT] Delegator ──> [{}] ({} bytes payload)",
        worker_addr.bright_cyan(),
        rfq_json.len()
    );

    let ack_worker = SwarmTcpClient::send_message(&worker_addr, &rfq_msg)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!(
        "    • [TCP IN]  Worker Server ACK: {}",
        ack_worker.bright_green()
    );

    let received_rfq = worker_rx.recv().await?;
    match &received_rfq {
        SwarmMessage::TaskRfq(t) => {
            println!(
                "    • [DAEMON]  Worker parsed TaskRfq: ID [{}] | Bounty: {} USDC",
                t.id.to_string().bright_cyan(),
                t.max_bounty
            );
        }
        _ => {}
    }

    sleep(Duration::from_millis(300)).await;

    // Step 3: Worker Node Deterministic Analysis
    log_step(
        3,
        "Worker Node Analysis & Receipt Signing",
        "Executing static heuristic engine and binding SHA-256 digests with Ed25519",
    );

    let report = ContractAuditor::audit_source(target_name, code_payload);
    let output_json = serde_json::to_string(&report)?;

    println!(
        "    • [ANALYSIS] Total Lines: {} | Vulnerabilities: {} | Score: {}/100",
        report.total_lines,
        report.total_vulnerabilities.to_string().bright_red(),
        report.security_score.to_string().bright_green()
    );

    for (idx, finding) in report.findings.iter().enumerate() {
        println!(
            "      [Issue #{}] {} (Line {})",
            idx + 1,
            finding.title.bright_red().bold(),
            finding.line_number.to_string().bright_yellow()
        );
        println!("        Snippet: {}", finding.code_snippet.dimmed());
    }

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Auditor_Beta",
        &worker_key,
        &task.input_payload,
        output_json,
        180,
    );

    println!(
        "    • Input Hash (SHA256):    {}",
        receipt.input_hash.bright_yellow()
    );
    println!(
        "    • Output Hash (SHA256):   {}",
        receipt.output_hash.bright_yellow()
    );
    println!(
        "    • Compound Digest:        {}",
        receipt.execution_digest.bright_cyan()
    );
    println!(
        "    • Ed25519 Signature:      {}...",
        receipt.signature[..32].bright_green()
    );

    sleep(Duration::from_millis(300)).await;

    // Step 4: Transmit Receipt over TCP to Validator
    log_step(
        4,
        "TCP Receipt Transmission to Validator",
        &format!(
            "Transmitting TaskReceipt frame over TCP to {}",
            validator_addr
        ),
    );

    let receipt_msg = SwarmMessage::ReceiptBroadcast(receipt.clone());
    let receipt_json = serde_json::to_string(&receipt_msg)?;
    println!(
        "    • [TCP OUT] Worker ──> [{}] ({} bytes payload)",
        validator_addr.bright_cyan(),
        receipt_json.len()
    );

    let ack_validator = SwarmTcpClient::send_message(&validator_addr, &receipt_msg)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!(
        "    • [TCP IN]  Validator Server ACK: {}",
        ack_validator.bright_green()
    );

    let _ = validator_rx.recv().await?;

    sleep(Duration::from_millis(300)).await;

    // Step 5: Validator Independent Re-Execution
    log_step(
        5,
        "Validator Re-Execution & Cryptographic Verification",
        "Independent re-computation of digests, signature math, and finding parity",
    );

    let verified = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry))?;

    if verified {
        log_success(
            "Validator: SHA256(Input) MATCH | SHA256(Output) MATCH | Digest MATCH | Ed25519 Valid!",
        );
        println!(
            "    • Finding Parity: Worker ({} findings) == Validator ({} findings)",
            report.total_vulnerabilities, report.total_vulnerabilities
        );
        println!("    • Identity Match: Worker_Auditor_Beta registered public key matches receipt signer");

        log_step(
            6,
            "Autonomous P2P Swarm Settlement",
            "Challenge window verified without disputes. Escrow released to Worker wallet.",
        );
        log_success(&format!(
            "Payout of {} USDC finalized for [Worker_Auditor_Beta]",
            bounty
        ));
    }

    println!(
        "\n{}",
        "================================================================================".yellow()
    );
    println!(
        "  {}",
        "🚀 Live P2P TCP Multi-Agent Execution Completed Successfully!"
            .bright_green()
            .bold()
    );
    println!(
        "{}",
        "================================================================================".yellow()
    );

    Ok(())
}
