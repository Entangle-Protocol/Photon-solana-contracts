import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Genome } from "../target/types/genome";
import { BN } from "bn.js";
import { getOperatorInfo, getGenomeAccounts } from "./genome";
import { getUserInfo } from "./treasury";
import { getFeeMeta } from "./feeProvider";
const ROOT = utf8.encode("genome-root");
export function getTeamPDA(
  program: Program<Genome>,
  root: Uint8Array,
  tournamentIndex: number,
  captain: PublicKey
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const tournament = anchor.web3.PublicKey.findProgramAddressSync(
    [root, utf8.encode("TEAM"), tournamentCounterBytes, captain.toBuffer()],
    program.programId
  )[0];

  return tournament;
}

export function getTeamParticipantPDA(
  program: Program<Genome>,
  root: Uint8Array,
  tournamentIndex: number,
  participant: PublicKey
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const teamParticipantPDA = anchor.web3.PublicKey.findProgramAddressSync(
    [
      root,
      utf8.encode("TEAM_PARTICIPANT"),
      tournamentCounterBytes,
      participant.toBuffer(),
    ],
    program.programId
  )[0];

  return teamParticipantPDA;
}

export interface TournamentParams {
  fee: anchor.BN;
  sponsorPool: anchor.BN;
  startTime: anchor.BN;
  playersInTeam: number;
  minTeams: number;
  maxTeams: number;
  organizerRoyalty: number;
  token: PublicKey;
}

export async function setTournamentParams(
  program: Program<Genome>,
  admin: Keypair,
  minimalAdmisionFee: number,
  minimalSponsorPool: number,
  operator: PublicKey
) {
  const accounts = getGenomeAccounts(program, ROOT);
  const setParamsIx = await program.methods
    .setTournamentParams(
      new anchor.BN(minimalAdmisionFee),
      new anchor.BN(minimalSponsorPool)
    )
    .accounts({
      admin: admin.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
      operatorInfo: operator,
      config: accounts.config,
    })
    .instruction();
  await program.provider.sendAndConfirm(
    new anchor.web3.Transaction().add(setParamsIx),
    [admin]
  );
}

export function getTournamentPDA(
  program: Program<Genome>,
  tournamentIndex: number
): anchor.web3.PublicKey {
  const tournamentCounterBytes = new BN(tournamentIndex).toArrayLike(
    Buffer,
    "le",
    8
  );
  const tournament = anchor.web3.PublicKey.findProgramAddressSync(
    [ROOT, utf8.encode("TOURNAMENT"), tournamentCounterBytes],
    program.programId
  )[0];
  return tournament;
}

export const initializeOrGetUserVault = async (
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey,
  initialTokens: number = 1000000000
) => {
  const vault = await token.getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    mint,
    owner,
    true
  );
  // if (initialTokens > 0) {
  //   await token.mintTo(
  //     connection,
  //     payer,
  //     mint,
  //     vault.address,
  //     owner,
  //     initialTokens
  //   );
  // }
  return vault;
};

export async function createTournamentSinglechain(
  program: Program<Genome>,
  admin: anchor.web3.Keypair,
  tournamentIndex: number,
  params: any,
  mint: PublicKey,
  operatorInfo,
  organizer = admin.publicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const sponsorVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    mint,
    admin.publicKey
  );

  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    mint,
    tournament,
    0
  );

  const accounts = getGenomeAccounts(program, ROOT);
  const ix = await program.methods
    .createTournamentSinglechain(organizer, params)
    .accountsStrict({
      sponsor: admin.publicKey,
      config: accounts.config,
      sponsorVault: sponsorVault.address,
      tournament,
      operatorInfo,
      mint,
      tournamentVault: tournamentVault.address,
      associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
    })
    .signers([admin]);

  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function createTournamentOmnichain(
  program: Program<Genome>,
  admin: anchor.web3.Keypair,
  tournamentIndex: number,
  params: any,
  operatorInfo,
  organizer = admin.publicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);

  const accounts = getGenomeAccounts(program, ROOT);
  const claimableUserInfo = getUserInfo(program, ROOT, admin.publicKey);
  const ix = await program.methods
    .createTournamentOmnichain(organizer, params)
    .accountsStrict({
      sponsor: admin.publicKey,
      config: accounts.config,
      tournament,
      operatorInfo,
      claimableUserInfo,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([admin]);

  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function registerTournamentSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  captain: Keypair,
  mint: PublicKey,
  operatorInfo
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const { config } = getGenomeAccounts(program, ROOT);

  const captainVault = await initializeOrGetUserVault(
    program.provider.connection,
    captain,
    mint,
    captain.publicKey,
    0
  );
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    mint,
    tournament,
    0
  );

  const teamParticipantPDA = getTeamParticipantPDA(
    program,
    ROOT,
    tournamentIndex,
    captain.publicKey
  );

  const team = getTeamPDA(program, ROOT, tournamentIndex, captain.publicKey);
  await program.methods
    .registerTournamentSinglechain()
    .accountsStrict({
      tournament,
      mint,
      captainVault: captainVault.address,
      tournamentVault: tournamentVault.address,
      captain: captain.publicKey,
      team,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      operatorInfo,
    })
    .remainingAccounts([
      {
        pubkey: teamParticipantPDA,
        isSigner: false,
        isWritable: true,
      },
    ])
    .signers([captain])
    .rpc({ commitment: "confirmed" });
  const tournamentAccount = await program.account.tournament.fetch(tournament);
  return tournamentAccount;
}

