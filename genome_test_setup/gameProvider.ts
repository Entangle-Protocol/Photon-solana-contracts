import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Keypair, TransactionSignature } from "@solana/web3.js";
import { BN } from "bn.js";
import { createApproveInstruction } from "@solana/spl-token";

export function getGamePDA(
  program: Program<Genome>,
  root: Uint8Array,
  gameIndex: number
): anchor.web3.PublicKey {
  const gameCounterBytes = new BN(gameIndex).toArrayLike(Buffer, "le", 8);
  const game = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("GAME"), gameCounterBytes],
    program.programId
  )[0];
  return game;
}

export function getParticipantPDA(
  program: Program<Genome>,
  root: Uint8Array,
  participant: anchor.web3.PublicKey,
  gameIndex: number
): anchor.web3.PublicKey {
  const gameCounterBytes = new BN(gameIndex).toArrayLike(Buffer, "le", 8);
  return anchor.web3.PublicKey.findProgramAddressSync(
    [
      root,
      utf8.encode("PARTICIPANT"),
      participant.toBuffer(),
      gameCounterBytes,
    ],
    program.programId
  )[0];
}
