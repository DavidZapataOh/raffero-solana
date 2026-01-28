import { address } from "@solana/kit";
import path from "path";
import "dotenv/config";
import {
  type CircuitConfig,
  generateProofWithInputs,
  createInstructionData,
} from "@solana-noir-examples/lib/proof";
import {
  verifyOnChain,
  printTransactionResult,
  handleVerifyError,
} from "@solana-noir-examples/lib/verify";

const RPC_URL = process.env.RPC_URL || "https://api.devnet.solana.com";

// NOTE: Deploy your own verifier via `sunspot deploy` and set PROGRAM_ID
const PROGRAM_ID = process.env.PROGRAM_ID || "YOUR_DEPLOYED_VERIFIER_PROGRAM_ID";

const MAX_DEPTH = 10;

const circuitConfig: CircuitConfig = {
  circuitDir: path.join(process.cwd(), ".."),
  circuitName: "claim",
};

const walletPath = path.join(
  circuitConfig.circuitDir,
  "keypair",
  "deployer.json"
);

/**
 * Example claim proof inputs for testing.
 * In production, these would come from your backend/smart contract.
 */
function generateTestInputs() {
  // Private inputs
  const secret = "123";
  const nullifier = "456";
  const recipient = "789";
  
  // For testing: create a simple 2-level tree
  // In production, these would be computed from actual Merkle tree data
  const siblings: string[] = [];
  for (let i = 0; i < MAX_DEPTH; i++) {
    siblings.push("0");
  }
  siblings[0] = "12345"; // First sibling for testing
  
  const path_indices: string[] = [];
  for (let i = 0; i < MAX_DEPTH; i++) {
    path_indices.push("0");
  }
  
  // Public inputs (these would normally be computed/verified)
  // For testing, we use placeholder values - these need to match!
  const root = "11416054470674754408325678912556888274328835808560862083112001173504341785622"; // Placeholder - needs to be computed from actual tree
  const nullifier_hash = "6717609415510369431648265157427190921578662879631220533561778315995839230701"; // Placeholder - hash2(nullifier, raffle_id)
  const recipient_binding = "21728063333761709356534029358792426533779764497506631048703304463536088229762"; // Placeholder - hash2(nullifier_hash, recipient)
  const raffle_id = "1";
  const winner_index = "0";
  const tree_depth = "2";

  return {
    // Private inputs
    secret,
    nullifier,
    siblings,
    path_indices,
    recipient,
    // Public inputs
    root,
    nullifier_hash,
    recipient_binding,
    raffle_id,
    winner_index,
    tree_depth,
  };
}

async function main() {
  console.log("Claim Circuit - Solana ZK Verifier Client\n");
  console.log("Circuit: Merkle proof verification for raffle winner claim\n");

  const args = process.argv.slice(2);
  const corrupt = args.includes("--corrupt");

  try {
    console.log("Generating claim proof with test inputs...\n");

    const inputs = generateTestInputs();
    console.log(`Tree depth: ${inputs.tree_depth}`);
    console.log(`Winner index: ${inputs.winner_index}`);
    console.log(`Raffle ID: ${inputs.raffle_id}\n`);

    const proofResult = generateProofWithInputs(circuitConfig, inputs);

    if (corrupt) {
      console.log("⚠️  CORRUPTING PROOF FOR TESTING\n");
      proofResult.proof[0] ^= 0xff;
    }

    console.log(`Proof size: ${proofResult.proof.length} bytes`);
    console.log(`Witness size: ${proofResult.publicWitness.length} bytes`);

    const instructionData = createInstructionData(proofResult);
    console.log(`Total instruction data: ${instructionData.length} bytes\n`);

    const sig = await verifyOnChain(instructionData, {
      rpcUrl: RPC_URL,
      programId: address(PROGRAM_ID),
      walletPath,
    });

    console.log("\n✅ Claim proof verified successfully on-chain!");
    printTransactionResult(sig);
  } catch (err) {
    handleVerifyError(err);
  }
}

main();