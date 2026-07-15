# Garanzi - Technical Documentation & Architecture

This document provides a comprehensive technical overview of Garanzi's architecture, deployment details, and guide on how to run the project locally.

---

## 🔒 Smart Contract Deployment Details

Garanzi utilizes custom-built Soroban smart contracts written in Rust to manage secure resource allocations, escrow lockups, and automatic multi-wallet release parameters.

## 📜 Soroban Smart Contract Details

*   **Network:** Stellar Mainnet
*   **Deployed Contract Address:** `[CB2FVWIMWHIYI73PS4CFTYSPNBQNJBZUOCFLKRWSCKMLFNQ4I2OAFVFH`
 
### 💳 Authorized Deployer Wallet
* **Public Stellar Address:** `GA6AMZ3TA5ICTV2DQ55M2OZHHZLCEBYNV2DXDNVI3N6IM4RRC4XQRWJN `
* *Note: This wallet address holds the signature profile responsible for smart contract execution, resource fee allocation simulations, and initial state initialization.*

---

## ⏳ Business Logic & State Machine

### One-Time Payment State Transitions
1. **`In Progress`**: The client deploys the contract and deposits funds into the escrow pool. Freelancers begin work.
2. **`Under Review`**: Freelancers submit project deliverables, pausing the main deadline and initiating the **48-Hour Auto-Disperse Countdown**.
3. **`Completed - Released`**: The client manually reviews and approves the submission, releasing funds immediately.
4. **`Auto Disperse`**: If the 48-hour window expires without client feedback or dispute initiation, a backend cron/smart-contract trigger dynamically updates the contract status to `Auto Disperse` and processes cryptographic multi-wallet asset routing.

---

## 📦 Local Installation & Setup

Follow these steps to set up and run Garanzi on your local environment:

### Prerequisites
* A modern web browser with a Stellar-compatible wallet extension (e.g., Freighter Wallet).
* A local development server or Node.js environment.

# Clone the repository
git clone https://github.com/YOUR_GITHUB_USERNAME/Garanzi.git
# Move into the project directory
cd Garanzi
# Start the local server
npx serve .

________________________________
GaranZ System Overview
Core Modules:
Home: Dashboard for quick actions (New Deal, QR Pay Links, Scan, Withdraw, Deposit).
Deals: Module for searching and initiating contracts between freelancers and clients.
Messages: Secure channel for project communication and payment release.
Passport: User credential and profile verification hub.
Settings: Personalization and system configuration.
System Workflow:
1) Initialization: User logs in and navigates to the 'Deals' section.
2) Contract Creation: Initiate a new deal; awaits freelancer approval.
3) Execution: Upon approval, the deal moves to 'Messages' for task collaboration.
4) Completion: Freelancer submits work; Client reviews and approves to release funds.
Safeguards:
1) Auto-Release: If the countdown ends, funds are auto-released after 48 hours.
2) Dispute System: Allows freelancers to request deadline extensions or additional payments if disagreements occur.

> [!IMPORTANT]
> These are **test accounts only**. Do not use real personal data or sensitive information when performing these tests.

| Role | Username | Password |
| :--- | :--- | :--- |
| **Client** | cath123 | cath123 |
| **Freelancer** | marielle123 | marielle123 |