export async function registerTournamentOmnichain(
  program: Program<Genome>,
  payer: anchor.web3.Keypair,
  tournamentIndex: number,
  teammates: Array<PublicKey>,
  captain: PublicKey,
  operatorInfo
) {
  const tournament = getTournamentPDA(program, tournamentIndex);

  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);
  const claimableUserInfo = getUserInfo(program, ROOT, payer.publicKey);

  const remainingAcountsArr = [];
  for (let i = 0; i < teammates.length; i++) {
    const teamParticipantPDA = getTeamParticipantPDA(
      program,
      ROOT,
      tournamentIndex,
      teammates[i]
    );
    remainingAcountsArr.push({
      pubkey: teamParticipantPDA,
      isSigner: false,
      isWritable: true,
    });
  }

  const ix = await program.methods
    .registerTournamentOmnichain(teammates.slice(1)) // because first value is the captain
    .accountsStrict({
      payer: payer.publicKey,
      captain: captain,
      team,
      operatorInfo,
      claimableUserInfo,
      tournament,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .remainingAccounts(remainingAcountsArr)
    .signers([payer]);

  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = payer.publicKey;
  transaction.recentBlockhash = blockhash;

  await ix.rpc();
}

export async function getParticipantVault(
  participant: anchor.web3.PublicKey,
  mint: anchor.web3.PublicKey
) {
  return {
    vault: await token.getAssociatedTokenAddress(mint, participant),
    mint,
  };
}

export async function registerParticipantToTournamentSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  captain: PublicKey,
  teammate: PublicKey,
  adminMint: PublicKey,
  paymentApproach: number = 0,
  operatorInfo
) {
  const tournament = getTournamentPDA(program, tournamentIndex);

  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    adminMint,
    tournament,
    0
  );

  const organizerVaultResult = await getParticipantVault(
    admin.publicKey,
    adminMint
  );

  const teamParticipantPDA = getTeamParticipantPDA(
    program,
    ROOT,
    tournamentIndex,
    teammate
  );

  await program.methods
    .registerParticipantToTournamentSinglechain(teammate)
    .accountsStrict({
      payer: admin.publicKey,
      payerVault: organizerVaultResult.vault,
      tournament,
      team: team,
      mint: adminMint,
      tournamentVault: tournamentVault.address,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      operatorInfo,
    })
    .remainingAccounts([
      {
        pubkey: teamParticipantPDA,
        isSigner: false,
        isWritable: true,
      },
    ])
    .signers([admin])
    .rpc();
  const teamacc = await program.account.team.fetch(team);
  return teamacc;
}

