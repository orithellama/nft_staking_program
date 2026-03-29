/**
 * Add/Initialize an NFT Collection for Staking
 *
 * This whitelists a collection by creating its CollectionConfig account with:
 * - Min/max lock durations
 * - Enabled status
 * - Stats tracking
 *
 * Run with: ts-node scripts/add-collection.ts <COLLECTION_MINT>
 *
 * Example: ts-node scripts/add-collection.ts 3Yqemc88mnNhQvUEW4NJMU7R93ap6ZCwLX5HzqJWZMJH
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { BalddevNftStaking } from "../target/types/balddev_nft_staking";
import * as fs from "fs";
import * as path from "path";

const PROGRAM_ID = new PublicKey("7dMir6E96FwiYQQ9mdsL6AKUmgzzrERwqj7mkhthxQgV");
const RPC_URL = process.env.RPC_URL || "https://api.mainnet-beta.solana.com";

// Lock duration limits (in seconds)
const MIN_LOCK_DURATION = 86400; // 1 day
const MAX_LOCK_DURATION = 31536000; // 365 days

async function main() {
  console.log("🚀 Adding NFT Collection to Staking Program...\n");

  // Get collection mint from command line
  const collectionMintArg = process.argv[2];
  if (!collectionMintArg) {
    console.error("❌ Error: Collection mint address required");
    console.log("\nUsage: ts-node scripts/add-collection.ts <COLLECTION_MINT>");
    console.log("Example: ts-node scripts/add-collection.ts 3Yqemc88mnNhQvUEW4NJMU7R93ap6ZCwLX5HzqJWZMJH");
    process.exit(1);
  }

  const collectionMint = new PublicKey(collectionMintArg);
  console.log(`🎨 Collection: ${collectionMint.toBase58()}\n`);

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

  // Derive PDAs
  const [config] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  );

  const [collectionConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection"), collectionMint.toBuffer()],
    program.programId
  );

  console.log(`🔑 Config PDA: ${config.toBase58()}`);
  console.log(`🔑 Collection Config PDA: ${collectionConfig.toBase58()}\n`);

  // Verify program is initialized
  try {
    const configData = await program.account.globalConfig.fetch(config);
    console.log("✅ Program config found");
    console.log(`   Authority: ${configData.authority.toBase58()}`);

    // Verify we're the authority
    if (!configData.authority.equals(walletKeypair.publicKey)) {
      console.error("\n❌ Error: You are not the program authority!");
      console.log(`   Program authority: ${configData.authority.toBase58()}`);
      console.log(`   Your wallet: ${walletKeypair.publicKey.toBase58()}`);
      process.exit(1);
    }
  } catch (err) {
    console.error("\n❌ Error: Program not initialized!");
    console.log("   Run 'ts-node scripts/initialize-program.ts' first");
    process.exit(1);
  }

  // Check if collection already added
  try {
    const existingCollection = await program.account.collectionConfig.fetch(collectionConfig);
    console.log("\n⚠️  Collection already added!");
    console.log(`   Min Lock: ${existingCollection.minLockDuration.toString()} seconds (${existingCollection.minLockDuration.toNumber() / 86400} days)`);
    console.log(`   Max Lock: ${existingCollection.maxLockDuration.toString()} seconds (${existingCollection.maxLockDuration.toNumber() / 86400} days)`);
    console.log(`   Enabled: ${existingCollection.enabled}`);
    console.log(`   Total Staked: ${existingCollection.totalStaked.toString()}`);
    return;
  } catch (err) {
    // Collection not added - proceed
    console.log("✅ Collection not found, proceeding with addition...\n");
  }

  // Collection parameters
  const minLockDuration = new BN(MIN_LOCK_DURATION);
  const maxLockDuration = new BN(MAX_LOCK_DURATION);
  const enabled = true;

  console.log("📝 Collection parameters:");
  console.log(`   Min Lock Duration: ${MIN_LOCK_DURATION} seconds (${MIN_LOCK_DURATION / 86400} days)`);
  console.log(`   Max Lock Duration: ${MAX_LOCK_DURATION} seconds (${MAX_LOCK_DURATION / 86400} days)`);
  console.log(`   Enabled: ${enabled}\n`);

  // Build and send transaction
  console.log("🔨 Building transaction...");

  const tx = await program.methods
    .addCollection(collectionMint, minLockDuration, maxLockDuration, enabled)
    .accounts({
      authority: walletKeypair.publicKey,
    })
    .rpc();

  console.log(`\n✅ Collection added successfully!`);
  console.log(`   Transaction: ${tx}`);
  console.log(`   Explorer: https://solscan.io/tx/${tx}\n`);

  // Fetch and display collection config
  const collectionData = await program.account.collectionConfig.fetch(collectionConfig);
  console.log("📊 Collection config:");
  console.log(`   Collection: ${collectionData.collection.toBase58()}`);
  console.log(`   Min Lock: ${collectionData.minLockDuration.toString()} seconds (${collectionData.minLockDuration.toNumber() / 86400} days)`);
  console.log(`   Max Lock: ${collectionData.maxLockDuration.toString()} seconds (${collectionData.maxLockDuration.toNumber() / 86400} days)`);
  console.log(`   Enabled: ${collectionData.enabled}`);
  console.log(`   Total Staked: ${collectionData.totalStaked.toString()}`);
  console.log(`   Lifetime Stakes: ${collectionData.lifetimeStakes.toString()}`);
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error("\n❌ Error:", err);
    process.exit(1);
  });
