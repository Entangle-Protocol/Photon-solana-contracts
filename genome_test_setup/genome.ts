import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Keypair, PublicKey, TransactionSignature } from "@solana/web3.js";
import { BN } from "bn.js";
import { createApproveInstruction } from "@solana/spl-token";

interface GenomeAccounts {
  config: anchor.web3.PublicKey;
}

export function getGenomeAccounts(
  program: Program<Genome>,
  root: Uint8Array
): GenomeAccounts {
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("CONFIG")],
    program.programId
  )[0];
  return { config };
}

export function getOperatorInfo(
  program: Program<Genome>,
  root: Uint8Array,
  operator: anchor.web3.PublicKey
): anchor.web3.PublicKey {
  return anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("OPERATOR"), operator.toBuffer()],
    program.programId
  )[0];
}

export async function initIx(
  root: Uint8Array,
  program: Program<Genome>,
  admin: anchor.web3.Keypair
): Promise<{
  ix: anchor.web3.TransactionInstruction;
  operatorInfo: PublicKey;
}> {
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("CONFIG")],
    program.programId
  )[0];
  const operatorInfo = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("OPERATOR"), admin.publicKey.toBuffer()],
    program.programId
  )[0];
  const ix = await program.methods
    .initialize()
    .accounts({
      admin: admin.publicKey,
      config,
      operatorInfo,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .instruction();
  return { ix, operatorInfo };
}

export async function init(
  root: Uint8Array,
  program: Program<Genome>,
  admin: anchor.web3.Keypair
): Promise<{ txSignature: TransactionSignature; operatorInfo: PublicKey }> {
  const { ix, operatorInfo } = await initIx(root, program, admin);
  const tx = new anchor.web3.Transaction().add(ix);
  const txResult = await program.provider.sendAndConfirm(tx, [admin]);
  return { txSignature: txResult, operatorInfo };
}

export async function approveOperatorIx(
  program: Program<Genome>,
  root: Uint8Array,
  admin: anchor.web3.Keypair,
  operator: anchor.web3.PublicKey,
  role: any
): Promise<anchor.web3.TransactionInstruction> {
  const accounts = getGenomeAccounts(program, root);
  const operatorInfo = getOperatorInfo(program, root, operator);
  const ix = await program.methods
    .approveOperator(role)
    .accountsStrict({
      admin: admin.publicKey,
      config: accounts.config,
      operator,
      operatorInfo,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .instruction();
  return ix;
}

export async function approveOperator(
  program: Program<Genome>,
  root: Uint8Array,
  admin: anchor.web3.Keypair,
  operator: anchor.web3.PublicKey,
  role: any
): Promise<TransactionSignature> {
  const ix = await approveOperatorIx(program, root, admin, operator, role);
  const tx = new anchor.web3.Transaction().add(ix);
  return await program.provider.sendAndConfirm(tx, [admin]);
}

export async function buildAndSendApproveTransaction(
  provider: anchor.Provider,
  account: anchor.web3.PublicKey,
  delegate: anchor.web3.PublicKey,
  owner: anchor.web3.Keypair,
  amount: number
): Promise<TransactionSignature> {
  const tx = new anchor.web3.Transaction().add(
    createApproveInstruction(account, delegate, owner.publicKey, amount)
  );
  return await provider.sendAndConfirm(tx, [owner]);
}
