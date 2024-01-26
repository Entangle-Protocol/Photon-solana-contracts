import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";
import { createMint, createAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  addProtocolPayload,
  hexToBytes,
  opHashFull,
  randomSigners,
  signOp,
  sleep,
} from "./utils";
import { SignerWithAddress } from "@nomiclabs/hardhat-ethers/signers";
import { Wallet } from "ethers";

const ROOT = utf8.encode("root-0");
const EOB_CHAIN_ID = 33133;
const SOLANA_CHAIN_ID = 111111111;
const CONSENSUS_TARGET_RATE = 6000;
const GOV_PROTOCOL_ID = Buffer.from(utf8.encode("aggregation-gov_________________"));

describe("photon", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Photon as Program<Photon>;

  const owner = anchor.web3.Keypair.generate();
  const executor = anchor.web3.Keypair.generate();
  const feeCollector = anchor.web3.Keypair.generate();

  let nglMint;
  let executorNglVault;
  let feeCollectorVault;
  let config;
  let govProtocolInfo;
  let keepers: Wallet[];
  let keepersRaw = [];
  let nonce = 0;

  before(async () => {
    let tx = await program.provider.connection.requestAirdrop(
      owner.publicKey,
      anchor.web3.LAMPORTS_PER_SOL,
    );
    await program.provider.connection.confirmTransaction(tx);
    tx = await program.provider.connection.requestAirdrop(
      executor.publicKey,
      anchor.web3.LAMPORTS_PER_SOL,
    );
    await program.provider.connection.confirmTransaction(tx);
    nglMint = await createMint(
      program.provider.connection,
      owner,
      owner.publicKey,
      null,
      9,
    );
    executorNglVault = await createAccount(
      program.provider.connection,
      executor,
      nglMint,
      executor.publicKey,
    );
    feeCollectorVault = await createAccount(
      program.provider.connection,
      owner,
      nglMint,
      feeCollector.publicKey,
    );
    config = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("CONFIG")],
      program.programId,
    )[0];
    govProtocolInfo = web3.PublicKey.findProgramAddressSync(
      [
        ROOT,
        utf8.encode("PROTOCOL"),
        GOV_PROTOCOL_ID,
      ],
      program.programId,
    )[0];
    keepers = randomSigners(1);
    for (var i = 0; i < keepers.length; i++) {
      keepersRaw.push(hexToBytes(keepers[i].address));
    }
  });

  async function executeProposal(
    protocolId: Buffer,
    protocolAddr: anchor.web3.PublicKey,
    functionSelector: number,
    params: Buffer,
    target_protocol: Buffer,
  ) {
    let functionSelectorBuf = Buffer.alloc(4);
    functionSelectorBuf.writeUInt32BE(functionSelector);
    let op = {
      protocolId,
      srcChainId: new anchor.BN(EOB_CHAIN_ID),
      srcBlockNumber: new anchor.BN(1),
      srcOpTxId: hexToBytes(
        "ce25f58a7fd8625deadc00a59b67c530c7d92acec1e5753c588269ade6ebf99f",
      ),
      nonce: new anchor.BN(nonce),
      destChainId: new anchor.BN(SOLANA_CHAIN_ID),
      protocolAddr,
      functionSelector: Array.from(functionSelectorBuf),
      params,
    };
    let op_hash = opHashFull(op);
    let opInfo = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("OP"), op_hash],
      program.programId,
    )[0];
    let signatures = [];
    for (var i = 0; i < keepers.length; i++) {
      signatures.push(await signOp(keepers[i], op));
    }
    let pubkeys = [];
    for (var i = 0; i < keepers.length; i++) {
      pubkeys.push(hexToBytes(keepers[i].publicKey));
    }
    await program.methods
      .loadOperation(op, op_hash, signatures, pubkeys)
      .accounts({
        executor: executor.publicKey,
        protocolInfo: govProtocolInfo,
        opInfo,
        nglMint,
        executorNglVault,
        feeCollectorVault,
        config,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([executor])
      .rpc();
    let callAuthority = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("CALL_AUTHORITY")],
      program.programId,
    )[0];
    if(protocolId == GOV_PROTOCOL_ID) {
      await program.methods
      .executeGovOperation(op, op_hash, target_protocol)
      .accounts({
        executor: executor.publicKey,
        protocolInfo: web3.PublicKey.findProgramAddressSync(
          [
            ROOT,
            utf8.encode("PROTOCOL"),
            target_protocol,
          ],
          program.programId,
        )[0],
        opInfo,
        systemProgram: web3.SystemProgram.programId
      })
      .signers([executor])
      .rpc();
    } else {
      await program.methods
      .executeOperation(op, op_hash)
      .accounts({
        executor: executor.publicKey,
        callAuthority,
        opInfo,
      })
      .signers([executor])
      .rpc();
    }
    console.log("Proposal", nonce, "executed");
    nonce++;
  }

  it("Initialize", async () => {
    //await sleep(5000);
    await program.methods
      .initialize(new anchor.BN(EOB_CHAIN_ID))
      .accounts({
        owner: owner.publicKey,
        nglMint,
        feeCollectorVault,
        config,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([owner])
      .rpc();
  });

  it("InitGovProtocol", async () => {
    await program.methods
      .initGovProtocol(new anchor.BN(CONSENSUS_TARGET_RATE), keepersRaw, [
        executor.publicKey,
      ])
      .accounts({
        admin: owner.publicKey,
        protocolInfo: govProtocolInfo,
        config,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([owner])
      .rpc();
  });

  it("Add Protocol", async () => {
    let protocolId = Buffer.from(utf8.encode("aggregation-gov2________________"));
    let params = addProtocolPayload(
      protocolId,
      program.programId,
      keepersRaw,
    );
    await executeProposal(GOV_PROTOCOL_ID, program.programId, 0x45a004b9, params, protocolId);
  });
});