export async function verifyAndPayoutTeamRegistration(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  captain: PublicKey
) {
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);
  const tournament = getTournamentPDA(program, tournamentIndex);

  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);

  const ix = await program.methods
    .teamRegistrationVerification() // Wager, participants
    .accountsStrict({
      admin: admin.publicKey,
      operatorInfo,
      tournament,
      systemProgram: anchor.web3.SystemProgram.programId,
      team,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();

  const tournamentAccount = await program.account.tournament.fetch(tournament);
  return tournamentAccount;
}

export async function startTournament(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);

  const ix = await program.methods
    .startTournament()
    .accountsStrict({
      participant: admin.publicKey,
      tournament,
      operatorInfo,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([admin]);

  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  const serializedTransaction = transaction.serialize();

  await ix.rpc();
}

export async function cancelTournamentSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  mint: PublicKey,
  operator: PublicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);

  const ix = await program.methods
    .cancelTournament() // Wager, participants
    .accountsStrict({
      organizer: admin.publicKey,
      tournament,
      operatorInfo: operator,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function refundParticipantCancelationSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  teammate: PublicKey,
  captain: PublicKey,
  adminMint: PublicKey,
  mint: PublicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);
  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    adminMint,
    tournament,
    0
  );

  const teammateVaultResult = await getParticipantVault(teammate, mint);

  const ix = await program.methods
    .refundParticipantCancelationSinglechain()
    .accountsStrict({
      admin: admin.publicKey,
      operatorInfo,
      participantVault: teammateVaultResult.vault,
      tournament,
      participant: teammate,
      mint,
      tournamentVault: tournamentVault.address,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      team,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function refundOrganizerCancelationSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  organizer: PublicKey,
  adminMint: PublicKey,
  mint: PublicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);
  const team = getTeamPDA(program, ROOT, tournamentIndex, organizer);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    adminMint,
    tournament,
    0
  );

  const organizerVaultResult = await getParticipantVault(organizer, mint);

  const ix = await program.methods
    .refundOrganizerCancelationSinglechain()
    .accountsStrict({
      admin: admin.publicKey,
      operatorInfo,
      organizerVault: organizerVaultResult.vault,
      tournament,
      mint: adminMint,
      tournamentVault: tournamentVault.address,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      team,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function finishTournament(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  winners: Array<PublicKey>,
  prizeFractions: Array<number>,
  feeType: number,
  mint: PublicKey,
  platformWallet: PublicKey,
  platformWalletVault: PublicKey
) {
  const accounts = getGenomeAccounts(program, ROOT);
  const tournament = getTournamentPDA(program, tournamentIndex);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    mint,
    tournament,
    0
  );
  const feeMeta = getFeeMeta(program, ROOT, feeType);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);

  const ix = await program.methods
    .finishTournament(prizeFractions, feeType,winners)
    .accountsStrict({
      operator: admin.publicKey,
      operatorInfo,
      config: accounts.config,
      platformWallet: platformWallet,
      platformWalletVault: platformWalletVault,
      feeMeta,
      tournamentVault: tournamentVault.address,
      tournament,
      mint,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function deliverFinishOrganizerTokensSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  organizer: PublicKey,
  captain: PublicKey,
  adminMint: PublicKey,
  mint: PublicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);
  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);

  const teamAccount = await program.account.team.fetch(team);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    adminMint,
    tournament,
    0
  );

  const organizerVaultResult = await getParticipantVault(organizer, mint);

  const ix = await program.methods
    .deliverFinishOrganizerTokensSinglechain()
    .accountsStrict({
      admin: admin.publicKey,
      operatorInfo,
      organizerVault: organizerVaultResult.vault,
      tournament,
      mint,
      tournamentVault: tournamentVault.address,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      team,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}

export async function deliverParticipantTokensSinglechain(
  program: Program<Genome>,
  admin: Keypair,
  tournamentIndex: number,
  teammate: PublicKey,
  captain: PublicKey,
  adminMint: PublicKey,
  mint: PublicKey
) {
  const tournament = getTournamentPDA(program, tournamentIndex);
  const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);
  const team = getTeamPDA(program, ROOT, tournamentIndex, captain);
  const tournamentVault = await initializeOrGetUserVault(
    program.provider.connection,
    admin,
    adminMint,
    tournament,
    0
  );

  const teammateVaultResult = await getParticipantVault(teammate, mint);

  const ix = await program.methods
    .deliverParticipantTokensSinglechain()
    .accountsStrict({
      admin: admin.publicKey,
      operatorInfo,
      participant: teammate,
      participantVault: teammateVaultResult.vault,
      tournament,
      mint,
      tournamentVault: tournamentVault.address,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      team,
    })
    .signers([admin]);
  const instruction = await ix.instruction();
  const transaction = new anchor.web3.Transaction().add(instruction);

  const { blockhash } = await program.provider.connection.getLatestBlockhash();
  transaction.feePayer = admin.publicKey;
  transaction.recentBlockhash = blockhash;

  transaction.sign(admin);

  await ix.rpc();
}
