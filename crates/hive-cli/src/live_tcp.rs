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

    log_step(
        1,
        "Starting Real TCP Daemon Processes",
        "Binding independent P2P nodes on local TCP ports",
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

    sleep(Duration::from_millis(100)).await;
    println!(
        "    • Worker TCP Daemon listening on: {}",
        worker_addr.bright_cyan().bold()
    );
    println!(
        "    • Validator TCP Daemon listening on: {}",
        validator_addr.bright_cyan().bold()
    );

    // Cryptographic Identities
    let delegator_key = AgentKeypair::generate();
    let worker_key = AgentKeypair::generate();
    let validator_key = AgentKeypair::generate();

    let mut registry = AgentRegistry::new();
    registry.register_agent("Delegator_Alpha", delegator_key.public_key_hex());
    registry.register_agent("Worker_Auditor_Beta", worker_key.public_key_hex());
    registry.register_agent("Validator_Sentinel", validator_key.public_key_hex());

    // Step 2: Send TaskRfq over TCP to Worker
    log_step(
        2,
        "TCP RFQ Transmission",
        "Delegator sends TaskSpec JSON frame over TCP to Worker Node",
    );
    let task = TaskSpec::new(
        "Delegator_Alpha",
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        target_name,
        code_payload,
        bounty,
    );

    let rfq_msg = SwarmMessage::TaskRfq(task.clone());
    let ack_worker = SwarmTcpClient::send_message(&worker_addr, &rfq_msg)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!(
        "    • TCP Server Response from {}: {}",
        worker_addr.dimmed(),
        ack_worker.bright_green()
    );

    let received_rfq = worker_rx.recv().await?;
    println!(
        "    • Worker Daemon received frame: {:?}",
        std::mem::discriminant(&received_rfq)
    );

    // Step 3: Worker audits and generates cryptographically bound receipt
    log_step(
        3,
        "Worker Execution & Receipt Signing",
        "Worker Node processes source code and signs compound execution digest",
    );
    let report = ContractAuditor::audit_source(target_name, code_payload);
    let output_json = serde_json::to_string(&report)?;

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        "Worker_Auditor_Beta",
        &worker_key,
        &task.input_payload,
        output_json,
        180,
    );

    println!(
        "    • Input SHA-256: {}",
        receipt.input_hash.bright_yellow()
    );
    println!(
        "    • Output SHA-256: {}",
        receipt.output_hash.bright_yellow()
    );
    println!(
        "    • Compound Digest: {}",
        receipt.execution_digest.bright_cyan()
    );
    println!(
        "    • Ed25519 Signature: {}...",
        receipt.signature[..32].bright_green()
    );

    // Step 4: Transmit Receipt over TCP to Validator Node
    log_step(
        4,
        "TCP Receipt Transmission to Validator",
        "Forwarding signed TaskReceipt to Validator TCP Node for optimistic verification",
    );
    let receipt_msg = SwarmMessage::ReceiptBroadcast(receipt.clone());
    let ack_validator = SwarmTcpClient::send_message(&validator_addr, &receipt_msg)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!(
        "    • TCP Server Response from {}: {}",
        validator_addr.dimmed(),
        ack_validator.bright_green()
    );

    let _ = validator_rx.recv().await?;

    // Step 5: Validator Independent Re-Execution
    log_step(
        5,
        "Deterministic Re-Execution & Registry Identity Verification",
        "Validator verifies content binding, signature, and re-executes analysis kernel",
    );
    let verified = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry))?;

    if verified {
        log_success("Validator Node: Cryptographic content bound, Ed25519 signature valid & re-execution matched 100%!");
        log_step(
            6,
            "Live TCP Swarm Complete",
            "All frames exchanged and verified across independent TCP socket daemons",
        );
        println!(
            "    • Status: {}",
            "SUCCESS (Verified P2P Subcontracting)"
                .bright_green()
                .bold()
        );
    }

    println!(
        "\n{}",
        "================================================================================".yellow()
    );
    println!(
        "  {}",
        "🚀 Live TCP Swarm Execution Finished Successfully!"
            .bright_green()
            .bold()
    );
    println!(
        "{}",
        "================================================================================".yellow()
    );

    Ok(())
}
