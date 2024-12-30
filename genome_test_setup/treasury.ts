import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Keypair, PublicKey, TransactionSignature } from "@solana/web3.js";
import { getOperatorInfo } from "./genome";

interface TreasuryAccounts {
  config: anchor.web3.PublicKey;
  authority: anchor.web3.PublicKey;
}

export function getTreasuryAccounts(
  program: Program<Genome>,
  root: Uint8Array
): TreasuryAccounts {
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("CONFIG")],
    program.programId
  )[0];
  const authority = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("AUTHORITY")],
    program.programId
  )[0];
  return { config, authority };
}

export function getUserInfo(
  program: Program<Genome>,
  root: Uint8Array,
  user: anchor.web3.PublicKey
): anchor.web3.PublicKey {
  return anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("USER"), user.toBuffer()],
    program.programId
  )[0];
}

export async function updateClaimableRewards(
  program: Program<Genome>,
  root: Uint8Array,
  operator: Keypair,
  amount: number
) {
  const operatorInfo = getOperatorInfo(program, root, operator.publicKey);
  const setParamsIx = await program.methods
    .updateClaimableRewards(operator.publicKey, new anchor.BN(amount))
    .accounts({
      operator: operator.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
      operatorInfo,
      claimableUserInfo: getUserInfo(program, root, operator.publicKey),
    })
    .instruction();
  await program.provider.sendAndConfirm(
    new anchor.web3.Transaction().add(setParamsIx),
    [operator]
  );
}
