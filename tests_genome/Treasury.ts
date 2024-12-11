import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { Program } from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  getTreasuryAccounts,
} from "../genome_test_setup/treasury";
import { getOperatorInfo, init, approveOperator } from "../genome_test_setup/genome";
import { assert } from "chai";

describe("zs-single-solana", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const ROOT = utf8.encode("genome-root");
  const program = anchor.workspace.Genome as Program<Genome>;
  const admin = anchor.web3.Keypair.generate();
  const operator = anchor.web3.Keypair.generate();
  const notOperator = anchor.web3.Keypair.generate();
  let mint: anchor.web3.PublicKey;
  let operatorVault: anchor.web3.PublicKey;

  before(async () => {
    const provider = anchor.getProvider();
    let tx = await provider.connection.requestAirdrop(
      admin.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);
    tx = await provider.connection.requestAirdrop(
      operator.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);
    tx = await provider.connection.requestAirdrop(
      notOperator.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);

    mint = await token.createMint(
      provider.connection,
      admin,
      admin.publicKey,
      null,
      6
    );
    operatorVault = await token.createAccount(
      provider.connection,
      admin,
      mint,
      operator.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      operatorVault,
      admin.publicKey,
      1000000000
    );
  });

  it("Is initialized!", async () => {
    await init(ROOT, program, admin);
  });
    
  it("Approve operator", async () => {
    const role = {backend:{}};
    await approveOperator(program, ROOT, admin, operator.publicKey, role);
  });

  it("Deposit", async () => {
    const accounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const vault = token.getAssociatedTokenAddressSync(
      mint,
      accounts.authority,
      true
    );
    const operatorAccountBefore = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    await program.methods
      .deposit(new anchor.BN(1000000))
      .accountsStrict({
        operator: operator.publicKey,
        authority: accounts.authority,
        operatorInfo,
        mint,
        vault,
        source: operatorVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();
    const operatorAccountAfter = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    assert.equal(
      Number(operatorAccountAfter.amount),
      Number(operatorAccountBefore.amount) - 1000000
    );
  });

  it("Deposit from unapproved operator", async () => {
    const accounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, notOperator.publicKey);
    const vault = token.getAssociatedTokenAddressSync(
      mint,
      accounts.authority,
      true
    );
    try {
      await program.methods
        .deposit(new anchor.BN(1000000))
        .accountsStrict({
          operator: notOperator.publicKey,
          authority: accounts.authority,
          operatorInfo,
          mint,
          vault,
          source: operatorVault,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([notOperator])
        .rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof anchor.AnchorError);
    }
  });

  it("Withdraw", async () => {
    const accounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const vault = token.getAssociatedTokenAddressSync(
      mint,
      accounts.authority,
      true
    );
    const operatorAccountBefore = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    await program.methods
      .withdraw(new anchor.BN(1000000))
      .accountsStrict({
        operator: operator.publicKey,
        authority: accounts.authority,
        operatorInfo,
        mint,
        vault,
        destination: operatorVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();
    const operatorAccountAfter = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    assert.equal(
      Number(operatorAccountAfter.amount),
      Number(operatorAccountBefore.amount) + 1000000
    );
  });

  it("Withdraw from unapproved operator", async () => {
    const accounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, notOperator.publicKey);
    const vault = token.getAssociatedTokenAddressSync(
      mint,
      accounts.authority,
      true
    );
    try {
      await program.methods
        .withdraw(new anchor.BN(1000000))
        .accountsStrict({
          operator: notOperator.publicKey,
          authority: accounts.authority,
          operatorInfo,
          mint,
          vault,
          destination: operatorVault,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([notOperator])
        .rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof anchor.AnchorError);
    }
  });
});
