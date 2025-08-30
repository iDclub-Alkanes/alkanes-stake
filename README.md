# Alkanes Staking Protocol

A decentralized staking protocol built on the Alkanes blockchain, enabling users to stake tokens and earn rewards through a sophisticated reward distribution system.

## 🚀 Overview

The Alkanes Staking Protocol consists of two main components:
- **StakingPool**: The main staking contract that manages the overall staking pool
- **StakingVault**: Individual vault contracts that represent user staking positions

## 🏗️ Architecture

### Core Components

#### 1. StakingPool (`stake/`)
The main staking pool contract that handles:
- Pool initialization and configuration
- User staking and unstaking operations
- Reward calculation and distribution
- Pool management and statistics

#### 2. StakingVault (`vault/`)
Individual vault contracts that:
- Represent user staking positions as NFTs
- Handle individual vault operations
- Manage vault-specific metadata and attributes

## 🔧 Features

### StakingPool Features
- **Dynamic Collection Naming**: Automatically generates collection names based on staking token names
- **Time-Limited Rewards**: 7-day (1008 blocks) claim period after staking ends
- **Weight-Based Rewards**: Rewards calculated based on staking amount × staking blocks
- **Early Withdrawal**: Users can unstake before maturity without rewards
- **Reward Redistribution**: Unclaimed rewards are redistributed to other stakers

### StakingVault Features
- **NFT Representation**: Each vault is a unique NFT with metadata
- **Dynamic Naming**: Vault names include collection name and index
- **Owner Authentication**: Only vault owners can perform vault operations
- **Collection Integration**: Vaults integrate with collection contracts for metadata

## 📋 API Reference

### StakingPool Messages

| Opcode | Message | Description |
|---------|---------|-------------|
| 0 | Initialize | Initialize the staking pool with parameters |
| 50 | Stake | Stake tokens into the pool |
| 51 | Unstake | Unstake tokens and claim rewards |
| 80 | Withdraw | Owner withdraws remaining rewards after claim period |
| 99 | GetName | Get collection name |
| 100 | GetSymbol | Get collection symbol |
| 101 | GetTotalSupply | Get total staking count |
| 998 | GetCollectionIdentifier | Get collection identifier |
| 1000 | GetData | Get collection image data |
| 1002 | GetAttributes | Get staking pool attributes |

### StakingVault Messages

| Opcode | Message | Description |
|---------|---------|-------------|
| 0 | Initialize | Initialize a new vault with index |
| 51 | Unstake | Unstake from the vault |
| 99 | GetName | Get vault name |
| 100 | GetSymbol | Get vault symbol |
| 101 | GetTotalSupply | Get vault total supply |
| 998 | GetCollectionIdentifier | Get collection identifier |
| 999 | GetNftIndex | Get vault index |
| 1000 | GetData | Get vault data |
| 1001 | GetContentType | Get content type |
| 1002 | GetAttributes | Get vault attributes |

## 🚀 Getting Started

### Prerequisites
- Rust 1.86.0 or later
- Cargo package manager
- WASM target: `rustup target add wasm32-unknown-unknown`

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd alkanes-stake
```

2. Build the projects:
```bash
# Build StakingPool
cd stake
cargo build --target wasm32-unknown-unknown --release

# Build StakingVault
cd ../vault
cargo build --target wasm32-unknown-unknown --release
```

3. Check compilation:
```bash
cargo check
```

## 📊 Staking Mechanics

### Reward Calculation
Rewards are calculated using a weight-based system:
```
User Weight = Staking Amount × Staking Blocks
Total Weight = Total Staking Amount × Total Staking Blocks
User Reward = Total Reward Pool × (User Weight / Total Weight)
```

### Time Constraints
- **Staking Period**: Configurable start and end blocks
- **Claim Period**: 7 days (1008 blocks) after staking ends
- **Early Withdrawal**: Available before maturity (no rewards)

### Reward Distribution
- Rewards are distributed proportionally based on staking weight
- Unclaimed rewards after the claim period are forfeited
- Early withdrawals redistribute rewards to remaining stakers

## 🔐 Security Features

- **Owner Authentication**: Vault operations require proper authentication
- **Parameter Validation**: All staking parameters are validated
- **Safe Math Operations**: Uses checked arithmetic operations
- **Access Control**: Restricted access to sensitive operations

## 🏗️ Technical Details

### Dependencies
- **alkanes-support**: Core Alkanes blockchain support
- **alkanes-runtime**: Alkanes runtime environment
- **metashrew-support**: Metashrew protocol support
- **anyhow**: Error handling utilities

### Storage Structure
- **Staking Data**: User staking amounts, blocks, and timestamps
- **Pool Statistics**: Total staking amounts, blocks, and rewards
- **Vault Metadata**: Individual vault information and attributes
- **Collection Data**: Dynamic naming and metadata

### Message Handling
- **MessageDispatch**: Automatic message routing based on opcodes
- **AlkaneResponder**: Blockchain interaction interface
- **Cellpack**: Cross-contract communication mechanism

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🔗 Links

- **Alkanes Protocol**: [https://github.com/kungfuflex/alkanes-rs](https://github.com/kungfuflex/alkanes-rs)
- **Metashrew Protocol**: [https://github.com/sandshrewmetaprotocols/metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

## 📞 Support

For questions and support, please open an issue on GitHub or contact the development team.

---

**Note**: This protocol is designed for the Alkanes blockchain ecosystem and integrates with the broader Alkanes and Metashrew protocols.
