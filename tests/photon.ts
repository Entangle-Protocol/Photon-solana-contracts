import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";
import { Onefunc } from "../target/types/onefunc";
import {
  createMint,
  createAccount,
  TOKEN_PROGRAM_ID,
  mintTo,
} from "@solana/spl-token";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  addAllowedProtocolAddress,
  addExecutor,
  addAllowedProtocol,
  hexToBytes,
  opHashFull,
  randomSigners,
  signOp,
  addKeepers,
  setConsensusTargetRate,
} from "./utils";
import { Wallet } from "ethers";

const TEST_REMOVE_FUNCS = true;
const ROOT = utf8.encode("root-0");
const EOB_CHAIN_ID = 33133;
const SOLANA_CHAIN_ID = 111111111;
const CONSENSUS_TARGET_RATE = 10000;
const KEEPERS = 16;
const KEEPERS_PER_CALL = 4;
const GOV_PROTOCOL_ID = Buffer.from(
  utf8.encode("aggregation-gov_________________"),
);
const ONE_FUNC_ID = Buffer.from(
  utf8.encode("onefunc_________________________"),
);

describe("photon", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Photon as Program<Photon>;
  const onefunc = anchor.workspace.Onefunc as Program<Onefunc>;

  const owner = anchor.web3.Keypair.generate();
  const executor = anchor.web3.Keypair.generate();
  const feeCollector = anchor.web3.Keypair.generate();

  let nglMint;
  let executorNglVault;
  let feeCollectorVault;
  let config;
  let govProtocolInfo;
  let counter;
  let callAuthority;
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
    await mintTo(
      program.provider.connection,
      owner,
      nglMint,
      executorNglVault,
      owner.publicKey,
      100000000,
    );
    config = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("CONFIG")],
      program.programId,
    )[0];
    govProtocolInfo = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
      program.programId,
    )[0];
    keepers = randomSigners(KEEPERS);
    for (var i = 0; i < keepers.length; i++) {
      console.log("Keeper", i, keepers[i].address);
      keepersRaw.push(hexToBytes(keepers[i].address));
    }
    callAuthority = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("CALL_AUTHORITY"), ONE_FUNC_ID],
      program.programId,
    )[0];
    counter = web3.PublicKey.findProgramAddressSync(
      [utf8.encode("COUNTER")],
      onefunc.programId,
    )[0];
  });

  async function executeProposal(
    protocolId: Buffer,
    protocolAddr: anchor.web3.PublicKey,
    functionSelector: number,
    params: Buffer,
    targetProtocol: Buffer,
    remainingAccounts?: anchor.web3.AccountMeta[],
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
    let protocolInfo = web3.PublicKey.findProgramAddressSync(
      [ROOT, utf8.encode("PROTOCOL"), op.protocolId],
      program.programId,
    )[0];
    let signatures = [];
    for (var i = 0; i < keepers.length; i++) {
      const sig = await signOp(keepers[i], op);
      signatures.push(sig);
    }
    // Load
    await program.methods
      .loadOperation(op, op_hash)
      .accounts({
        executor: executor.publicKey,
        protocolInfo,
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
    // Sign
    const chunkSize = KEEPERS_PER_CALL;
    for (let i = 0; i < signatures.length; i += chunkSize) {
      const chunk = signatures.slice(i, i + chunkSize);
      await program.methods
        .signOperation(op_hash, chunk)
        .accounts({
          executor: executor.publicKey,
          opInfo,
          protocolInfo,
        })
        .signers([executor])
        .rpc();
    }
    // Execute
    if (protocolId == GOV_PROTOCOL_ID) {
      await program.methods
        .executeGovOperation(op_hash, targetProtocol)
        .accounts({
          executor: executor.publicKey,
          opInfo,
          govInfo: govProtocolInfo,
          protocolInfo: web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), targetProtocol],
            program.programId,
          )[0],
          systemProgram: web3.SystemProgram.programId,
        })
        .signers([executor])
        .rpc();
    } else {
      await program.methods
        .executeOperation(op_hash)
        .accounts({
          executor: executor.publicKey,
          opInfo,
          protocolInfo,
        })
        .signers([executor])
        .remainingAccounts(remainingAccounts)
        .rpc();
    }
    console.log("Proposal", nonce, "executed");
    nonce++;
  }

  it("Initialize", async () => {
    await onefunc.methods
      .initialize()
      .accounts({ owner: owner.publicKey, callAuthority, counter })
      .signers([owner])
      .rpc();
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
      .initGovProtocol(
        new anchor.BN(CONSENSUS_TARGET_RATE),
        [keepersRaw[0]],
        [executor.publicKey],
      )
      .accounts({
        admin: owner.publicKey,
        protocolInfo: govProtocolInfo,
        config,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([owner])
      .rpc();
    const chunkSize = KEEPERS_PER_CALL;
    for (let i = 1; i < keepersRaw.length; i += chunkSize) {
      const chunk = keepersRaw.slice(i, i + chunkSize);
      let params = addKeepers(GOV_PROTOCOL_ID, chunk);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xa8da4c51,
        params,
        GOV_PROTOCOL_ID,
      );
    }
  });

  it("addAllowedProtocol", async () => {
    let params = addAllowedProtocol(ONE_FUNC_ID, [], CONSENSUS_TARGET_RATE);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0x45a004b9,
      params,
      ONE_FUNC_ID,
    );
  });

  it("setConsensusTargetRate", async () => {
    let params = setConsensusTargetRate(ONE_FUNC_ID, 6000);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0x970b6109,
      params,
      ONE_FUNC_ID,
    );
  });

  it("setProtocolFee", async () => {
    let params = setConsensusTargetRate(ONE_FUNC_ID, 0);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0xafe50cc2,
      params,
      ONE_FUNC_ID,
    );
  });

  it("addAllowedProtocolAddress", async () => {
    if (TEST_REMOVE_FUNCS) {
      let addr = anchor.web3.Keypair.generate().publicKey;
      let params = addAllowedProtocolAddress(ONE_FUNC_ID, addr);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xd296a0ff,
        params,
        ONE_FUNC_ID,
      );
      // removeAllowedProtocolAddress(bytes)
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xb0a4ca98,
        params,
        ONE_FUNC_ID,
      );
    }
    let params = addAllowedProtocolAddress(ONE_FUNC_ID, onefunc.programId);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0xd296a0ff,
      params,
      ONE_FUNC_ID,
    );
  });

  it("addExecutor", async () => {
    if (TEST_REMOVE_FUNCS) {
      let addr = anchor.web3.Keypair.generate().publicKey;
      let params = addExecutor(ONE_FUNC_ID, addr);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xe0aafb68,
        params,
        ONE_FUNC_ID,
      );
      // removeExecutor(bytes)
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0x04fa384a,
        params,
        ONE_FUNC_ID,
      );
    }
    let params = addExecutor(ONE_FUNC_ID, executor.publicKey);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0xe0aafb68,
      params,
      ONE_FUNC_ID,
    );
  });

  it("addProposer", async () => {
    if (TEST_REMOVE_FUNCS) {
      let addr = anchor.web3.Keypair.generate().publicKey;
      let params = addExecutor(ONE_FUNC_ID, addr);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xce0940a5,
        params,
        ONE_FUNC_ID,
      );
      // removeAllowedProposerAddress(bytes)
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xb8e5f3f4,
        params,
        ONE_FUNC_ID,
      );
    }
    let params = addExecutor(ONE_FUNC_ID, onefunc.programId);
    await executeProposal(
      GOV_PROTOCOL_ID,
      program.programId,
      0xce0940a5,
      params,
      ONE_FUNC_ID,
    );
  });

  it("addKeepers", async () => {
    if (TEST_REMOVE_FUNCS) {
      let keepers2 = randomSigners(3);
      let keepersRaw2 = [];
      for (var i = 0; i < keepers2.length; i++) {
        keepersRaw2.push(hexToBytes(keepers2[i].address));
      }
      let params = addKeepers(ONE_FUNC_ID, keepersRaw2);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xa8da4c51,
        params,
        ONE_FUNC_ID,
      );
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0x80936851,
        params,
        ONE_FUNC_ID,
      );
    }
    const chunkSize = KEEPERS_PER_CALL;
    for (let i = 0; i < keepersRaw.length; i += chunkSize) {
      const chunk = keepersRaw.slice(i, i + chunkSize);
      let params = addKeepers(ONE_FUNC_ID, chunk);
      await executeProposal(
        GOV_PROTOCOL_ID,
        program.programId,
        0xa8da4c51,
        params,
        ONE_FUNC_ID,
      );
    }
  });

  it("executeOperation", async () => {
    let instr = await onefunc.methods
      .increment()
      .accounts({ callAuthority, counter })
      .instruction();
    let params = instr.data;
    let keys = instr.keys;
    keys[0].isSigner = false;
    await executeProposal(
      ONE_FUNC_ID,
      onefunc.programId,
      0,
      params,
      null,
      [
        { pubkey: onefunc.programId, isSigner: false, isWritable: false },
      ].concat(keys),
    );
  });
});
