# balddev NFT Staking - Kani Verification Suite

Standalone Kani harness crate for `balddev_nft_staking`.

## Quick Start
```bash
cd kani_verification
./verify.sh --list
./verify.sh --quick
./verify.sh --harness prove_lock_enforcement
./verify.sh
```

## What Gets Verified

- Lock period enforcement (cannot unstake before unlock_at)
- Arithmetic safety (no overflows in timestamp calculations)
- Escrow authority uniqueness (per-user collision prevention)
- Access control (owner verification)
- Stats tracking (total_staked consistency)

## Current Inventory
- Total harnesses: Check with `./verify.sh --list`
- All proofs in `src/*_proofs.rs` files

## Installation

```bash
# Install Kani
cargo install --locked kani-verifier
cargo kani setup
```

## Usage

```bash
# List all available proofs
./verify.sh --list

# Run quick verification (critical proofs only)
./verify.sh --quick

# Run specific proof
./verify.sh --harness prove_lock_enforcement

# Run all proofs (may take several minutes)
./verify.sh
```

## Proof Output

Successful proofs are saved to `proofs/` directory with timestamps:
- `kani_quick_proof_YYYYMMDD_HHMMSS.txt` - Quick verification runs
- `kani_full_proof_YYYYMMDD_HHMMSS.txt` - Full verification runs

## Notes

- `cargo kani --list` may not work on all systems
- Use `./verify.sh --list` instead for reliable harness listing
- Each proof is bounded to avoid infinite loops
- Proofs use symbolic inputs to test all possible values
