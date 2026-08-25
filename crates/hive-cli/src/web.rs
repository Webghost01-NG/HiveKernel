use colored::*;
use hive_core::auditor::ContractAuditor;
use hive_core::receipt::{AgentKeypair, TaskReceipt};
use hive_core::registry::AgentRegistry;
use hive_core::types::{AgentCapability, TaskSpec};
use hive_escrow::escrow::EscrowManager;
use hive_escrow::settlement::SwarmLedger;
use hive_escrow::verifier::SwarmVerifier;
use hive_p2p::auction::{AuctionMatcher, CandidateBid};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize)]
pub struct AuditRequest {
    pub file_name: String,
    pub code: String,
    pub bounty: u64,
    pub simulate_fraud: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct AuditResponse {
    pub task_id: String,
    pub target_name: String,
    pub winning_worker: String,
    pub input_hash: String,
    pub output_hash: String,
    pub execution_digest: String,
    pub signature: String,
    pub verified: bool,
    pub dispute_raised: bool,
    pub dispute_reason: Option<String>,
    pub report: hive_core::auditor::AuditReport,
}

pub async fn start_web_dashboard(port: u16) -> anyhow::Result<()> {
    // Bind to 0.0.0.0 so localhost, 127.0.0.1, and forwarded host ports connect seamlessly
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    println!("\n{}", "================================================================================".yellow());
    println!("  {}", "✨ HIVEKERNEL MISSION CONTROL DASHBOARD ACTIVE".bright_cyan().bold());
    println!("  {}", format!("  🚀 Localhost URL: http://localhost:{}", port).bright_green().bold());
    println!("  {}", format!("  🌐 Network URL:   http://0.0.0.0:{}", port).bright_yellow().bold());
    println!("  {}", "  ⚡ Features: Swarm Audit, P2P Topology, Dispute Sandbox, Live Staking Ledger".magenta());
    println!("{}", "================================================================================".yellow());

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_http_client(socket).await {
                eprintln!("HTTP Error: {}", e);
            }
        });
    }
}

async fn handle_http_client(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut header_buf = Vec::new();
    let mut temp_buf = [0u8; 2048];

    // Read headers until \r\n\r\n
    let (header_str, body_start_idx) = loop {
        let n = stream.read(&mut temp_buf).await?;
        if n == 0 {
            return Ok(());
        }
        header_buf.extend_from_slice(&temp_buf[..n]);

        if let Some(pos) = header_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_str = String::from_utf8_lossy(&header_buf[..pos]).to_string();
            break (header_str, pos + 4);
        }
    };

    let first_line = header_str.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    if method == "GET" && (path == "/" || path == "/index.html") {
        let html = get_sleek_dashboard_html();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        stream.write_all(response.as_bytes()).await?;
    } else if method == "POST" && path == "/api/audit" {
        let content_length: usize = header_str
            .lines()
            .find(|l| l.to_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);

        let mut body_bytes = header_buf[body_start_idx..].to_vec();
        while body_bytes.len() < content_length {
            let n = stream.read(&mut temp_buf).await?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&temp_buf[..n]);
        }

        if let Ok(req) = serde_json::from_slice::<AuditRequest>(&body_bytes) {
            let res = process_audit_request(req);
            let json = serde_json::to_string(&res)?;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        }

        let err = "{\"error\":\"Invalid JSON request body\"}";
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            err.len(),
            err
        );
        stream.write_all(response.as_bytes()).await?;
    } else {
        let not_found = "404 Not Found";
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            not_found.len(),
            not_found
        );
        stream.write_all(response.as_bytes()).await?;
    }

    Ok(())
}

