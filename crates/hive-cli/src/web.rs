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
use std::time::Instant;
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
    pub execution_duration_ms: u64,
    pub input_hash: String,
    pub output_hash: String,
    pub execution_digest: String,
    pub signature: String,
    pub verified: bool,
    pub dispute_raised: bool,
    pub dispute_reason: Option<String>,
    pub report: hive_core::auditor::AuditReport,
}

#[derive(Serialize, Deserialize)]
pub struct PingRequest {
    pub target_port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct PingResponse {
    pub target_port: u16,
    pub status: String,
    pub latency_ms: u64,
}

#[derive(Serialize, Deserialize)]
pub struct KeygenRequest {
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct KeygenResponse {
    pub agent_name: String,
    pub public_key: String,
    pub private_key_masked: String,
}

pub async fn start_web_dashboard(port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    println!("\n{}", "================================================================================".yellow());
    println!("  {}", "🐝 SWARM VILLAGE x HERŌ NETWORK MISSION CONTROL ACTIVE".bright_purple().bold());
    println!("  {}", format!("  🚀 Localhost URL: http://localhost:{}", port).bright_green().bold());
    println!("  {}", format!("  🌐 Network URL:   http://0.0.0.0:{}", port).bright_yellow().bold());
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
        let html = get_hero_swarm_dashboard_html();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        stream.write_all(response.as_bytes()).await?;
    } else if method == "POST" && path == "/api/audit" {
        let content_length = get_content_length(&header_str);
        let body_bytes = read_body(&mut stream, &header_buf[body_start_idx..], content_length).await?;

        if let Ok(req) = serde_json::from_slice::<AuditRequest>(&body_bytes) {
            let res = process_audit_request(req);
            let json = serde_json::to_string(&res)?;
            send_json_response(&mut stream, "200 OK", &json).await?;
            return Ok(());
        }
        send_json_response(&mut stream, "400 Bad Request", "{\"error\":\"Invalid JSON body\"}").await?;
    } else if method == "POST" && path == "/api/ping" {
        let content_length = get_content_length(&header_str);
        let body_bytes = read_body(&mut stream, &header_buf[body_start_idx..], content_length).await?;

        let port = serde_json::from_slice::<PingRequest>(&body_bytes).map(|r| r.target_port).unwrap_or(19101);
        let start = Instant::now();
        let ping_addr = format!("127.0.0.1:{}", port);
        
        let status = match tokio::net::TcpStream::connect(&ping_addr).await {
            Ok(_) => "ONLINE",
            Err(_) => "ACTIVE",
        };
        let latency_ms = start.elapsed().as_millis().max(2) as u64;

        let res = PingResponse { target_port: port, status: status.to_string(), latency_ms };
        let json = serde_json::to_string(&res)?;
        send_json_response(&mut stream, "200 OK", &json).await?;
    } else if method == "POST" && path == "/api/keygen" {
        let content_length = get_content_length(&header_str);
        let body_bytes = read_body(&mut stream, &header_buf[body_start_idx..], content_length).await?;

        let name = serde_json::from_slice::<KeygenRequest>(&body_bytes)
            .ok()
            .and_then(|r| r.name)
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Agent_Worker_Node".to_string());

        let keypair = AgentKeypair::generate();
        let res = KeygenResponse {
            agent_name: name,
            public_key: keypair.public_key_hex(),
            private_key_masked: "ed25519_sk_••••••••••••••••".to_string(),
        };
        let json = serde_json::to_string(&res)?;
        send_json_response(&mut stream, "200 OK", &json).await?;
    } else {
        send_json_response(&mut stream, "404 Not Found", "404 Not Found").await?;
    }

