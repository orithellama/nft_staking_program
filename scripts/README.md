# NFT Staking Program Scripts

Scripts for initializing and managing the balddev NFT Staking program.

## Prerequisites

1. Install dependencies (from project root):
```bash
npm install
# or
yarn install
```

2. Make sure you have `ts-node` installed:
```bash
npm install -g ts-node typescript
```

3. Set up your wallet:
   - Default: `~/.config/solana/id.json`
   - Or set `WALLET_PATH` environment variable

4. (Optional) Set custom RPC:
```bash
export RPC_URL="https://your-rpc-url.com"
```

## Usage

### Step 1: Initialize Program

This creates the global config account (one-time setup):

```bash
cd <project-root>
ts-node scripts/initialize-program.ts
```

**What it does:**
- Creates global config PDA
- Sets you as program authority
- Sets protocol fee to 0%
- Initializes stats counters

**Output:**
```
🚀 Initializing NFT Staking Program...

📁 Loading wallet from: ~/.config/solana/id.json
👛 Authority: x9QDWHuHZFBfvna9hYKV2ax9j7M482muVvX14tfeNAn

📦 Program ID: 7dMir6E96FwiYQQ9mdsL6AKUmgzzrERwqj7mkhthxQgV
🔑 Config PDA: ABC...123

✅ Program initialized successfully!
   Transaction: xyz...
```

### Step 2: Add Collection

This whitelists an NFT collection for staking:

```bash
ts-node scripts/add-collection.ts <COLLECTION_MINT>
```

**Example:**
```bash
ts-node scripts/add-collection.ts 3Yqemc88mnNhQvUEW4NJMU7R93ap6ZCwLX5HzqJWZMJH
```

**What it does:**
- Creates collection config PDA
- Sets lock duration limits (1 day min, 365 days max)
- Enables the collection for staking
- Initializes collection stats

**Output:**
```
🚀 Adding NFT Collection to Staking Program...

🎨 Collection: 3Yqemc88mnNhQvUEW4NJMU7R93ap6ZCwLX5HzqJWZMJH
👛 Authority: x9QDWHuHZFBfvna9hYKV2ax9j7M482muVvX14tfeNAn

📝 Collection parameters:
   Min Lock Duration: 86400 seconds (1 days)
   Max Lock Duration: 31536000 seconds (365 days)
   Enabled: true

✅ Collection added successfully!
   Transaction: xyz...
```

## Configuration

You can modify the collection parameters in `add-collection.ts`:

```typescript
// Lock duration limits (in seconds)
const MIN_LOCK_DURATION = 86400;      // 1 day
const MAX_LOCK_DURATION = 31536000;   // 365 days
```

## Troubleshooting

### "Program not initialized"
Run `initialize-program.ts` first before adding collections.

### "You are not the program authority"
Make sure you're using the wallet that deployed the program or was set as authority during initialization.

### "Collection already added"
The collection is already whitelisted. The script will show its current configuration.

### RPC errors
Try setting a custom RPC with better rate limits:
```bash
export RPC_URL="https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"
```

## Architecture

```
Program Deployed (7dMir6E...)
  ↓
Initialize Program (creates global config)
  ├─ Authority set
  ├─ Protocol fee set
  └─ Stats initialized
  ↓
Add Collection (creates collection config)
  ├─ Lock durations set
  ├─ Collection enabled
  └─ Ready for staking
  ↓
Users Can Stake NFTs
```

## Security Notes

- Only the program authority can initialize the program
- Only the program authority can add collections
- Keep your authority keypair secure
- Consider using a multisig for production