fn process_audit_request(req: AuditRequest) -> AuditResponse {
    let mut ledger = SwarmLedger::new();
    let mut escrow = EscrowManager::new();
    let mut registry = AgentRegistry::new();

    let delegator_key = AgentKeypair::generate();
    let worker_key = AgentKeypair::generate();

    let delegator_id = "Delegator_Alpha".to_string();
    let worker_id = "Worker_Auditor_Beta".to_string();

    registry.register_agent(&delegator_id, delegator_key.public_key_hex());
    registry.register_agent(&worker_id, worker_key.public_key_hex());

    ledger.deposit(&delegator_id, 1000);
    let task = TaskSpec::new(
        delegator_id.clone(),
        delegator_key.public_key_hex(),
        AgentCapability::SmartContractAuditor,
        &req.file_name,
        &req.code,
        req.bounty,
    );

    ledger.withdraw(&delegator_id, req.bounty);
    escrow.lock_escrow(task.id, delegator_id.clone(), req.bounty).ok();

    let bid = CandidateBid {
        worker_id: worker_id.clone(),
        bid_bounty: req.bounty,
        estimated_duration_ms: 220,
        reputation_score: 98,
    };
    let winning_bid = AuctionMatcher::select_best_bid(&[bid], req.bounty).unwrap();
    escrow.assign_worker(task.id, winning_bid.worker_id.clone()).ok();

    let simulate_fraud = req.simulate_fraud.unwrap_or(false);

    let (output_json, report) = if simulate_fraud {
        let fake_clean_report = hive_core::auditor::AuditReport {
            target_name: req.file_name.clone(),
            total_lines: req.code.lines().count(),
            total_vulnerabilities: 0,
            security_score: 100,
            gas_efficiency_grade: "A+".to_string(),
            findings: vec![],
            summary: "Forged clean audit report: 0 vulnerabilities found".to_string(),
        };
        (serde_json::to_string(&fake_clean_report).unwrap(), fake_clean_report)
    } else {
        let genuine_report = ContractAuditor::audit_source(&req.file_name, &req.code);
        (serde_json::to_string(&genuine_report).unwrap(), genuine_report)
    };

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        winning_bid.worker_id.clone(),
        &worker_key,
        &task.input_payload,
        output_json,
        winning_bid.estimated_duration_ms,
    );

    escrow.submit_receipt(receipt.clone(), task.challenge_window_seconds).ok();
    
    let verification_result = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry));
    
    let (verified, dispute_raised, dispute_reason) = match verification_result {
        Ok(v) => {
            escrow.finalize_settlement(task.id).ok();
            (v, false, None)
        }
        Err(e) => {
            let reason = e.to_string();
            escrow.raise_dispute(task.id, reason.clone()).ok();
            escrow.finalize_settlement(task.id).ok();
            (false, true, Some(reason))
        }
    };

    AuditResponse {
        task_id: task.id.to_string(),
        target_name: req.file_name,
        winning_worker: winning_bid.worker_id,
        input_hash: receipt.input_hash,
        output_hash: receipt.output_hash,
        execution_digest: receipt.execution_digest,
        signature: receipt.signature,
        verified,
        dispute_raised,
        dispute_reason,
        report,
    }
}

fn get_sleek_dashboard_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>HiveKernel | Mission Control Dashboard</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css" rel="stylesheet">
    <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
    <script>
        tailwind.config = {
            darkMode: 'class',
            theme: {
                extend: {
                    fontFamily: {
                        sans: ['"Plus Jakarta Sans"', 'sans-serif'],
                        mono: ['"JetBrains Mono"', 'monospace'],
                    },
                    colors: {
                        hive: {
                            50: '#fefce8',
                            100: '#fef9c3',
                            400: '#facc15',
                            500: '#eab308',
                            glow: 'rgba(234, 179, 8, 0.25)'
                        }
                    }
                }
            }
        }
    </script>
    <style>
        body { background-color: #060913; color: #f1f5f9; }
        .glass-card { background: rgba(13, 19, 36, 0.7); backdrop-filter: blur(16px); border: 1px solid rgba(255, 255, 255, 0.08); }
        .glass-panel { background: rgba(15, 23, 42, 0.85); backdrop-filter: blur(20px); border: 1px solid rgba(255, 255, 255, 0.09); }
        .glow-hive { box-shadow: 0 0 30px rgba(234, 179, 8, 0.25); }
        .glow-cyan { box-shadow: 0 0 30px rgba(6, 182, 212, 0.25); }
        .glow-purple { box-shadow: 0 0 30px rgba(168, 85, 247, 0.25); }
        .custom-scrollbar::-webkit-scrollbar { width: 6px; height: 6px; }
        .custom-scrollbar::-webkit-scrollbar-track { background: rgba(15, 23, 42, 0.6); }
        .custom-scrollbar::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.15); border-radius: 9999px; }
        .custom-scrollbar::-webkit-scrollbar-thumb:hover { background: rgba(255, 255, 255, 0.25); }
    </style>
