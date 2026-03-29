/**
 * Initialize the NFT Staking Program
 *
 * This creates the global config account with:
 * - Authority (your wallet)
 * - Protocol fee (set to 0%)
 * - Total stats tracking
 *
 * Run with: ts-node scripts/initialize-program.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { BalddevNftStaking } from "../target/types/balddev_nft_staking";
import * as fs from "fs";
import * as path from "path";

const PROGRAM_ID = new PublicKey("7dMir6E96FwiYQQ9mdsL6AKUmgzzrERwqj7mkhthxQgV");
const RPC_URL = process.env.RPC_URL || "https://api.mainnet-beta.solana.com";

async function main() {
  console.log("🚀 Initializing NFT Staking Program...\n");

  // Load wallet
  const walletPath = process.env.WALLET_PATH || path.join(process.env.HOME!, ".config/solana/id.json");
  console.log(`📁 Loading wallet from: ${walletPath}`);

  const walletKeypair = Keypair.fromSecretKey(
    Buffer.from(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  console.log(`👛 Authority: ${walletKeypair.publicKey.toBase58()}\n`);

  // Setup connection and provider
  const connection = new Connection(RPC_URL, "confirmed");
  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });

  // Load program
  const idlPath = path.join(__dirname, "../target/idl/balddev_nft_staking.json");
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl, provider) as Program<BalddevNftStaking>;

  console.log(`📦 Program ID: ${program.programId.toBase58()}`);

  // Derive config PDA
  const [config] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  );

  console.log(`🔑 Config PDA: ${config.toBase58()}\n`);

  // Check if already initialized
  try {
    const existingConfig = await program.account.globalConfig.fetch(config);
    console.log("⚠️  Program already initialized!");
    console.log(`   Authority: ${existingConfig.authority.toBase58()}`);
    console.log(`   Total Stakes: ${existingConfig.totalStakes.toString()}`);
    console.log(`   Collection Count: ${existingConfig.collectionCount.toString()}`);
    console.log(`   Protocol Fee: ${existingConfig.protocolFeeBps} bps (${existingConfig.protocolFeeBps / 100}%)`);
    console.log(`   Paused: ${existingConfig.paused}`);
    return;
  } catch (err) {
    // Config doesn't exist - proceed with initialization
    console.log("✅ Config not found, proceeding with initialization...\n");
  }

  // Protocol fee (0 = 0%, 1000 = 10%)
  const protocolFeeBps = 0;

  console.log("📝 Transaction parameters:");
  console.log(`   Authority: ${walletKeypair.publicKey.toBase58()}`);
  console.log(`   Protocol Fee: ${protocolFeeBps} bps (${protocolFeeBps / 100}%)\n`);

  // Build and send transaction
  console.log("🔨 Building transaction...");

  const tx = await program.methods
    .initializeConfig(protocolFeeBps)
    .accounts({
      authority: walletKeypair.publicKey,
    })
    .rpc();

  console.log(`\n✅ Program initialized successfully!`);
  console.log(`   Transaction: ${tx}`);
  console.log(`   Explorer: https://solscan.io/tx/${tx}\n`);

  // Fetch and display config
  const configData = await program.account.globalConfig.fetch(config);
  console.log("📊 Config details:");
  console.log(`   Authority: ${configData.authority.toBase58()}`);
  console.log(`   Protocol Fee: ${configData.protocolFeeBps} bps`);
  console.log(`   Paused: ${configData.paused}`);
  console.log(`   Total Stakes: ${configData.totalStakes.toString()}`);
  console.log(`   Collection Count: ${configData.collectionCount.toString()}`);
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("\n❌ Error:", err);
    process.exit(1);
  });