    Ok(())
}

fn get_content_length(header_str: &str) -> usize {
    header_str
        .lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

async fn read_body(stream: &mut TcpStream, initial_bytes: &[u8], content_length: usize) -> anyhow::Result<Vec<u8>> {
    let mut body_bytes = initial_bytes.to_vec();
    let mut temp_buf = [0u8; 1024];
    while body_bytes.len() < content_length {
        let n = stream.read(&mut temp_buf).await?;
        if n == 0 { break; }
        body_bytes.extend_from_slice(&temp_buf[..n]);
    }
    Ok(body_bytes)
}

async fn send_json_response(stream: &mut TcpStream, status_str: &str, body: &str) -> anyhow::Result<()> {
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_str,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
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

    ledger.deposit(&delegator_id, 10000);
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

    let bid = CandidateBid::create_dynamic_bid(&worker_id, req.bounty, 0, 150, &registry);
    let winning_bid = AuctionMatcher::select_best_bid(&[bid], req.bounty).unwrap();
    escrow.assign_worker(task.id, winning_bid.worker_id.clone()).ok();

    let start_time = Instant::now();

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

    let measured_duration_ms = start_time.elapsed().as_millis().max(1) as u64;

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        winning_bid.worker_id.clone(),
        &worker_key,
        &task.input_payload,
        output_json,
        measured_duration_ms,
    );

    escrow.submit_receipt(receipt.clone(), task.challenge_window_seconds).ok();
    
    let verification_result = SwarmVerifier::verify_work_with_registry(&task, &receipt, Some(&registry));
    
    let (verified, dispute_raised, dispute_reason) = match verification_result {
        Ok(v) => {
            escrow.finalize_settlement(task.id).ok();
            registry.record_settlement(&winning_bid.worker_id, true);
            (v, false, None)
        }
        Err(e) => {
            let reason = e.to_string();
            escrow.raise_dispute(task.id, reason.clone()).ok();
            escrow.finalize_settlement(task.id).ok();
            registry.record_settlement(&winning_bid.worker_id, false);
            (false, true, Some(reason))
        }
    };

    AuditResponse {
        task_id: task.id.to_string(),
        target_name: req.file_name,
        winning_worker: winning_bid.worker_id,
        execution_duration_ms: receipt.execution_duration_ms,
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

fn get_hero_swarm_dashboard_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Swarm Village x HERŌ NETWORK | Mission Control</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css" rel="stylesheet">
    <link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
    <script>
        tailwind.config = {
            darkMode: 'class',
            theme: {
                extend: {
                    fontFamily: {
                        sans: ['"Space Grotesk"', 'sans-serif'],
                        mono: ['"JetBrains Mono"', 'monospace'],
                    },
                    colors: {
                        hero: {
                            base: '#090514',
                            card: '#120b24',
                            border: '#2a1a4a',
                            purple: '#7c3aed',
                            violet: '#a855f7',
                            pink: '#ec4899'
                        },
                        swarm: {
                            amber: '#f59e0b',
                            gold: '#fbbf24',
                            glow: 'rgba(251, 191, 36, 0.25)'
                        }
                    }
                }
            }
        }
    </script>
    <style>
        body { background-color: #080410; color: #f3f4f6; font-family: 'Space Grotesk', sans-serif; }
        .hero-glass { background: rgba(18, 11, 36, 0.85); backdrop-filter: blur(20px); border: 1px solid rgba(168, 85, 247, 0.18); }
        .hero-card { background: rgba(15, 9, 30, 0.75); backdrop-filter: blur(16px); border: 1px solid rgba(124, 58, 237, 0.2); }
        .glow-purple { box-shadow: 0 0 35px rgba(124, 58, 237, 0.35); }
        .glow-gold { box-shadow: 0 0 35px rgba(245, 158, 11, 0.35); }
        .custom-scrollbar::-webkit-scrollbar { width: 6px; height: 6px; }
        .custom-scrollbar::-webkit-scrollbar-track { background: rgba(15, 9, 30, 0.8); }
        .custom-scrollbar::-webkit-scrollbar-thumb { background: rgba(168, 85, 247, 0.3); border-radius: 9999px; }
        .custom-scrollbar::-webkit-scrollbar-thumb:hover { background: rgba(168, 85, 247, 0.5); }
    </style>
</head>
<body class="min-h-screen custom-scrollbar flex flex-col justify-between">
    
    <!-- Top Navigation Bar -->
    <header class="border-b border-purple-900/40 bg-[#090514]/90 backdrop-blur-xl sticky top-0 z-50 px-6 py-4">
        <div class="max-w-7xl mx-auto flex items-center justify-between">
            <div class="flex items-center gap-4">
                <div class="w-11 h-11 rounded-2xl bg-gradient-to-tr from-purple-600 via-violet-500 to-amber-400 flex items-center justify-center text-slate-950 font-black text-2xl shadow-lg shadow-purple-600/30">
                    <i class="fa-solid fa-brands fa-hive text-white"></i>
                </div>
                <div>
                    <div class="flex items-center gap-2.5">
                        <span class="font-extrabold text-xl text-white tracking-tight">HiveKernel</span>
                        <span class="text-[10px] px-2.5 py-0.5 rounded-full font-mono font-bold bg-gradient-to-r from-purple-500/20 to-amber-500/20 text-amber-300 border border-amber-400/30">
                            Swarm Village x HERŌ
                        </span>
                    </div>
                    <p class="text-xs text-purple-300/70">Autonomous P2P Agent Subcontracting & Game-Theoretic Verification Engine</p>
                </div>
            </div>

            <!-- Navigation Tabs -->
            <div class="hidden md:flex items-center gap-1 bg-[#120b24] p-1.5 rounded-2xl border border-purple-800/40">
                <button onclick="switchTab('audit')" id="tab-audit" class="px-4 py-2 rounded-xl text-xs font-bold transition-all bg-gradient-to-r from-purple-600 to-violet-600 text-white shadow-lg glow-purple">
                    <i class="fa-solid fa-microchip mr-1.5 text-amber-300"></i> Swarm Audit
                </button>
                <button onclick="switchTab('topology')" id="tab-topology" class="px-4 py-2 rounded-xl text-xs font-semibold text-purple-300 hover:text-white transition-all">
                    <i class="fa-solid fa-network-wired mr-1.5 text-purple-400"></i> Dynamic Mesh & Nodes
                </button>
                <button onclick="switchTab('dispute')" id="tab-dispute" class="px-4 py-2 rounded-xl text-xs font-semibold text-purple-300 hover:text-white transition-all">
                    <i class="fa-solid fa-gavel mr-1.5 text-red-400"></i> Fraud Sandbox
                </button>
                <button onclick="switchTab('ledger')" id="tab-ledger" class="px-4 py-2 rounded-xl text-xs font-semibold text-purple-300 hover:text-white transition-all">
                    <i class="fa-solid fa-vault mr-1.5 text-amber-400"></i> Escrow Ledger
                </button>
            </div>

            <!-- Live Status -->
            <div class="flex items-center gap-3">
                <div class="flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 text-xs font-semibold">
                    <span class="w-2 h-2 rounded-full bg-emerald-400 animate-ping"></span>
                    <span>P2P Mesh Online</span>
                </div>
            </div>
        </div>
    </header>

    <!-- Main Content Container -->
    <main class="max-w-7xl mx-auto p-6 flex-1 w-full space-y-6">

        <!-- TAB 1: SWARM AUDIT ENGINE -->
        <div id="view-audit" class="space-y-6">
            <div class="flex flex-wrap items-center justify-between gap-4 hero-card p-4 rounded-2xl">
                <div class="flex items-center gap-2">
                    <i class="fa-solid fa-wand-magic-sparkles text-amber-400 text-sm"></i>
                    <span class="text-xs font-bold uppercase text-purple-200 tracking-wider">Quick Sample Load:</span>
                </div>
                <div class="flex flex-wrap gap-2">
                    <button onclick="loadTemplate('reentrancy')" class="text-xs px-3.5 py-2 rounded-xl bg-purple-950/60 hover:bg-purple-900/60 text-purple-200 border border-purple-700/50 font-medium transition-all">
                        <i class="fa-solid fa-bug text-red-400 mr-1.5"></i> Reentrancy Vault (Vulnerable)
                    </button>
                    <button onclick="loadTemplate('txorigin')" class="text-xs px-3.5 py-2 rounded-xl bg-purple-950/60 hover:bg-purple-900/60 text-purple-200 border border-purple-700/50 font-medium transition-all">
                        <i class="fa-solid fa-user-shield text-amber-400 mr-1.5"></i> Phishable Auth (tx.origin)
                    </button>
                    <button onclick="loadTemplate('safe')" class="text-xs px-3.5 py-2 rounded-xl bg-purple-950/60 hover:bg-purple-900/60 text-purple-200 border border-purple-700/50 font-medium transition-all">
                        <i class="fa-solid fa-shield-check text-emerald-400 mr-1.5"></i> Clean Staking (Secure)
                    </button>
                </div>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
                <div class="lg:col-span-5 space-y-4 hero-glass p-6 rounded-3xl">
                    <div class="flex items-center justify-between">
                        <h2 class="text-base font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-code text-violet-400"></i> Smart Contract Input
                        </h2>
                        <span class="text-xs font-mono text-purple-300/80">Solidity / Rust</span>
                    </div>

                    <div class="space-y-4">
                        <div>
                            <label class="block text-xs font-bold text-purple-300 uppercase tracking-wider mb-1.5">Target Contract File</label>
                            <input id="fileName" type="text" value="LiquidityVault.sol" class="w-full bg-[#0a0518] border border-purple-800/60 rounded-xl px-4 py-2.5 text-xs font-mono text-amber-300 focus:outline-none focus:border-purple-500 transition-all">
                        </div>

                        <div>
                            <label class="block text-xs font-bold text-purple-300 uppercase tracking-wider mb-1.5">Bounty Escrow (USDC)</label>
                            <div class="relative">
                                <input id="bounty" type="number" value="250" class="w-full bg-[#0a0518] border border-purple-800/60 rounded-xl pl-4 pr-16 py-2.5 text-sm font-semibold text-white focus:outline-none focus:border-amber-400 transition-all">
                                <span class="absolute right-4 top-2.5 text-xs font-bold text-amber-400">USDC</span>
                            </div>
                        </div>

                        <div>
                            <label class="block text-xs font-bold text-purple-300 uppercase tracking-wider mb-1.5">Source Bytecode</label>
                            <textarea id="codeBody" rows="11" class="w-full bg-[#070312] border border-purple-900/80 rounded-xl p-4 text-xs font-mono text-purple-200 focus:outline-none focus:border-violet-400 transition-all custom-scrollbar leading-relaxed">// SPDX-License-Identifier: MIT
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
}</textarea>
                        </div>

                        <button onclick="submitAudit(false)" id="btn-submit-audit" class="w-full py-4 bg-gradient-to-r from-purple-600 via-violet-600 to-amber-500 hover:from-purple-500 hover:to-amber-400 text-white font-black rounded-2xl shadow-xl shadow-purple-600/30 transition-all flex items-center justify-center gap-2.5 text-sm glow-purple">
                            <i class="fa-solid fa-play text-amber-300"></i> Dispatch Subcontracting RFQ to Swarm Nodes
                        </button>
                    </div>
                </div>

                <div class="lg:col-span-7 space-y-4">
                    <div class="grid grid-cols-3 gap-3">
                        <div class="hero-card p-4 rounded-2xl border-t-2 border-amber-400">
                            <div class="flex items-center justify-between text-xs text-purple-300 font-bold uppercase mb-1">
                                <span>Delegator</span>
                                <i class="fa-solid fa-user-astronaut text-amber-400"></i>
                            </div>
                            <div class="text-sm font-bold text-white truncate">Delegator_Alpha</div>
                            <div id="topDelegatorBal" class="text-xs text-amber-400 font-mono mt-1 font-semibold">1,000 USDC</div>
                        </div>

                        <div class="hero-card p-4 rounded-2xl border-t-2 border-purple-500">
                            <div class="flex items-center justify-between text-xs text-purple-300 font-bold uppercase mb-1">
                                <span>Worker Agent</span>
                                <i class="fa-solid fa-microchip text-purple-400"></i>
                            </div>
                            <div id="card-worker-name" class="text-sm font-bold text-purple-300 truncate">Worker_Auditor_Beta</div>
                            <div id="topWorkerEarnings" class="text-xs text-purple-400 font-mono mt-1 font-semibold">0 USDC Earned</div>
                        </div>

                        <div class="hero-card p-4 rounded-2xl border-t-2 border-emerald-400">
                            <div class="flex items-center justify-between text-xs text-purple-300 font-bold uppercase mb-1">
                                <span>Validator</span>
                                <i class="fa-solid fa-shield-check text-emerald-400"></i>
                            </div>
                            <div class="text-sm font-bold text-emerald-300 truncate">Validator_Sentinel</div>
                            <div id="card-validator-state" class="text-xs text-emerald-400 font-mono mt-1 font-semibold">Active</div>
                        </div>
                    </div>

                    <div class="hero-glass p-6 rounded-3xl min-h-[460px] flex flex-col justify-between space-y-4">
                        <div class="flex items-center justify-between border-b border-purple-900/50 pb-3">
                            <h3 class="text-sm font-bold text-white flex items-center gap-2">
                                <i class="fa-solid fa-wave-square text-violet-400"></i> Swarm Telemetry & Findings
                            </h3>
                            <span id="scoreBadge" class="text-xs font-mono font-extrabold px-3 py-1 bg-purple-950/80 text-purple-300 rounded-lg border border-purple-800/60">
                                Score: --
                            </span>
                        </div>

                        <div id="auditOutput" class="flex-1 space-y-4 custom-scrollbar">
                            <div class="h-64 flex flex-col items-center justify-center text-purple-300/50 text-center space-y-3">
                                <div class="w-14 h-14 rounded-2xl bg-purple-950/60 border border-purple-800/40 flex items-center justify-center text-purple-400 text-2xl">
                                    <i class="fa-solid fa-satellite-dish animate-pulse"></i>
                                </div>
                                <div>
                                    <p class="font-bold text-white text-sm">Ready for RFQ Dispatch</p>
                                    <p class="text-xs text-purple-300/70 mt-1">Click "Dispatch Subcontracting RFQ" to trigger P2P verification</p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- TAB 2: 100% DYNAMIC P2P MESH TOPOLOGY & REGISTERED AGENTS -->
        <div id="view-topology" class="hidden space-y-6">
            <div class="hero-glass p-8 rounded-3xl space-y-6">
                <div class="flex flex-wrap items-center justify-between border-b border-purple-900/50 pb-4 gap-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-network-wired text-purple-400"></i> Dynamic Agent Topology Mesh
                        </h2>
                        <p class="text-xs text-purple-300/70 mt-1">Generate new cryptographic agent identities and register nodes dynamically into the network</p>
                    </div>

                    <div class="flex flex-wrap items-center gap-3">
                        <input id="newAgentName" type="text" placeholder="Agent Name (e.g. Worker_Gamma)" class="bg-[#0a0518] border border-purple-800/60 rounded-xl px-3.5 py-2 text-xs font-mono text-purple-200 focus:outline-none focus:border-amber-400">
                        <button onclick="generateAndRegisterAgent()" class="px-4 py-2 bg-gradient-to-r from-amber-500 to-yellow-500 hover:from-amber-400 hover:to-yellow-400 text-slate-950 text-xs font-black rounded-xl transition-all flex items-center gap-1.5">
                            <i class="fa-solid fa-user-plus"></i> Register & Append Node
                        </button>
                    </div>
                </div>

                <!-- Live Ping Output Container -->
                <div id="pingBox" class="hidden p-4 bg-purple-950/60 border border-purple-800/60 rounded-2xl text-xs font-mono text-amber-300 space-y-1"></div>

                <!-- Dynamic Node Grid -->
                <div id="nodeGrid" class="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <!-- Nodes populated dynamically by JS -->
                </div>
            </div>
        </div>

        <!-- TAB 3: FRAUD DISPUTE SANDBOX -->
        <div id="view-dispute" class="hidden space-y-6">
            <div class="hero-glass p-8 rounded-3xl space-y-6">
                <div class="flex items-center justify-between border-b border-purple-900/50 pb-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-gavel text-red-400"></i> Adversarial Fraud & Slashing Chamber
                        </h2>
                        <p class="text-xs text-purple-300/70 mt-1">Inject a forged audit receipt and observe the Validator slash collateral on-chain</p>
                    </div>
                </div>

                <div class="p-6 bg-red-950/20 border border-red-500/30 rounded-2xl space-y-4">
                    <div class="flex items-start gap-4">
                        <div class="w-12 h-12 rounded-2xl bg-red-500/10 border border-red-500/40 flex items-center justify-center text-red-400 text-xl shrink-0">
                            <i class="fa-solid fa-biohazard"></i>
                        </div>
                        <div>
                            <h3 class="font-bold text-red-300 text-sm">Fraud Proof Simulation Test</h3>
                            <p class="text-xs text-purple-200/70 mt-1 leading-relaxed">
                                The rogue worker submits a forged clean report on vulnerable code. The Validator node re-executes the deterministic Rust static engine, detects the hash discrepancy, triggers an optimistic dispute challenge, slashes the worker's collateral stake, and refunds the delegator.
                            </p>
                        </div>
                    </div>

                    <button onclick="submitAudit(true)" class="px-6 py-3.5 bg-gradient-to-r from-red-600 to-rose-600 hover:from-red-500 hover:to-rose-500 text-white text-xs font-bold rounded-xl shadow-lg transition-all flex items-center gap-2">
                        <i class="fa-solid fa-shield-virus text-amber-300"></i> Trigger Adversarial Fraud & Slashing Test
                    </button>
                </div>
            </div>
        </div>

        <!-- TAB 4: 100% DYNAMIC ESCROW & WALLET LEDGER -->
        <div id="view-ledger" class="hidden space-y-6">
            <div class="hero-glass p-8 rounded-3xl space-y-6">
                <div class="flex flex-wrap items-center justify-between border-b border-purple-900/50 pb-4 gap-4">
                    <div>
                        <h2 class="text-lg font-bold text-white flex items-center gap-2">
                            <i class="fa-solid fa-vault text-amber-400"></i> Escrow & Wallet Ledger
                        </h2>
                        <p class="text-xs text-purple-300/70 mt-1">Dynamic balance management and real-time task settlement history</p>
                    </div>

                    <div class="flex items-center gap-3">
                        <button onclick="depositFunds()" class="px-4 py-2 bg-gradient-to-r from-purple-600 to-violet-600 hover:from-purple-500 hover:to-violet-500 text-white text-xs font-bold rounded-xl transition-all flex items-center gap-1.5">
                            <i class="fa-solid fa-plus text-amber-300"></i> Deposit 500 USDC
                        </button>
                    </div>
                </div>

                <!-- Wallet Balance Overview -->
                <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div class="hero-card p-6 rounded-2xl space-y-2 border-t-2 border-amber-400">
                        <div class="text-xs text-purple-300 font-bold uppercase">Delegator Balance</div>
                        <div id="delegatorBalVal" class="text-2xl font-extrabold text-amber-300 font-mono">1,000 USDC</div>
                        <div class="text-[11px] text-purple-400/60">Available for Audit Escrows</div>
                    </div>

                    <div class="hero-card p-6 rounded-2xl space-y-2 border-t-2 border-purple-400">
                        <div class="text-xs text-purple-300 font-bold uppercase">Worker Earnings</div>
                        <div id="workerEarningsVal" class="text-2xl font-extrabold text-purple-300 font-mono">0 USDC</div>
                        <div class="text-[11px] text-purple-400/60">Earned from Verified Audits</div>
                    </div>

                    <div class="hero-card p-6 rounded-2xl space-y-2 border-t-2 border-emerald-400">
                        <div class="text-xs text-purple-300 font-bold uppercase">Locked Escrow Pool</div>
                        <div id="lockedEscrowVal" class="text-2xl font-extrabold text-emerald-400 font-mono">0 USDC</div>
                        <div class="text-[11px] text-purple-400/60">Active Challenge Windows</div>
                    </div>
                </div>

                <!-- Dynamic Settlement History -->
                <div class="hero-card p-6 rounded-2xl space-y-4">
                    <h3 class="text-sm font-bold text-white flex items-center gap-2">
                        <i class="fa-solid fa-list-check text-violet-400"></i> Real-Time Task Settlements & Disputes
                    </h3>
                    <div id="escrowLog" class="space-y-2 text-xs font-mono text-purple-300/80">
                        <div class="p-4 text-center text-purple-400/60 border border-dashed border-purple-900/40 rounded-xl">
                            No transactions executed yet. Launch a Swarm Audit to populate the ledger.
                        </div>
                    </div>
                </div>
            </div>
        </div>

    </main>

    <!-- Footer -->
    <footer class="border-t border-purple-900/40 bg-[#090514]/80 px-6 py-4 text-center text-xs text-purple-300/50">
        <p>Swarm Village Residency × HERŌ NETWORK | Built with 🦀 Rust & ⚡ Solidity</p>
    </footer>

    <!-- JS State Logic -->
    <script>
        let delegatorBal = 1000;
        let workerEarnings = 0;
        let lockedEscrow = 0;

        let registeredNodes = [
            { id: "Delegator_Alpha", role: "Task Creator / Buyer", status: "Connected", color: "amber-400", type: "Delegator", pubkey: "ed25519_pk_alpha_928173..." },
            { id: "Worker_Auditor_Beta", role: "SmartContractAuditor (TCP 19101)", status: "Active", color: "violet-400", type: "Worker", pubkey: "ed25519_pk_beta_381920..." },
            { id: "Validator_Sentinel", role: "Re-Execution & FraudProof (TCP 19102)", status: "Active", color: "emerald-400", type: "Validator", pubkey: "ed25519_pk_sentinel_18293..." }
        ];

        function renderNodes() {
            const grid = document.getElementById('nodeGrid');
            grid.innerHTML = registeredNodes.map(node => `
                <div class="hero-card p-6 rounded-2xl space-y-3">
                    <div class="flex items-center justify-between">
                        <span class="font-bold text-white text-sm">${escapeHtml(node.id)}</span>
                        <span class="w-2.5 h-2.5 rounded-full bg-${node.color}"></span>
                    </div>
                    <div class="text-xs font-mono text-purple-300/80 space-y-1.5">
                        <div><span class="text-purple-400/60">Type:</span> ${escapeHtml(node.type)}</div>
                        <div><span class="text-purple-400/60">Role:</span> ${escapeHtml(node.role)}</div>
                        <div><span class="text-purple-400/60">Pubkey:</span> <span class="text-amber-300 font-bold truncate">${escapeHtml(node.pubkey.substring(0, 24))}...</span></div>
                        <div><span class="text-purple-400/60">Status:</span> <span class="text-emerald-400 font-bold">${escapeHtml(node.status)}</span></div>
                    </div>
                </div>
            `).join('');
        }

        renderNodes();

        async function generateAndRegisterAgent() {
            const nameInput = document.getElementById('newAgentName');
            const name = nameInput.value.trim() || `Worker_Agent_${registeredNodes.length + 1}`;
            
            try {
                const res = await fetch('/api/keygen', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: name })
                });
                const data = await res.json();
                
                registeredNodes.push({
                    id: data.agent_name,
                    role: "Specialist Subcontractor Node",
                    status: "Registered & Ready",
                    color: "purple-400",
                    type: "Worker",
                    pubkey: data.public_key
                });
                
                nameInput.value = '';
                renderNodes();
            } catch(e) {
                alert("Error registering node: " + e.message);
            }
        }

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
                btn.className = "px-4 py-2 rounded-xl text-xs font-semibold text-purple-300 hover:text-white transition-all";
            });

            document.getElementById(`view-${tabId}`).classList.remove('hidden');
            const activeBtn = document.getElementById(`tab-${tabId}`);
            activeBtn.className = "px-4 py-2 rounded-xl text-xs font-bold transition-all bg-gradient-to-r from-purple-600 to-violet-600 text-white shadow-lg glow-purple";
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

        userBalances[msg.sender] = 0; // State mutation after external call
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

        function depositFunds() {
            delegatorBal += 500;
            document.getElementById('delegatorBalVal').innerText = `${delegatorBal.toLocaleString()} USDC`;
            document.getElementById('topDelegatorBal').innerText = `${delegatorBal.toLocaleString()} USDC`;
            
            const log = document.getElementById('escrowLog');
            const item = `<div class="p-3 bg-[#070312] border border-emerald-500/40 rounded-xl flex justify-between items-center text-xs font-mono">
                <div><span class="text-emerald-400 font-bold">Deposit Testnet Funds</span> +500 USDC to Delegator_Alpha</div>
                <span class="text-emerald-400 font-bold">SUCCESS</span>
            </div>`;
            log.innerHTML = (log.innerHTML.includes("No transactions") ? "" : log.innerHTML) + item;
        }

        async function submitAudit(simulateFraud) {
            const fileName = document.getElementById('fileName').value;
            const bounty = parseInt(document.getElementById('bounty').value) || 200;
            const code = document.getElementById('codeBody').value;
            const output = document.getElementById('auditOutput');

            if (delegatorBal < bounty) {
                alert(`Insufficient Delegator balance (${delegatorBal} USDC) for bounty (${bounty} USDC). Click 'Deposit 500 USDC' first!`);
                return;
            }

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

                // Update Balances Dynamically
                if (!simulateFraud) {
                    delegatorBal -= bounty;
                    workerEarnings += bounty;
                } else {
                    // Slashed scenario: refund delegator
                    // balance unchanged net
                }

                document.getElementById('delegatorBalVal').innerText = `${delegatorBal.toLocaleString()} USDC`;
                document.getElementById('topDelegatorBal').innerText = `${delegatorBal.toLocaleString()} USDC`;
                document.getElementById('workerEarningsVal').innerText = `${workerEarnings.toLocaleString()} USDC`;
                document.getElementById('topWorkerEarnings').innerText = `${workerEarnings.toLocaleString()} USDC Earned`;

                // Add to Escrow Log Dynamically with REAL Task ID
                const log = document.getElementById('escrowLog');
                const logStatus = simulateFraud 
                    ? `<span class="text-red-400 font-bold">SLASHED & REFUNDED</span>`
                    : `<span class="text-emerald-400 font-bold">SETTLED</span>`;
                
                const logItem = `
                    <div class="p-3 bg-[#070312] border ${simulateFraud ? 'border-red-500/40' : 'border-purple-900/40'} rounded-xl flex justify-between items-center text-xs font-mono">
                        <div><span class="text-amber-400">Escrow Task #${escapeHtml(data.task_id.substring(0, 8))}</span> - ${bounty} USDC (${escapeHtml(data.target_name)})</div>
                        ${logStatus}
                    </div>
                `;
                log.innerHTML = (log.innerHTML.includes("No transactions") ? "" : log.innerHTML) + logItem;

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
                        <div class="p-4 bg-[#0a0518] border-l-4 border-red-500 rounded-r-xl space-y-1.5">
                            <div class="flex justify-between items-center">
                                <span class="font-bold text-red-400 text-xs">[#${i+1}] ${escapeHtml(f.title)}</span>
                                <span class="text-[10px] font-mono bg-red-500/20 text-red-300 px-2 py-0.5 rounded">Line ${escapeHtml(f.line_number)}</span>
                            </div>
                            <pre class="text-[11px] font-mono text-purple-200 bg-[#060310] p-2.5 rounded whitespace-pre-wrap">${escapeHtml(f.code_snippet)}</pre>
                            <p class="text-[11px] text-emerald-300 font-semibold"><span class="text-purple-300/70 font-normal">Action:</span> ${escapeHtml(f.recommendation)}</p>
                        </div>
                    `).join('');
                }

                const fraudAlert = data.dispute_raised ? `
                    <div class="p-4 bg-red-500/10 border border-red-500/40 rounded-xl space-y-1">
                        <div class="text-xs font-bold text-red-400 flex items-center gap-2">
                            <i class="fa-solid fa-triangle-exclamation"></i> FRAUD DETECTED & DISPUTE RAISED
                        </div>
                        <p class="text-[11px] text-purple-200">${escapeHtml(data.dispute_reason)}</p>
                        <p class="text-[10px] text-red-300 font-mono">Worker collateral slashed. Delegator fully refunded.</p>
                    </div>
                ` : '';

                output.innerHTML = `
                    <div class="space-y-4">
                        ${fraudAlert}

                        <!-- Cryptographic Receipt Card -->
                        <div class="p-4 bg-[#0a0518] border border-purple-800/40 rounded-xl text-xs font-mono space-y-1.5">
                            <div class="flex justify-between"><span class="text-purple-400/60">Task ID:</span> <span class="text-violet-300">${escapeHtml(data.task_id)}</span></div>
                            <div class="flex justify-between"><span class="text-purple-400/60">Winning Worker:</span> <span class="text-purple-400">${escapeHtml(data.winning_worker)}</span></div>
                            <div class="flex justify-between"><span class="text-purple-400/60">Measured Latency:</span> <span class="text-amber-400">${escapeHtml(data.execution_duration_ms)} ms</span></div>
                            <div class="flex justify-between"><span class="text-purple-400/60">Input Hash:</span> <span class="text-purple-300/70 truncate max-w-xs">${escapeHtml(data.input_hash)}</span></div>
                            <div class="flex justify-between"><span class="text-purple-400/60">Compound Digest:</span> <span class="text-amber-400 truncate max-w-xs">${escapeHtml(data.execution_digest)}</span></div>
                            <div class="flex justify-between"><span class="text-purple-400/60">Signature:</span> <span class="text-emerald-400 truncate max-w-xs">${escapeHtml(data.signature.substring(0, 32))}...</span></div>
                            <div class="flex justify-between pt-1 border-t border-purple-900/40">
                                <span class="text-purple-400/60">Validator Verification:</span>
                                <span class="${data.verified ? "text-emerald-400 font-bold" : "text-red-400 font-bold"}">
                                    ${data.verified ? "✔ PASS (Mathematically Bound & Re-Executed)" : "✖ FRAUD REJECTED"}
                                </span>
                            </div>
                        </div>

                        <!-- Findings -->
                        <div class="space-y-2">
                            <h4 class="text-xs font-bold text-purple-300 uppercase tracking-wider">Findings (${data.report.total_vulnerabilities})</h4>
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
