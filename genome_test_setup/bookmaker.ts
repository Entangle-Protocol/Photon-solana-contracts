import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Keypair, TransactionSignature } from "@solana/web3.js";
import { BN } from "bn.js";
import { createApproveInstruction } from "@solana/spl-token";

export function getTournamentBookPDA(
  program: Program<Genome>,
  root: Uint8Array,
  tournamentIndex: number
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const tournamentBook = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("BOOK"), tournamentCounterBytes],
    program.programId
  )[0];
  return tournamentBook;
}

export function getCaptainBetPDA(
  program: Program<Genome>,
  root: Uint8Array,
  tournamentIndex: number,
  captain: anchor.web3.PublicKey
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const captainBet = anchor.web3.PublicKey.findProgramAddressSync(
    [
      root,
      utf8.encode("CAPTAIN_BET"),
      tournamentCounterBytes,
      captain.toBuffer(),
    ],
    program.programId
  )[0];
  return captainBet;
}

export function getGamblerInfoPDA(
  program: Program<Genome>,
  root: Uint8Array,
  tournamentIndex: number,
  captain: anchor.web3.PublicKey,
  gambler: anchor.web3.PublicKey
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const gamblerInfo = anchor.web3.PublicKey.findProgramAddressSync(
    [
      root,
      utf8.encode("GAMBLER"),
      tournamentCounterBytes,
      captain.toBuffer(),
      gambler.toBuffer(),
    ],
    program.programId
  )[0];
  return gamblerInfo;
}
