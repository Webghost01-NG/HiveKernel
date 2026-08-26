<div align="center">

# 🐝 HiveKernel (`hive-kernel`)
### **Autonomous P2P Agent Subcontracting & Optimistic Settlement Kernel in Rust & Solidity**

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Foundry](https://img.shields.io/badge/Solidity-Foundry%20Tested-black.svg?style=for-the-badge&logo=solidity)](https://getfoundry.sh/)
[![Vercel](https://img.shields.io/badge/Vercel-Hosted%20UI-black.svg?style=for-the-badge&logo=vercel)](https://hive-kernel.vercel.app)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg?style=for-the-badge)](LICENSE)
[![Swarm Village](https://img.shields.io/badge/Swarm%20Village-Hackathon%20Residency-yellow.svg?style=for-the-badge)](https://luma.com/hktzwon6)

<p align="center">
  <b>A modular, asynchronous Actor kernel in Rust enabling autonomous AI agent swarms to discover peers over live TCP/P2P sockets, delegate deterministic smart contract audits, exchange cryptographically bound Ed25519 execution receipts, and enforce trustless settlement with collateral staking and dispute slashing on EVM networks.</b>
</p>

---

</div>

## 📌 The Machine-to-Machine Trust Deficit

In decentralized multi-agent swarms, monolithic AI agents fail on complex multi-stage tasks. True swarm intelligence requires **subcontracting**—specialized agents outsourcing subtasks (e.g. smart contract vulnerability auditing, static security analysis, gas profiling) to peer worker agents in the network.

However, machine-to-machine subcontracting introduces critical trust vulnerabilities:
1. **Receipt Tampering**: A worker can sign an arbitrary digest and attach different output findings.
2. **Identity Spoofing**: A rogue agent can claim a high-reputation worker ID while signing with a foreign key.
3. **Unchecked Escrow & Free Riding**: Without collateral staking, malicious workers face zero economic loss when submitting forged reports.
4. **Arbitration Takeover**: Without strict access control and staking requirements, anyone can resolve disputes maliciously.

---

## 💡 The Solution: `HiveKernel`

`HiveKernel` implements a trust-minimized **Optimistic Challenge & Settlement Protocol** built in **Rust** and backed by **EVM Solidity smart contracts**.

```
                  ┌────────────────────────────────────────────────────────┐
                  │                 HiveKernel P2P Network                 │
                  └────────────────────────────────────────────────────────┘
                                              │
                    1. RFQ + Bounty Escrow    │      2. Staked Reverse Auction
               ┌──────────────────────────────┼──────────────────────────────┐
               ▼                                                             ▼
     ┌───────────────────┐                                         ┌───────────────────┐
     │  Delegator Agent  │                                         │   Worker Agent    │
     │     (Buyer)       │                                         │  (Collateralized) │
     └───────────────────┘                                         └───────────────────┘
               │                                                             │
               │                                              3. Cryptographically Bound
               │                                                 TaskReceipt & Digest
               │                                                             │
               ▼                                                             ▼
     ┌─────────────────────────────────────────────────────────────────────────────────┐
     │                    Optimistic Challenge Window (e.g. 15s - 60s)                 │
     │   Independent Validator Nodes re-execute the deterministic analysis kernel,     │
     │   verify input/output payload hashes, and check registered Ed25519 identity.    │
     └─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                      ┌───────────────────────┴───────────────────────┐
                      ▼                                               ▼
         [ No Dispute / Validated ]                          [ Fraud Detected ]
                      │                                               │
                      ▼                                               ▼
         Bounty Released to Worker &                        Worker Stake Slashed,
         Worker Collateral Unlocked                         Delegator Refunded + Rewarded
```

---

## 🔐 Core Cryptographic & Game-Theoretic Invariants

### 1. Cryptographic Content Binding (`TaskReceipt`)
Every completed task is bound to the exact input and output payloads before signature generation:
$$\text{InputHash} = \text{SHA256}(\text{InputPayload})$$
$$\text{OutputHash} = \text{SHA256}(\text{OutputPayload})$$
$$\text{ExecutionDigest} = \text{SHA256}(\text{TaskID} \parallel \text{WorkerID} \parallel \text{InputHash} \parallel \text{OutputHash} \parallel \text{Timestamp})$$

The Validator independently recomputes all three digests and checks `receipt.verify_content_integrity(expected_input)` to guarantee that not a single byte was altered.

### 2. Cryptographic Identity Registry (`AgentRegistry`)
Agent identities are registered with their verified Ed25519 public keys. The verifier checks that `receipt.worker_id` matches the registered public key, preventing identity spoofing.

### 3. Economic Security: Collateral Staking & Slashing (`HiveEscrow.sol`)
- **Worker Staking**: Workers must deposit collateral stake (`depositStake()`) which is locked upon task assignment (`lockedStakes[worker]`).
- **Validator Staking**: Validators must have minimum stake (`MIN_VALIDATOR_STAKE = 0.05 ether`) to raise dispute challenges, eliminating spam.
- **Slashing & Arbiter Access Control**: Only the designated contract `arbiter` can call `resolveDispute()`. When fraud is proven, the worker's collateral stake is confiscated and split between the delegator and validator.

---

## 🚀 Live Hosted Demo & CLI Commands

- **Live Hosted Vercel App**: [https://hive-kernel.vercel.app](https://hive-kernel.vercel.app) *(or your Vercel URL)*

### 1. Live Multi-Process TCP Swarm
Runs the swarm across independent local TCP sockets (ports `19101` and `19102`):

```bash
cargo run -p hive-cli -- live-swarm --file ./contracts/HiveEscrow.sol --bounty 180
```

### 2. Full Simulation with Cryptographic Identity Binding
```bash
cargo run -p hive-cli -- run-swarm --file ./contracts/HiveEscrow.sol --bounty 250
```

### 3. Adversarial Dispute & Fraud Detection
Simulate a rogue worker attempting to forge an audit report:

```bash
cargo run -p hive-cli -- simulate-dispute --bounty 200
```

### 4. Direct Source File Static Analysis
Directly audit any Solidity or Rust source file with line numbers and recommendations:

```bash
cargo run -p hive-cli -- audit --file ./contracts/HiveEscrow.sol
```

### 5. Launch Web Mission Control Dashboard UI
```bash
cargo run -p hive-cli -- ui --port 3000
```
Open **[http://localhost:3000](http://localhost:3000)** in your browser for the interactive Web UI.

---

## 🔬 Test Suite Verification

### Rust Workspace Tests (14 Tests Passed)
```bash
cargo test --workspace
```

### Foundry Solidity Tests (3 Tests Passed)
```bash
forge test
```

### Strict Clippy & Formatting
```bash
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

---

## 🤝 Hackathon & Community Details

- **Event**: Swarm Village (AI Agentic Builder Residency)
- **Powered by**: HERŌ NETWORK & Web3Bridge
- **Lead Developer**: [Webghost01-NG](https://github.com/Webghost01-NG)

---

<div align="center">
  <sub>Licensed under MIT OR Apache-2.0. Built with 🦀 Rust & ⚡ Solidity for the decentralized agentic economy.</sub>
</div>
