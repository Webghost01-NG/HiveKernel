use crate::display::*;
use chrono::{Duration as ChronoDuration, Utc};
use colored::*;
use hive_core::{
    auditor::{AuditReport, ContractAuditor},
    receipt::{AgentKeypair, TaskReceipt},
    registry::AgentRegistry,
    types::{AgentCapability, TaskSpec},
};
use hive_escrow::{escrow::EscrowManager, settlement::SwarmLedger, verifier::SwarmVerifier};
use hive_p2p::auction::{AuctionMatcher, CandidateBid};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

pub async fn run_swarm_simulation(
    task_title: &str,
    file_path: Option<&str>,
    bounty: u64,
    trigger_dispute: bool,
) -> anyhow::Result<()> {
    print_banner();

    // 1. Resolve Code Payload & Target Name
    let (target_name, code_payload) = match file_path {
        Some(path) => {
            let content = fs::read_to_string(path)?;
            let file_name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(task_title);
            (file_name.to_string(), content)
        }
        None => {
            let default_contract = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract LiquidityVault {
    mapping(address => uint256) public userBalances;

    function withdraw() external {
        uint256 amount = userBalances[msg.sender];
        require(amount > 0, "Zero balance");
        
        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "Transfer failed");

        userBalances[msg.sender] = 0; // State mutation after external call
    }
}
"#;
            (task_title.to_string(), default_contract.to_string())
        }
    };

    // 2. Initialize Swarm Ledger, Escrow, and Agent Identity Registry
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();
    let mut registry = AgentRegistry::new();

    let delegator_key = AgentKeypair::generate();
    let worker_beta_key = AgentKeypair::generate();
    let worker_gamma_key = AgentKeypair::generate();
    let validator_key = AgentKeypair::generate();

    let delegator_id = "Delegator_Alpha".to_string();
    let worker_beta_id = "Worker_Auditor_Beta".to_string();
    let worker_gamma_id = "Worker_Speedy_Gamma".to_string();
    let validator_id = "Validator_Sentinel".to_string();

    // Register cryptographic identities in Swarm Registry
    registry.register_agent(&delegator_id, delegator_key.public_key_hex());
    registry.register_agent(&worker_beta_id, worker_beta_key.public_key_hex());
    registry.register_agent(&worker_gamma_id, worker_gamma_key.public_key_hex());
    registry.register_agent(&validator_id, validator_key.public_key_hex());

    ledger.deposit(&delegator_id, 1000);
    ledger.deposit(&worker_beta_id, 100);
    ledger.deposit(&worker_gamma_id, 100);
    ledger.deposit(&validator_id, 250);

    log_step(
        1,
        "Swarm Initialization & Cryptographic Registry",
        "Registering Ed25519 node identities and verifying balances",
    );
    println!(
        "    • Delegator balance: {} USDC | PubKey: {}...",
        ledger.balance_of(&delegator_id).to_string().bright_yellow(),
        &delegator_key.public_key_hex()[..16]
    );
    println!(
        "    • Worker Beta balance: {} USDC | PubKey: {}...",
        ledger
            .balance_of(&worker_beta_id)
            .to_string()
            .bright_yellow(),
        &worker_beta_key.public_key_hex()[..16]
    );
    println!(
        "    • Worker Gamma balance: {} USDC | PubKey: {}...",
        ledger
            .balance_of(&worker_gamma_id)
            .to_string()
            .bright_yellow(),
        &worker_gamma_key.public_key_hex()[..16]
    );
    println!(
        "    • Validator balance: {} USDC | PubKey: {}...",
        ledger.balance_of(&validator_id).to_string().bright_yellow(),
        &validator_key.public_key_hex()[..16]
    );

    sleep(Duration::from_millis(300)).await;

    // 3. Step 2: Task Creation & Non-Custodial Escrow Locking
    log_step(
        2,
        "Task Creation & Non-Custodial Escrow",
        "Delegator locks bounty funds in escrow ledger and broadcasts RFQ",
    );

    if !ledger.withdraw(&delegator_id, bounty) {
        let err_msg = format!(
            "Insufficient funds: Delegator has {} USDC, but bounty requires {} USDC",
            ledger.balance_of(&delegator_id),
            bounty
        );
        log_dispute(&err_msg);
        return Err(anyhow::anyhow!(err_msg));
    }

    let task = TaskSpec::new(
        delegator_id.clone(),
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        &target_name,
        &code_payload,
        bounty,
    );

    escrow.lock_escrow(task.id, delegator_id.clone(), bounty)?;

    log_event(
        "DELEGATOR",
        &format!(
            "Created Task [{}] for [{}] - Bounty: {} USDC",
            task.id.to_string().bright_cyan(),
            target_name.bold(),
            bounty
        ),
    );
    log_event("ESCROW", &format!("Locked {} USDC into Escrow", bounty));

    sleep(Duration::from_millis(400)).await;

    // 4. Step 3: P2P Reverse Auction & Bid Ranking
    log_step(
        3,
        "P2P Reverse Auction & Capability Matching",
        "Specialist agents discover RFQ and submit competitive cryptographic bids",
    );

    let bid_a = CandidateBid {
        worker_id: worker_beta_id.clone(),
        bid_bounty: bounty,
        estimated_duration_ms: 320,
        reputation_score: 98,
    };

    let bid_b = CandidateBid {
        worker_id: worker_gamma_id.clone(),
        bid_bounty: bounty.saturating_sub(20).max(1),
        estimated_duration_ms: 150,
        reputation_score: 82,
    };

    println!(
        "    • Bid from {}: Bounty {} USDC | Rep: {} | Est: {}ms",
        worker_beta_id.bright_purple(),
        bid_a.bid_bounty,
        bid_a.reputation_score,
        bid_a.estimated_duration_ms
    );
    println!(
        "    • Bid from {}: Bounty {} USDC | Rep: {} | Est: {}ms",
        worker_gamma_id.bright_purple(),
        bid_b.bid_bounty,
        bid_b.reputation_score,
        bid_b.estimated_duration_ms
    );

    let winning_bid = AuctionMatcher::select_best_bid(&[bid_a, bid_b], bounty)?;
    log_success(&format!(
        "Auction Won by [{}] with optimal reputation & price efficiency!",
        winning_bid.worker_id.bright_green().bold()
    ));

    escrow.assign_worker(task.id, winning_bid.worker_id.clone())?;
    sleep(Duration::from_millis(400)).await;

    // 5. Step 4: Autonomous Work Execution & Cryptographic Receipt Generation
    log_step(
        4,
        "Autonomous Work Execution & Signed Receipt",
        "Assigned Worker executes deterministic static analysis and signs execution digest",
    );

    // Select the keypair matching the actual winning worker
    let winning_worker_key = if winning_bid.worker_id == worker_beta_id {
        &worker_beta_key
    } else {
        &worker_gamma_key
    };

    let output_content = if trigger_dispute {
        // Rogue Worker intentionally forges a clean report with 0 issues on vulnerable code
        let fake_clean_report = AuditReport {
            target_name: target_name.clone(),
            total_lines: code_payload.lines().count(),
            total_vulnerabilities: 0,
            security_score: 100,
            gas_efficiency_grade: "A+".to_string(),
            findings: vec![],
            summary: "Forged clean audit report: 0 vulnerabilities found".to_string(),
        };
        serde_json::to_string_pretty(&fake_clean_report)?
    } else {
        let genuine_report = ContractAuditor::audit_source(&target_name, &code_payload);
        serde_json::to_string_pretty(&genuine_report)?
    };

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        winning_bid.worker_id.clone(),
        winning_worker_key,
        &task.input_payload,
        output_content,
        winning_bid.estimated_duration_ms,
    );

    println!(
        "    • Task ID: {}",
        receipt.task_id.to_string().bright_cyan()
    );
    println!(
        "    • Worker Identity: {}",
        receipt.worker_id.bright_purple()
    );
    println!(
        "    • Input Hash (SHA256): {}",
        receipt.input_hash.bright_yellow()
    );
    println!(
        "    • Output Hash (SHA256): {}",
        receipt.output_hash.bright_yellow()
    );
    println!(
        "    • Compound Execution Digest: {}",
        receipt.execution_digest.bright_cyan()
    );
    println!(
        "    • Ed25519 Signature: {}...",
        receipt.signature[..32].bright_yellow()
    );

    escrow.submit_receipt(receipt.clone(), task.challenge_window_seconds)?;
    sleep(Duration::from_millis(400)).await;

    // 6. Step 5: Optimistic Challenge Window & Deterministic Re-Execution
    log_step(
        5,
        "Optimistic Challenge Window & Deterministic Verification",
        "Validator verifies Ed25519 signature, registry identity, and recomputes execution digest",
    );

    let verification_result =
        SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry));

    match verification_result {
        Ok(_) => {
            log_success("Validator Node: Cryptographic digest bound, signature verified & re-execution passed!");
            log_event(
                "CHALLENGE WINDOW",
                &format!(
                    "Challenge deadline set to +{}s. Waiting for optimistic finality...",
                    task.challenge_window_seconds
                ),
            );

            // Advance time past challenge window for settlement
            let settlement_time =
                Utc::now() + ChronoDuration::seconds(task.challenge_window_seconds as i64 + 1);
            let status = escrow.finalize_settlement_at(task.id, settlement_time)?;
            ledger.deposit(&winning_bid.worker_id, bounty);

            log_step(
                6,
                "Autonomous Settlement Finality",
                "Escrow releases bounty directly to Worker wallet",
            );
            log_success(&format!(
                "Payout of {} USDC transferred to [{}]",
                bounty,
                winning_bid.worker_id.bright_green().bold()
            ));
            println!(
                "    • New Worker [{}] Balance: {} USDC",
                winning_bid.worker_id,
                ledger
                    .balance_of(&winning_bid.worker_id)
                    .to_string()
                    .bright_green()
                    .bold()
            );
            println!("    • Escrow State: {:?}", status);
        }
        Err(e) => {
            log_dispute(&format!("Fraud Detected by Validator Node: {}", e));
            escrow.raise_dispute(task.id, e.to_string())?;

            let settlement_time =
                Utc::now() + ChronoDuration::seconds(task.challenge_window_seconds as i64 + 1);
            let status = escrow.finalize_settlement_at(task.id, settlement_time)?;

            ledger.deposit(&delegator_id, bounty);
            log_dispute("Worker slashed & Delegator fully refunded by Escrow!");
            println!(
                "    • Delegator Refunded Balance: {} USDC",
                ledger.balance_of(&delegator_id).to_string().bright_yellow()
            );
            println!("    • Escrow State: {:?}", status);
        }
    }

    println!(
        "\n{}",
        "================================================================================".yellow()
    );
    println!(
        "  {}",
        "🚀 HiveKernel Execution Completed Successfully!"
            .bright_green()
            .bold()
    );
    println!(
        "{}",
        "================================================================================".yellow()
    );

    Ok(())
}
