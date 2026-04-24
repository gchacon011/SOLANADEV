# AIHIMERATRADING DeFi — Solana Developer Certification Project

AIHIMERATRADING DeFi is a Solana Anchor backend for the **DeFi Track**. It upgrades the original AIHIMERATRADING AI-agent concept into an on-chain AI trading signal marketplace with CRUD, PDAs, SPL-token staking, vault custody, and rewards.

## Certification Requirements Covered

- Rust Solana program using Anchor
- CRUD + PDA implementation
- Public GitHub-ready structure
- Clear documentation and usage instructions
- DeFi vertical: staking, vaults, rewards, fees, token accounts
- Backend-first implementation, with optional Next.js integration helper

## Project Concept

AIHIMERATRADING publishes AI-generated trading signals. Each signal is an on-chain account. Creators must stake tokens behind the signal, users can stake into a signal, and rewards can be claimed from a protocol treasury after a positive performance score.

This makes the project more than a generic CRUD. It demonstrates DeFi mechanics:

1. SPL token mint and accounts
2. PDA-based protocol state
3. PDA-based token vault per signal
4. User staking positions
5. Reward calculation in basis points
6. Authority checks and lifecycle controls

## Folder Structure

```text
aihimeratrading-defi/
├── Anchor.toml
├── package.json
├── tsconfig.json
├── programs/
│   └── aihimeratrading_defi/
│       ├── Cargo.toml
│       └── src/lib.rs
├── tests/
│   └── aihimeratrading_defi.ts
├── app/
│   └── solana/aihimeratradingDefiClient.ts
└── docs/
    └── DEFI_TRACK_ALIGNMENT.md
```

## On-chain Accounts

### `Protocol`
Global protocol PDA.

Stores:

- authority
- SPL mint
- treasury vault
- reward rate
- platform fee
- signal counter

### `Signal`
AI trading signal PDA.

Stores:

- creator
- symbol, for example `SOL`
- strategy URI, for example IPFS metadata
- rationale hash
- long / short / neutral direction
- confidence score
- total stake
- performance score
- active/inactive status

### `UserPosition`
User position PDA for each user and signal.

Stores:

- owner
- signal
- staked amount
- rewards claimed
- last action timestamp

## Main Instructions

### Initialize Protocol
Creates protocol PDA and treasury token vault.

```rust
initialize_protocol(reward_rate_bps, platform_fee_bps)
```

### Create Signal
Creates the AI trading signal and transfers initial stake into the signal vault.

```rust
create_signal(symbol, strategy_uri, rationale_hash, direction, confidence_bps, initial_stake)
```

### Update Signal
Creator can update metadata and confidence.

```rust
update_signal(strategy_uri, rationale_hash, direction, confidence_bps)
```

### Stake Signal
User stakes SPL tokens into the signal vault.

```rust
stake_signal(amount)
```

### Score Signal
Protocol authority scores the signal performance.

```rust
score_signal(performance_bps)
```

### Claim Rewards
User claims rewards from the treasury when signal performance is positive.

```rust
claim_rewards()
```

### Deactivate Signal
Creator marks the signal inactive.

```rust
deactivate_signal()
```

### Withdraw Stake
User withdraws staked tokens from the signal vault.

```rust
withdraw_stake(amount)
```

## Setup

```bash
npm install
anchor build
anchor test
```

## Add to Existing GitHub Repo

From your local `AIHIMERATRADING` folder:

```bash
cp -R aihimeratrading-defi/* .
git add .
git commit -m "Add Solana DeFi Track Anchor CRUD with SPL staking"
git push origin main
```

Or use it as a separate repo:

```bash
git init
git add .
git commit -m "Initial AIHIMERATRADING DeFi certification project"
git branch -M main
git remote add origin https://github.com/gchacon011/aihimeratrading-defi.git
git push -u origin main
```

## Demo Day Pitch

AIHIMERATRADING DeFi is an AI-powered signal marketplace on Solana. Instead of letting AI trading bots publish unaccountable recommendations, every signal is stored on-chain and backed by stake. Users can stake into strategies, protocol performance can be scored, and rewards are paid from a treasury through SPL-token vaults. The project demonstrates Anchor, Rust, PDAs, token accounts, vault custody, DeFi incentive design, and secure account validation.

## Important Notes

This is a certification-ready educational protocol, not audited financial software. Before mainnet use, it would need formal audits, oracle integration, anti-sybil controls, robust reward accounting, and a real risk model.