</head>
<body class="min-h-screen custom-scrollbar flex flex-col justify-between">
    
    <!-- Top Navigation Bar -->
    <header class="border-b border-slate-800/80 bg-slate-950/80 backdrop-blur-xl sticky top-0 z-50 px-6 py-3.5">
        <div class="max-w-7xl mx-auto flex items-center justify-between">
            <div class="flex items-center gap-4">
                <div class="w-10 h-10 rounded-xl bg-gradient-to-tr from-amber-500 to-yellow-300 flex items-center justify-center text-slate-950 font-black text-xl shadow-lg shadow-amber-500/20">
                    <i class="fa-solid fa-brands fa-hive text-2xl"></i>
                </div>
                <div>
                    <div class="flex items-center gap-2">
                        <span class="font-extrabold text-lg text-white tracking-tight">HiveKernel</span>
                        <span class="text-xs px-2 py-0.5 rounded-md font-mono font-bold bg-amber-400/10 text-amber-300 border border-amber-400/20">v0.3.0</span>
                    </div>
                    <p class="text-xs text-slate-400">Autonomous P2P Agent Subcontracting & Settlement Engine</p>
                </div>
            </div>

            <!-- Navigation Tabs -->
            <div class="hidden md:flex items-center gap-1 bg-slate-900/90 p-1.5 rounded-xl border border-slate-800">
                <button onclick="switchTab('audit')" id="tab-audit" class="px-4 py-1.5 rounded-lg text-xs font-bold transition-all bg-amber-400 text-slate-950 shadow-md">
                    <i class="fa-solid fa-shield-halved mr-1.5"></i> Swarm Audit
                </button>
                <button onclick="switchTab('topology')" id="tab-topology" class="px-4 py-1.5 rounded-lg text-xs font-semibold text-slate-400 hover:text-white transition-all">
                    <i class="fa-solid fa-network-wired mr-1.5"></i> P2P Topology
                </button>
                <button onclick="switchTab('dispute')" id="tab-dispute" class="px-4 py-1.5 rounded-lg text-xs font-semibold text-slate-400 hover:text-white transition-all">
                    <i class="fa-solid fa-gavel mr-1.5"></i> Fraud Sandbox
                </button>
                <button onclick="switchTab('ledger')" id="tab-ledger" class="px-4 py-1.5 rounded-lg text-xs font-semibold text-slate-400 hover:text-white transition-all">
                    <i class="fa-solid fa-vault mr-1.5"></i> Staking & Escrow
                </button>
            </div>

            <!-- System Live Status -->
            <div class="flex items-center gap-3">
                <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 text-xs font-semibold">
                    <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
                    <span>Local Host Online</span>
                </div>
                <div class="hidden sm:flex px-3 py-1.5 rounded-full bg-purple-500/10 border border-purple-500/30 text-purple-300 text-xs font-semibold">
                    <i class="fa-solid fa-trophy mr-1.5 text-purple-400"></i> Swarm Village
                </div>
            </div>
        </div>
    </header>

    <!-- Main Content Container -->
    <main class="max-w-7xl mx-auto p-6 flex-1 w-full space-y-6">

        <!-- TAB 1: SWARM AUDIT CONSOLE -->
        <div id="view-audit" class="space-y-6">
            
            <!-- Quick Template Selector -->
            <div class="flex flex-wrap items-center justify-between gap-4 glass-card p-4 rounded-2xl">
                <div class="flex items-center gap-2">
                    <span class="text-xs font-bold uppercase text-slate-400 tracking-wider">Load Contract Template:</span>
                </div>
                <div class="flex flex-wrap gap-2">
                    <button onclick="loadTemplate('reentrancy')" class="text-xs px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 font-medium transition-all">
                        <i class="fa-solid fa-triangle-exclamation text-amber-400 mr-1.5"></i> Reentrancy Vault (Vulnerable)
                    </button>
                    <button onclick="loadTemplate('txorigin')" class="text-xs px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 font-medium transition-all">
                        <i class="fa-solid fa-user-lock text-red-400 mr-1.5"></i> Insecure Auth (tx.origin)
                    </button>
                    <button onclick="loadTemplate('safe')" class="text-xs px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 font-medium transition-all">
                        <i class="fa-solid fa-circle-check text-emerald-400 mr-1.5"></i> Clean Staking (Secure)
                    </button>
                </div>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
                
                <!-- Left Column: Code Input & Parameters -->
                <div class="lg:col-span-5 space-y-4 glass-panel p-6 rounded-2xl">
                    <div class="flex items-center justify-between">
                        <h2 class="text-base font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-code text-cyan-400"></i> Smart Contract Ingestion
                        </h2>
                        <span class="text-xs font-mono text-slate-400">Solidity / Rust</span>
                    </div>

                    <div class="space-y-3">
                        <div>
                            <label class="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1.5">Contract Target Name</label>
                            <input id="fileName" type="text" value="LiquidityVault.sol" class="w-full bg-slate-950/80 border border-slate-800 rounded-xl px-4 py-2.5 text-xs font-mono text-cyan-300 focus:outline-none focus:border-cyan-500 transition-all">
                        </div>

                        <div>
                            <label class="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1.5">Bounty Escrow (USDC)</label>
                            <div class="relative">
                                <input id="bounty" type="number" value="250" class="w-full bg-slate-950/80 border border-slate-800 rounded-xl pl-4 pr-16 py-2.5 text-sm font-semibold text-white focus:outline-none focus:border-amber-400 transition-all">
                                <span class="absolute right-4 top-2.5 text-xs font-bold text-amber-400">USDC</span>
                            </div>
                        </div>

                        <div>
                            <label class="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1.5">Source Bytecode / Payload</label>
                            <textarea id="codeBody" rows="11" class="w-full bg-slate-950/90 border border-slate-800 rounded-xl p-4 text-xs font-mono text-slate-200 focus:outline-none focus:border-cyan-400 transition-all custom-scrollbar leading-relaxed">// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract LiquidityVault {
    mapping(address => uint256) public userBalances;

    function withdraw() external {
        uint256 amount = userBalances[msg.sender];
        require(amount > 0, "Zero balance");

        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "Transfer failed");

        userBalances[msg.sender] = 0; // Reentrancy state mutation
    }
}</textarea>
                        </div>

                        <button onclick="submitAudit(false)" id="btn-submit-audit" class="w-full py-3.5 bg-gradient-to-r from-amber-400 via-amber-500 to-yellow-500 hover:from-amber-300 hover:to-yellow-400 text-slate-950 font-black rounded-xl shadow-lg shadow-amber-500/20 transition-all flex items-center justify-center gap-2 text-sm glow-hive">
                            <i class="fa-solid fa-play"></i> Launch Autonomous Swarm Audit
                        </button>
                    </div>
                </div>

                <!-- Right Column: Swarm Telemetry & Findings -->
                <div class="lg:col-span-7 space-y-4">
                    
                    <!-- Real-Time Agent State Cards -->
                    <div class="grid grid-cols-3 gap-3">
                        <div class="glass-card p-4 rounded-2xl border-t-2 border-amber-400">
                            <div class="flex items-center justify-between text-xs text-slate-400 font-bold uppercase mb-1">
                                <span>Delegator</span>
                                <i class="fa-solid fa-user-astronaut text-amber-400"></i>
                            </div>
                            <div class="text-sm font-bold text-white truncate">Delegator_Alpha</div>
                            <div class="text-xs text-amber-400 font-mono mt-1 font-semibold">1,000 USDC</div>
                        </div>

                        <div class="glass-card p-4 rounded-2xl border-t-2 border-purple-400">
                            <div class="flex items-center justify-between text-xs text-slate-400 font-bold uppercase mb-1">
                                <span>Worker Agent</span>
                                <i class="fa-solid fa-microchip text-purple-400"></i>
                            </div>
                            <div id="card-worker-name" class="text-sm font-bold text-purple-300 truncate">Worker_Auditor_Beta</div>
                            <div class="text-xs text-purple-400 font-mono mt-1 font-semibold">Rep: 98/100 | Staked</div>
                        </div>

                        <div class="glass-card p-4 rounded-2xl border-t-2 border-emerald-400">
                            <div class="flex items-center justify-between text-xs text-slate-400 font-bold uppercase mb-1">
                                <span>Validator</span>
                                <i class="fa-solid fa-shield-check text-emerald-400"></i>
                            </div>
                            <div class="text-sm font-bold text-emerald-300 truncate">Validator_Sentinel</div>
                            <div id="card-validator-state" class="text-xs text-emerald-400 font-mono mt-1 font-semibold">Standby</div>
                        </div>
                    </div>

                    <!-- Telemetry Output Container -->
                    <div class="glass-panel p-6 rounded-2xl min-h-[460px] flex flex-col justify-between space-y-4">
                        <div class="flex items-center justify-between border-b border-slate-800/80 pb-3">
                            <h3 class="text-sm font-bold text-white flex items-center gap-2">
                                <i class="fa-solid fa-chart-line text-cyan-400"></i> Swarm Telemetry & Findings
                            </h3>
                            <span id="scoreBadge" class="text-xs font-mono font-extrabold px-3 py-1 bg-slate-900 text-slate-400 rounded-lg border border-slate-800">
                                Score: --
                            </span>
                        </div>

                        <div id="auditOutput" class="flex-1 space-y-4 custom-scrollbar">
                            <div class="h-64 flex flex-col items-center justify-center text-slate-500 text-center space-y-3">
                                <div class="w-14 h-14 rounded-2xl bg-slate-900 border border-slate-800 flex items-center justify-center text-slate-600 text-2xl">
                                    <i class="fa-solid fa-satellite-dish animate-pulse"></i>
                                </div>
                                <div>
                                    <p class="font-semibold text-slate-300 text-sm">Ready to audit</p>
                                    <p class="text-xs text-slate-500 mt-0.5">Click "Launch Autonomous Swarm Audit" to broadcast to nodes</p>
                                </div>
                            </div>
                        </div>
                    </div>

                </div>

            </div>
        </div>

        <!-- TAB 2: P2P TOPOLOGY -->
        <div id="view-topology" class="hidden space-y-6">
            <div class="glass-panel p-8 rounded-2xl space-y-6">
                <div class="flex items-center justify-between border-b border-slate-800 pb-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-network-wired text-cyan-400"></i> Active P2P Mesh Topology
                        </h2>
                        <p class="text-xs text-slate-400 mt-1">Live TCP socket connections and verified Ed25519 node identities</p>
                    </div>
                    <span class="px-3 py-1 bg-cyan-500/10 text-cyan-300 border border-cyan-500/20 text-xs font-mono font-bold rounded-lg">3 Nodes Mesh</span>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div class="glass-card p-5 rounded-xl border border-slate-800 space-y-3">
                        <div class="flex items-center justify-between">
                            <span class="font-bold text-white text-sm">Delegator Node</span>
                            <span class="w-2.5 h-2.5 rounded-full bg-amber-400"></span>
                        </div>
                        <div class="text-xs font-mono text-slate-400 space-y-1">
                            <div><span class="text-slate-500">ID:</span> Delegator_Alpha</div>
                            <div><span class="text-slate-500">Role:</span> Task Creator / Buyer</div>
                            <div><span class="text-slate-500">Protocol:</span> Tokio Async Broadcast</div>
                            <div><span class="text-slate-500">Status:</span> <span class="text-emerald-400">Connected</span></div>
                        </div>
                    </div>

                    <div class="glass-card p-5 rounded-xl border border-slate-800 space-y-3">
                        <div class="flex items-center justify-between">
                            <span class="font-bold text-white text-sm">Worker Node</span>
                            <span class="w-2.5 h-2.5 rounded-full bg-purple-400"></span>
                        </div>
                        <div class="text-xs font-mono text-slate-400 space-y-1">
                            <div><span class="text-slate-500">ID:</span> Worker_Auditor_Beta</div>
                            <div><span class="text-slate-500">TCP Port:</span> 19101</div>
                            <div><span class="text-slate-500">Capability:</span> SmartContractAuditor</div>
                            <div><span class="text-slate-500">Stake:</span> <span class="text-purple-400">0.50 ETH</span></div>
                        </div>
                    </div>

                    <div class="glass-card p-5 rounded-xl border border-slate-800 space-y-3">
                        <div class="flex items-center justify-between">
                            <span class="font-bold text-white text-sm">Validator Node</span>
                            <span class="w-2.5 h-2.5 rounded-full bg-emerald-400"></span>
                        </div>
                        <div class="text-xs font-mono text-slate-400 space-y-1">
                            <div><span class="text-slate-500">ID:</span> Validator_Sentinel</div>
                            <div><span class="text-slate-500">TCP Port:</span> 19102</div>
                            <div><span class="text-slate-500">Mode:</span> Re-Execution & FraudProof</div>
                            <div><span class="text-slate-500">Stake:</span> <span class="text-emerald-400">0.25 ETH</span></div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- TAB 3: FRAUD DISPUTE SANDBOX -->
        <div id="view-dispute" class="hidden space-y-6">
            <div class="glass-panel p-8 rounded-2xl space-y-6">
                <div class="flex items-center justify-between border-b border-slate-800 pb-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-gavel text-red-400"></i> Adversarial Dispute & Slashing Sandbox
                        </h2>
                        <p class="text-xs text-slate-400 mt-1">Simulate a malicious worker attempting to forge a clean report on vulnerable code</p>
                    </div>
                </div>

                <div class="p-6 bg-red-950/20 border border-red-500/20 rounded-2xl space-y-4">
                    <div class="flex items-start gap-4">
                        <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/30 flex items-center justify-center text-red-400 text-lg shrink-0">
                            <i class="fa-solid fa-shield-virus"></i>
                        </div>
                        <div>
                            <h3 class="font-bold text-red-300 text-sm">Adversarial Simulation Scenario</h3>
                            <p class="text-xs text-slate-400 mt-1 leading-relaxed">
                                The rogue worker receives a contract containing known vulnerabilities, but signs and submits a forged receipt claiming 0 issues and 100/100 score. The Validator re-executes the deterministic analysis kernel, catches the discrepancy, raises a cryptographic FraudProof, slashes the worker's collateral stake, and refunds the delegator.
                            </p>
                        </div>
                    </div>

                    <button onclick="submitAudit(true)" class="px-6 py-3 bg-red-600 hover:bg-red-500 text-white text-xs font-bold rounded-xl shadow-lg transition-all flex items-center gap-2">
                        <i class="fa-solid fa-biohazard"></i> Run Fraud Injection & Slashing Test
                    </button>
                </div>
            </div>
        </div>

        <!-- TAB 4: STAKING & ESCROW LEDGER -->
        <div id="view-ledger" class="hidden space-y-6">
            <div class="glass-panel p-8 rounded-2xl space-y-6">
                <div class="flex items-center justify-between border-b border-slate-800 pb-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-vault text-amber-400"></i> Non-Custodial Escrow & Staking Ledger
                        </h2>
                        <p class="text-xs text-slate-400 mt-1">Cryptographic balances and collateral state across all participants</p>
                    </div>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div class="glass-card p-6 rounded-2xl space-y-4">
                        <h3 class="text-sm font-bold text-white">Smart Contract Escrow Parameters</h3>
                        <div class="space-y-2 text-xs font-mono text-slate-300">
                            <div class="flex justify-between border-b border-slate-800/60 pb-1.5"><span class="text-slate-500">Contract Standard:</span> HiveEscrow.sol (EVM 0.8.20)</div>
                            <div class="flex justify-between border-b border-slate-800/60 pb-1.5"><span class="text-slate-500">Challenge Window:</span> 15 Seconds (Optimistic)</div>
                            <div class="flex justify-between border-b border-slate-800/60 pb-1.5"><span class="text-slate-500">Min Validator Stake:</span> 0.05 ETH</div>
                            <div class="flex justify-between"><span class="text-slate-500">Access Control:</span> onlyArbiter Protected</div>
                        </div>
                    </div>

                    <div class="glass-card p-6 rounded-2xl space-y-4">
                        <h3 class="text-sm font-bold text-white">Cryptographic Digest Formula</h3>
                        <div class="bg-slate-950 p-4 rounded-xl text-xs font-mono text-cyan-300 space-y-1">
                            <div>Digest = SHA256(</div>
                            <div class="pl-4">TaskId || WorkerID || InputHash || OutputHash || Timestamp</div>
                            <div>)</div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

    </main>

    <!-- Footer -->
    <footer class="border-t border-slate-800/80 bg-slate-950/60 px-6 py-4 text-center text-xs text-slate-500">
        <p>Built with 🦀 Rust & ⚡ Solidity for <span class="text-slate-400 font-semibold">Swarm Village Residency (HERŌ Network & Web3Bridge)</span></p>
    </footer>

    <!-- JavaScript Application Logic -->
    <script>
        function escapeHtml(text) {
            if (!text) return '';
            return String(text)
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#039;");
        }

        function switchTab(tabId) {
            const tabs = ['audit', 'topology', 'dispute', 'ledger'];
            tabs.forEach(t => {
                document.getElementById(`view-${t}`).classList.add('hidden');
                const btn = document.getElementById(`tab-${t}`);
                btn.className = "px-4 py-1.5 rounded-lg text-xs font-semibold text-slate-400 hover:text-white transition-all";
            });

            document.getElementById(`view-${tabId}`).classList.remove('hidden');
            const activeBtn = document.getElementById(`tab-${tabId}`);
            activeBtn.className = "px-4 py-1.5 rounded-lg text-xs font-bold transition-all bg-amber-400 text-slate-950 shadow-md";
        }

        const templates = {
            reentrancy: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract LiquidityVault {
    mapping(address => uint256) public userBalances;

    function withdraw() external {
        uint256 amount = userBalances[msg.sender];
        require(amount > 0, "Zero balance");

        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "Transfer failed");

        userBalances[msg.sender] = 0; // Reentrancy state mutation
    }
}`,
            txorigin: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract PhishableWallet {
    address public owner;

    constructor() { owner = msg.sender; }

    function transfer(address payable to, uint256 amount) public {
        require(tx.origin == owner, "Unauthorized"); // Vulnerable to phishing
        to.transfer(amount);
    }
}`,
            safe: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SafeStaking {
    mapping(address => uint256) public staked;

    function stake() external payable {
        require(msg.value > 0, "Zero stake");
        staked[msg.sender] += msg.value;
    }
}`
        };

        function loadTemplate(name) {
            document.getElementById('codeBody').value = templates[name];
            document.getElementById('fileName').value = name === 'reentrancy' ? 'LiquidityVault.sol' : (name === 'txorigin' ? 'PhishableWallet.sol' : 'SafeStaking.sol');
        }

        async function submitAudit(simulateFraud) {
            const fileName = document.getElementById('fileName').value;
            const bounty = parseInt(document.getElementById('bounty').value) || 200;
            const code = document.getElementById('codeBody').value;
            const output = document.getElementById('auditOutput');

            output.innerHTML = `
                <div class="p-8 text-center text-amber-400 space-y-3">
                    <i class="fa-solid fa-circle-notch fa-spin text-3xl"></i>
                    <p class="font-bold text-sm">Broadcasting Task to P2P Nodes & Running Verification...</p>
                </div>
            `;

            try {
                const res = await fetch('/api/audit', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ file_name: fileName, code: code, bounty: bounty, simulate_fraud: simulateFraud })
                });

                const data = await res.json();

                // Update Badges
                const scoreBadge = document.getElementById('scoreBadge');
                scoreBadge.innerText = `Score: ${data.report.security_score}/100`;
                scoreBadge.className = data.report.security_score > 70 
                    ? "text-xs font-mono font-extrabold px-3 py-1 bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-lg"
                    : "text-xs font-mono font-extrabold px-3 py-1 bg-red-500/20 text-red-400 border border-red-500/30 rounded-lg";

                let findingsList = '';
                if (data.report.findings.length === 0) {
                    findingsList = `<div class="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-emerald-400 text-xs font-bold flex items-center gap-2">
                        <i class="fa-solid fa-circle-check text-sm"></i> Clean Codebase! 0 vulnerabilities detected across ${data.report.total_lines} lines.
                    </div>`;
                } else {
                    findingsList = data.report.findings.map((f, i) => `
                        <div class="p-4 bg-slate-900 border-l-4 border-red-500 rounded-r-xl space-y-1.5">
                            <div class="flex justify-between items-center">
                                <span class="font-bold text-red-400 text-xs">[#${i+1}] ${escapeHtml(f.title)}</span>
                                <span class="text-[10px] font-mono bg-red-500/20 text-red-300 px-2 py-0.5 rounded">Line ${escapeHtml(f.line_number)}</span>
                            </div>
                            <pre class="text-[11px] font-mono text-slate-300 bg-slate-950 p-2.5 rounded whitespace-pre-wrap">${escapeHtml(f.code_snippet)}</pre>
                            <p class="text-[11px] text-emerald-300 font-semibold"><span class="text-slate-400 font-normal">Action:</span> ${escapeHtml(f.recommendation)}</p>
                        </div>
                    `).join('');
                }

                const fraudAlert = data.dispute_raised ? `
                    <div class="p-4 bg-red-500/10 border border-red-500/40 rounded-xl space-y-1">
                        <div class="text-xs font-bold text-red-400 flex items-center gap-2">
                            <i class="fa-solid fa-triangle-exclamation"></i> FRAUD DETECTED & DISPUTE RAISED
                        </div>
                        <p class="text-[11px] text-slate-300">${escapeHtml(data.dispute_reason)}</p>
                        <p class="text-[10px] text-red-300 font-mono">Worker collateral slashed. Delegator fully refunded.</p>
                    </div>
                ` : '';

                output.innerHTML = `
                    <div class="space-y-4">
                        ${fraudAlert}

                        <!-- Cryptographic Receipt Card -->
                        <div class="p-4 bg-slate-900/90 border border-slate-800 rounded-xl text-xs font-mono space-y-1.5">
                            <div class="flex justify-between"><span class="text-slate-500">Task ID:</span> <span class="text-cyan-400">${escapeHtml(data.task_id)}</span></div>
                            <div class="flex justify-between"><span class="text-slate-500">Winning Worker:</span> <span class="text-purple-400">${escapeHtml(data.winning_worker)}</span></div>
                            <div class="flex justify-between"><span class="text-slate-500">Input Hash:</span> <span class="text-slate-400 truncate max-w-xs">${escapeHtml(data.input_hash)}</span></div>
                            <div class="flex justify-between"><span class="text-slate-500">Compound Digest:</span> <span class="text-amber-400 truncate max-w-xs">${escapeHtml(data.execution_digest)}</span></div>
                            <div class="flex justify-between"><span class="text-slate-500">Signature:</span> <span class="text-emerald-400 truncate max-w-xs">${escapeHtml(data.signature.substring(0, 32))}...</span></div>
                            <div class="flex justify-between pt-1 border-t border-slate-800">
                                <span class="text-slate-500">Validator Verification:</span>
                                <span class="${data.verified ? "text-emerald-400 font-bold" : "text-red-400 font-bold"}">
                                    ${data.verified ? "✔ PASS (Mathematically Bound & Re-Executed)" : "✖ FRAUD REJECTED"}
                                </span>
                            </div>
                        </div>

                        <!-- Findings -->
                        <div class="space-y-2">
                            <h4 class="text-xs font-bold text-slate-400 uppercase tracking-wider">Findings (${data.report.total_vulnerabilities})</h4>
                            <div class="space-y-2.5">${findingsList}</div>
                        </div>
                    </div>
                `;

            } catch (err) {
                output.innerHTML = `<div class="p-4 bg-red-500/10 border border-red-500/30 rounded-xl text-red-400 text-xs">Error: ${escapeHtml(err.message)}</div>`;
            }
        }
    </script>
</body>
</html>"#.to_string()
}
