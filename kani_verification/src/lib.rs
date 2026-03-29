// balddev NFT Staking - Kani Verification Harness Library
//
// All proofs are self-contained pure-Rust logic harnesses.
// No Anchor/Solana runtime types are imported here to keep
// compilation fast and deterministic under the Kani toolchain.

#![cfg(kani)]

pub mod stake_proofs;
