import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Keypair, TransactionSignature } from "@solana/web3.js";
import { BN } from "bn.js";

export function getFeeMeta(
  program: Program<Genome>,
  root: Uint8Array,
  feeType: number
): anchor.web3.PublicKey {
  //const feeTypeBytes = new BN(feeType).toArrayLike(Buffer, 'le', 8);
  const feeTypeBytes = Buffer.alloc(2); // Aseguramos que sean exactamente 2 bytes
  feeTypeBytes.writeUInt16LE(feeType, 0);

  return anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("FEE_META"), feeTypeBytes],
    program.programId
  )[0];
}
