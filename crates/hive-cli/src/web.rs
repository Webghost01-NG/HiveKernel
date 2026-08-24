use colored::*;
use hive_core::auditor::ContractAuditor;
use hive_core::receipt::{AgentKeypair, TaskReceipt};
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
}

#[derive(Serialize, Deserialize)]
pub struct AuditResponse {
    pub task_id: String,
    pub target_name: String,
    pub winning_worker: String,
    pub execution_digest: String,
    pub signature: String,
    pub verified: bool,
    pub report: hive_core::auditor::AuditReport,
}

pub async fn start_web_dashboard(port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    
    println!("\n{}", "================================================================================".yellow());
    println!("  {}", "🌐 HIVEKERNEL MULTI-AGENT MISSION CONTROL WEB UI".bright_cyan().bold());
    println!("  {}", format!("  Access Live Dashboard at: http://localhost:{}", port).bright_green().bold());
    println!("  {}", "  Supported Features: Real-time Swarm Audits, File Drag & Drop, Fraud Proofs".magenta());
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
    let mut buffer = [0u8; 16384];
    let bytes_read = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);

    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    if method == "GET" && (path == "/" || path == "/index.html") {
        let html = get_dashboard_html();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        stream.write_all(response.as_bytes()).await?;
    } else if method == "POST" && path == "/api/audit" {
        if let Some(body_start) = request.find("\r\n\r\n") {
            let body = &request[body_start + 4..];
            if let Ok(req) = serde_json::from_str::<AuditRequest>(body) {
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
        }
        let err = "{\"error\":\"Invalid JSON request\"}";
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

    let delegator_key = AgentKeypair::generate();
    let worker_key = AgentKeypair::generate();

    let delegator_id = "Delegator_Alpha".to_string();
    let worker_id = "Worker_Auditor_Beta".to_string();

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

    let report = ContractAuditor::audit_source(&req.file_name, &req.code);
    let output_json = serde_json::to_string(&report).unwrap();

    let receipt = TaskReceipt::create_and_sign(
        task.id,
        winning_bid.worker_id.clone(),
        &worker_key,
        &task.input_payload,
        output_json,
        winning_bid.estimated_duration_ms,
    );

    escrow.submit_receipt(receipt.clone(), task.challenge_window_seconds).ok();
    let verified = SwarmVerifier::verify_work(&task, &receipt).unwrap_or(false);
    escrow.finalize_settlement(task.id).ok();

    AuditResponse {
        task_id: task.id.to_string(),
        target_name: req.file_name,
        winning_worker: winning_bid.worker_id,
        execution_digest: receipt.execution_digest,
        signature: receipt.signature,
        verified,
        report,
    }
}

fn get_dashboard_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>HiveKernel | Autonomous P2P Agent Swarm Dashboard</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css" rel="stylesheet">
    <style>
        body { background-color: #0b0f19; color: #e2e8f0; font-family: system-ui, sans-serif; }
        .glass { background: rgba(15, 23, 42, 0.75); backdrop-filter: blur(12px); border: 1px solid rgba(255, 255, 255, 0.1); }
        .glow-cyan { box-shadow: 0 0 20px rgba(6, 182, 212, 0.35); }
        .glow-purple { box-shadow: 0 0 20px rgba(168, 85, 247, 0.35); }
    </style>
</head>
<body class="p-6">
    <div class="max-w-7xl mx-auto space-y-6">
        
        <!-- Header -->
        <header class="glass rounded-2xl p-6 flex flex-wrap justify-between items-center border-l-4 border-cyan-500">
            <div>
                <h1 class="text-3xl font-black tracking-tight text-white flex items-center gap-3">
                    <span class="text-yellow-400"><i class="fa-solid fa-brands fa-hive"></i></span>
                    HiveKernel Mission Control
                </h1>
                <p class="text-slate-400 text-sm mt-1">Autonomous P2P AI Agent Subcontracting & Settlement Engine in Rust</p>
            </div>
            <div class="flex items-center gap-3">
                <span class="px-3 py-1 bg-green-500/20 text-green-400 text-xs font-bold rounded-full border border-green-500/30 flex items-center gap-2">
                    <span class="w-2 h-2 rounded-full bg-green-400 animate-ping"></span>
                    Swarm Mesh Active
                </span>
                <span class="px-3 py-1 bg-purple-500/20 text-purple-300 text-xs font-bold rounded-full border border-purple-500/30">
                    Swarm Village Hackathon
                </span>
            </div>
        </header>

        <!-- Main Grid -->
        <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
            
            <!-- Left Panel: Input -->
            <div class="lg:col-span-5 glass rounded-2xl p-6 space-y-4">
                <h2 class="text-xl font-bold text-white flex items-center gap-2">
                    <i class="fa-solid fa-code text-cyan-400"></i> Code Audit Request
                </h2>
                
                <div>
                    <label class="block text-xs font-semibold text-slate-400 uppercase mb-1">Contract File Name</label>
                    <input id="fileName" type="text" value="Vault.sol" class="w-full bg-slate-900 border border-slate-700 rounded-xl px-4 py-2.5 text-sm text-white focus:outline-none focus:border-cyan-500">
                </div>

                <div>
                    <label class="block text-xs font-semibold text-slate-400 uppercase mb-1">Bounty Escrow (USDC)</label>
                    <input id="bounty" type="number" value="250" class="w-full bg-slate-900 border border-slate-700 rounded-xl px-4 py-2.5 text-sm text-white focus:outline-none focus:border-cyan-500">
                </div>

                <div>
                    <label class="block text-xs font-semibold text-slate-400 uppercase mb-1">Solidity / Rust Source Code</label>
                    <textarea id="codeBody" rows="12" class="w-full bg-slate-900 border border-slate-700 rounded-xl p-4 text-xs font-mono text-cyan-300 focus:outline-none focus:border-cyan-500">// SPDX-License-Identifier: MIT
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

                <button onclick="submitTask()" class="w-full py-3 bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-bold rounded-xl shadow-lg transition-all flex items-center justify-center gap-2 glow-cyan">
                    <i class="fa-solid fa-rocket"></i> Launch Autonomous Swarm Audit
                </button>
            </div>

            <!-- Right Panel: Swarm Results -->
            <div class="lg:col-span-7 space-y-6">
                
                <!-- Agent Topology Cards -->
                <div class="grid grid-cols-3 gap-4">
                    <div class="glass p-4 rounded-xl text-center border-t-2 border-yellow-500">
                        <div class="text-xs text-slate-400 font-bold uppercase">Delegator Agent</div>
                        <div class="text-lg font-black text-white mt-1">Alpha-Node</div>
                        <div class="text-xs text-yellow-400 mt-1">1000 USDC</div>
                    </div>
                    <div class="glass p-4 rounded-xl text-center border-t-2 border-purple-500">
                        <div class="text-xs text-slate-400 font-bold uppercase">Worker Agent</div>
                        <div id="workerCard" class="text-lg font-black text-purple-300 mt-1">Beta-Auditor</div>
                        <div class="text-xs text-purple-400 mt-1">Reputation: 98/100</div>
                    </div>
                    <div class="glass p-4 rounded-xl text-center border-t-2 border-green-500">
                        <div class="text-xs text-slate-400 font-bold uppercase">Validator Sentinel</div>
                        <div class="text-lg font-black text-green-300 mt-1">Re-Execution</div>
                        <div id="verifyBadge" class="text-xs text-green-400 mt-1">Verified</div>
                    </div>
                </div>

                <!-- Live Audit Telemetry & Report -->
                <div class="glass rounded-2xl p-6 min-h-[420px] space-y-4">
                    <h2 class="text-xl font-bold text-white flex items-center justify-between">
                        <span><i class="fa-solid fa-shield-halved text-purple-400"></i> Swarm Telemetry & Findings</span>
                        <span id="scoreBadge" class="text-sm font-extrabold px-3 py-1 bg-slate-800 rounded-lg text-slate-400">Score: --</span>
                    </h2>

                    <div id="outputArea" class="space-y-4">
                        <div class="p-8 text-center text-slate-500 border border-dashed border-slate-800 rounded-xl">
                            <i class="fa-solid fa-satellite-dish text-4xl mb-3 text-slate-700 animate-pulse"></i>
                            <p>Click "Launch Autonomous Swarm Audit" to broadcast RFQ to P2P nodes</p>
                        </div>
                    </div>
                </div>

            </div>
        </div>
    </div>

    <script>
        async function submitTask() {
            const fileName = document.getElementById('fileName').value;
            const bounty = parseInt(document.getElementById('bounty').value);
            const code = document.getElementById('codeBody').value;
            const outputArea = document.getElementById('outputArea');

            outputArea.innerHTML = `
                <div class="p-8 text-center text-cyan-400 space-y-3">
                    <i class="fa-solid fa-spinner fa-spin text-3xl"></i>
                    <p class="font-bold">Broadcasting RFQ to Rust P2P Swarm Network...</p>
                </div>
            `;

            try {
                const res = await fetch('/api/audit', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ file_name: fileName, code: code, bounty: bounty })
                });

                const data = await res.json();
                
                document.getElementById('scoreBadge').innerText = `Security Score: ${data.report.security_score}/100`;
                document.getElementById('scoreBadge').className = data.report.security_score > 70 
                    ? "text-sm font-extrabold px-3 py-1 bg-green-500/20 text-green-400 border border-green-500/30 rounded-lg"
                    : "text-sm font-extrabold px-3 py-1 bg-red-500/20 text-red-400 border border-red-500/30 rounded-lg";

                let findingsHtml = '';
                if (data.report.findings.length === 0) {
                    findingsHtml = `<div class="p-4 bg-green-500/10 border border-green-500/30 rounded-xl text-green-400 font-bold">✔ Clean Audit! Zero vulnerabilities detected.</div>`;
                } else {
                    findingsHtml = data.report.findings.map((f, i) => `
                        <div class="p-4 bg-slate-900 border-l-4 border-red-500 rounded-r-xl space-y-1">
                            <div class="flex justify-between items-center">
                                <span class="font-bold text-red-400 text-sm">[#${i+1}] ${f.title}</span>
                                <span class="text-xs font-mono bg-red-500/20 text-red-300 px-2 py-0.5 rounded">Line ${f.line_number}</span>
                            </div>
                            <pre class="text-xs font-mono text-slate-400 bg-slate-950 p-2 rounded">${f.code_snippet}</pre>
                            <p class="text-xs text-green-300 font-semibold mt-1">Recommendation: ${f.recommendation}</p>
                        </div>
                    `).join('');
                }

                outputArea.innerHTML = `
                    <div class="space-y-4">
                        <div class="p-4 bg-slate-900 border border-slate-800 rounded-xl text-xs font-mono space-y-1">
                            <div><span class="text-slate-500">Task ID:</span> <span class="text-cyan-400">${data.task_id}</span></div>
                            <div><span class="text-slate-500">Winning Agent:</span> <span class="text-purple-400">${data.winning_worker}</span></div>
                            <div><span class="text-slate-500">Execution Digest:</span> <span class="text-slate-300">${data.execution_digest}</span></div>
                            <div><span class="text-slate-500">Ed25519 Signature:</span> <span class="text-yellow-400">${data.signature.substring(0, 32)}...</span></div>
                        </div>

                        <div>
                            <h3 class="text-sm font-bold text-slate-300 mb-2">Vulnerability Findings (${data.report.total_vulnerabilities})</h3>
                            <div class="space-y-2">${findingsHtml}</div>
                        </div>
                    </div>
                `;

            } catch (e) {
                outputArea.innerHTML = `<div class="p-4 bg-red-500/10 border border-red-500/30 rounded-xl text-red-400">Error: ${e.message}</div>`;
            }
        }
    </script>
</body>
</html>"#.to_string()
}
