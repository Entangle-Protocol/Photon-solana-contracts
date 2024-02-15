import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { getKeypairFromFile } from "@solana-developers/node-helpers";
import fs from "fs";

const EOB_CHAIN_ID = 33133;
const ROOT = utf8.encode("root-0");
const GOV_CONSENSUS_TARGET_RATE = 6000;
const GOV_PROTOCOL_ID = Buffer.from(
  utf8.encode("aggregation-gov_________________"),
);
const KEEPERS = [];
const KEYS_PATH = "./keys";

function initOrLoadGovExecutor(): web3.Keypair {
  const executorPath = `${KEYS_PATH}/executor.json`;
  if (!fs.existsSync(executorPath)) {
    const executor = web3.Keypair.generate();
    fs.writeFileSync(executorPath, JSON.stringify(Array.from(executor.secretKey)));
    return executor;
  } else {
    const decodedKey = new Uint8Array(
      JSON.parse(fs.readFileSync(executorPath).toString()),
    );
    return web3.Keypair.fromSecretKey(decodedKey);
  }
}

function hexToBytes(hex: string): number[] {
  return Array.from(Buffer.from(hex.replace("0x", ""), "hex"));
}

module.exports = async function (provider: anchor.AnchorProvider) {
  console.log("Deploying to devnet");
  process.chdir('..'); // starts in .anchor by default
  if (!fs.existsSync(KEYS_PATH)) {
    fs.mkdirSync(KEYS_PATH);
  }
  anchor.setProvider(provider);
  const admin = await getKeypairFromFile();
  const executor = initOrLoadGovExecutor();
  const program = anchor.workspace.Photon as Program<Photon>;
  const config = web3.PublicKey.findProgramAddressSync(
    [ROOT, utf8.encode("CONFIG")],
    program.programId,
  )[0];
  const protocolInfo = web3.PublicKey.findProgramAddressSync(
    [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
    program.programId,
  )[0];
  await program.methods
    .initialize(
      new anchor.BN(EOB_CHAIN_ID),
      new anchor.BN(GOV_CONSENSUS_TARGET_RATE),
      KEEPERS.map((x) => hexToBytes(x)),
      [executor.publicKey],
    )
    .accounts({
      admin: admin.publicKey,
      protocolInfo,
      config,
      systemProgram: web3.SystemProgram.programId,
    })
    .signers([admin])
    .rpc();
  console.log("Photon program address:", program.programId.toBase58());
};
