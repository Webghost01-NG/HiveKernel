use crate::display::*;
use colored::*;
use hive_core::{
    auditor::ContractAuditor,
    receipt::{AgentKeypair, TaskReceipt},
    types::{AgentCapability, TaskSpec},
};
use hive_escrow::{
    escrow::EscrowManager,
    settlement::SwarmLedger,
    verifier::SwarmVerifier,
};
use hive_p2p::{
    auction::{AuctionMatcher, CandidateBid},
    network::SwarmMeshRouter,
    protocol::SwarmMessage,
};
use std::time::Duration;
use tokio::time::sleep;

pub async fn run_swarm_simulation(task_title: &str, bounty: u64, trigger_dispute: bool) -> anyhow::Result<()> {
    print_banner();

    // 1. Initialize Swarm Ledger & Mesh Network
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();
    let router = SwarmMeshRouter::new(100);

    // Create Agent Keypairs
    let delegator_key = AgentKeypair::generate();
    let worker_a_key = AgentKeypair::generate();
    let _worker_b_key = AgentKeypair::generate();
    let _validator_key = AgentKeypair::generate();

    let delegator_id = "Delegator_Alpha".to_string();
    let worker_a_id = "Worker_Auditor_Beta".to_string();
    let worker_b_id = "Worker_Speedy_Gamma".to_string();
    let validator_id = "Validator_Sentinel".to_string();

    // Fund Accounts
    ledger.deposit(&delegator_id, 1000);
    ledger.deposit(&worker_a_id, 50);
    ledger.deposit(&worker_b_id, 50);
    ledger.deposit(&validator_id, 200);

    log_step(1, "Swarm Initialization & Wallet States", "Setting up P2P Mesh nodes and Initial Balances");
    println!("    • Delegator balance: {} USDC", ledger.balance_of(&delegator_id).to_string().bright_yellow());
    println!("    • Worker Beta balance: {} USDC", ledger.balance_of(&worker_a_id).to_string().bright_yellow());
    println!("    • Worker Gamma balance: {} USDC", ledger.balance_of(&worker_b_id).to_string().bright_yellow());
    println!("    • Validator balance: {} USDC", ledger.balance_of(&validator_id).to_string().bright_yellow());

    sleep(Duration::from_millis(600)).await;

    // 2. Step 2: Task Creation & Escrow Locking
    log_step(2, "Task Creation & Non-Custodial Escrow", "Delegator locks bounty funds in smart contract and broadcasts RFQ");
    
    let sample_contract = r#"
    contract Vault {
        mapping(address => uint256) public balances;
        function withdraw() external {
            uint256 bal = balances[msg.sender];
            (bool s, ) = msg.sender.call{value: bal}("");
            balances[msg.sender] = 0; // State change after external call
        }
    }
    "#;

    let task = TaskSpec::new(
        delegator_id.clone(),
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        task_title,
        sample_contract,
        bounty,
    );

    ledger.withdraw(&delegator_id, bounty);
    escrow.lock_escrow(task.id, delegator_id.clone(), bounty)?;
    
    log_event("DELEGATOR", &format!("Created Task [{}] - Bounty: {} USDC", task.id.to_string().bright_cyan(), bounty));
    log_event("ESCROW", &format!("Locked {} USDC into Escrow Contract", bounty));

    router.broadcast(SwarmMessage::TaskRfq(task.clone())).ok();
    sleep(Duration::from_millis(700)).await;

    // 3. Step 3: P2P Reverse Auction & Bid Ranking
    log_step(3, "P2P Reverse Auction & Capability Matching", "Specialist agents discover RFQ on GossipSub and submit cryptographic bids");

    let bid_a = CandidateBid {
        worker_id: worker_a_id.clone(),
        bid_bounty: bounty,
        estimated_duration_ms: 320,
        reputation_score: 98,
    };

    let bid_b = CandidateBid {
        worker_id: worker_b_id.clone(),
        bid_bounty: bounty - 20,
        estimated_duration_ms: 150,
        reputation_score: 82,
    };

    println!("    • Bid from {}: Bounty {} USDC | Rep: {} | Est: {}ms", worker_a_id.bright_purple(), bid_a.bid_bounty, bid_a.reputation_score, bid_a.estimated_duration_ms);
    println!("    • Bid from {}: Bounty {} USDC | Rep: {} | Est: {}ms", worker_b_id.bright_purple(), bid_b.bid_bounty, bid_b.reputation_score, bid_b.estimated_duration_ms);

    let winning_bid = AuctionMatcher::select_best_bid(&[bid_a, bid_b], bounty)?;
    log_success(&format!("Auction Won by [{}] with optimal score!", winning_bid.worker_id.bright_green().bold()));
    
    escrow.assign_worker(task.id, winning_bid.worker_id.clone())?;
    sleep(Duration::from_millis(700)).await;

    // 4. Step 4: Autonomous Work Execution & Cryptographic Receipt Generation
    log_step(4, "Autonomous Work Execution & Signed Receipt", "Worker agent executes static analysis on contract code and seals output with Ed25519");

    let output_content = if trigger_dispute {
        "Audit Report: Vulnerability scan failed <<MALICIOUS_INJECTION>> bypass checks".to_string()
    } else {
        let audit_report = ContractAuditor::audit_solidity_code(&task.input_payload);
        serde_json::to_string_pretty(&audit_report)?
    };

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        winning_bid.worker_id.clone(),
        &worker_a_key,
        &task.input_payload,
        output_content,
        winning_bid.estimated_duration_ms,
    );

    println!("    • Execution Digest: {}", receipt.execution_digest.bright_cyan());
    println!("    • Ed25519 Signature: {}...", &receipt.signature[..32].bright_yellow());
    println!("    • Real Audit Output:\n{}", receipt.output_payload.italic());

    escrow.submit_receipt(receipt.clone(), task.challenge_window_seconds)?;
    sleep(Duration::from_millis(800)).await;

    // 5. Step 5: Optimistic Challenge Window & Multi-Agent Verification
    log_step(5, "Optimistic Challenge Window & Sentinel Verification", "Independent Validator nodes review execution digest and payload integrity");

    let verification_result = SwarmVerifier::verify_work(&task, &receipt);

    match verification_result {
        Ok(_) => {
            log_success("Validator Node: Cryptographic signature verified & output passed semantic validation!");
            log_event("OPTIMISTIC WINDOW", "No disputes raised during the challenge period (15s). Ready for settlement.");
            
            // Release funds
            let status = escrow.finalize_settlement(task.id)?;
            ledger.deposit(&winning_bid.worker_id, bounty);

            log_step(6, "Autonomous Settlement Finality", "Smart contract releases bounty directly to Worker wallet");
            log_success(&format!("Payout of {} USDC transferred to [{}]", bounty, winning_bid.worker_id.bright_green().bold()));
            println!("    • New Worker Beta Balance: {} USDC", ledger.balance_of(&worker_a_id).to_string().bright_green().bold());
            println!("    • Escrow State: {:?}", status);
        }
        Err(e) => {
            log_dispute(&format!("Validator Node Detected Fraud: {}", e));
            escrow.raise_dispute(task.id, e.to_string())?;
            let status = escrow.finalize_settlement(task.id)?;
            
            // Refund delegator, slash worker
            ledger.deposit(&delegator_id, bounty);
            log_dispute("Worker penalised & Delegator fully refunded by Escrow contract!");
            println!("    • Delegator Refunded Balance: {} USDC", ledger.balance_of(&delegator_id).to_string().bright_yellow());
            println!("    • Escrow State: {:?}", status);
        }
    }

    println!("\n{}", "================================================================================".yellow());
    println!("  {}", "🚀 HiveKernel Demo Completed Successfully!".bright_green().bold());
    println!("{}", "================================================================================".yellow());

    Ok(())
}
