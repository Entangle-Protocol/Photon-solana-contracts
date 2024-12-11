import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createTournamentOmnichain,
  createTournamentSinglechain,
  deliverFinishOrganizerTokensSinglechain,
  deliverParticipantTokensSinglechain,
  finishTournament,
  getTournamentPDA,
  registerParticipantToTournamentSinglechain,
  registerTournamentOmnichain,
  registerTournamentSinglechain,
  setTournamentParams,
  startTournament,
  verifyAndPayoutTeamRegistration,
} from "../../genome_test_setup/tournament";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";
import * as token from "@solana/spl-token";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  approveOperator,
  buildAndSendApproveTransaction,
  getOperatorInfo,
  getGenomeAccounts,
  init,
} from "../../genome_test_setup/genome";
import { Genome } from "../../target/types/genome";
import { updateClaimableRewards } from "../../genome_test_setup/treasury";
import { getFeeMeta } from "../../genome_test_setup/feeProvider";

describe("Tournament Program Tests", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const ROOT = utf8.encode("genome-root");
  let tournamentIndex = 0;
  const program = anchor.workspace.Genome as Program<Genome>;
  const admin = anchor.web3.Keypair.generate();
  let messenger = anchor.web3.Keypair.generate();
  let messengerOperator: PublicKey;
  let adminMint: PublicKey;
  const platformWallet = anchor.web3.Keypair.generate();
  let platformWalletVault: anchor.web3.PublicKey;
  let treasuryVault;
  let treasuryAuthority;
  const teamA = [
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
  ];

  const teamB = [
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
    anchor.web3.Keypair.generate(),
  ];
  let ownerOperator: PublicKey;
  const fakeAccountsToValidate = [anchor.web3.Keypair.generate()];
  const teamsWithParticipants = [teamA, teamB];
  const numberOfBeneficiaries = 5;
  const beneficiaries = Array.from({ length: numberOfBeneficiaries }, (_, i) =>
    anchor.web3.Keypair.generate()
  );
  const beneficiariesKeys = beneficiaries.map(
    (beneficiary) => beneficiary.publicKey
  );

  let mintsTokensPDAForParticipants: PublicKey[][] = [];

  before(async () => {
    const provider = anchor.getProvider();

    const tx = await provider.connection.requestAirdrop(
      admin.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.requestAirdrop(
      messenger.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);

    adminMint = await token.createMint(
      provider.connection,
      admin,
      admin.publicKey,
      null,
      6
    );
    const vault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      adminMint,
      admin.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      adminMint,
      vault,
      admin.publicKey,
      1000000000
    );
    for (const team of teamsWithParticipants) {
      let mintsByParticipant = [];
      for (const participant of team) {
        const vault = await token.createAssociatedTokenAccount(
          provider.connection,
          admin,
          adminMint,
          participant.publicKey
        );
        mintsByParticipant.push(vault);

        await provider.connection.requestAirdrop(
          participant.publicKey,
          anchor.web3.LAMPORTS_PER_SOL * 100
        );
        await token.mintTo(
          provider.connection,
          admin,
          adminMint,
          vault,
          admin.publicKey,
          1000000000
        );

        await buildAndSendApproveTransaction(
          provider,
          vault,
          admin.publicKey,
          participant,
          500000000
        );
      }
      mintsTokensPDAForParticipants.push(mintsByParticipant);
    }

    // send airdrop to other accounts
    for (const fakeAccount of fakeAccountsToValidate) {
      const tx = await provider.connection.requestAirdrop(
        fakeAccount.publicKey,
        anchor.web3.LAMPORTS_PER_SOL * 100
      );
    }

    platformWalletVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      adminMint,
      platformWallet.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      adminMint,
      platformWalletVault,
      admin.publicKey,
      1000000000
    );
  });

  it("Initializes", async () => {
    const { operatorInfo } = await init(ROOT, program, admin);
    ownerOperator = operatorInfo;
  });
  it("Sets Operator info", async () => {
    const messengerRole = { messenger: {} };
    await approveOperator(
      program,
      ROOT,
      admin,
      messenger.publicKey,
      messengerRole
    );
    messengerOperator = getOperatorInfo(program, ROOT, messenger.publicKey);
  });

  it("Sets Initial Params", async () => {
    const minimalAdmisionFee = 100;
    const minimalSponsorPool = 200;
    await setTournamentParams(
      program,
      admin,
      minimalAdmisionFee,
      minimalSponsorPool,
      ownerOperator
    );
  });

  describe("Tournament Happy Flow Omnichain", async () => {
    it("Creates a Tournament", async () => {
      const tournamentParams = {
        fee: new anchor.BN(500),
        sponsorPool: new anchor.BN(1000),
        startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
        playersInTeam: 4,
        minTeams: 2,
        maxTeams: 10,
        organizerRoyalty: 5,
        token: adminMint,
      };

      await updateClaimableRewards(program, ROOT, admin, 100000000);
      await createTournamentOmnichain(
        program,
        admin,
        tournamentIndex,
        tournamentParams,
        ownerOperator
      );
      const tournament = getTournamentPDA(program, tournamentIndex);
      const tournamentAccount = await program.account.tournament.fetch(
        tournament
      );
      assert.equal(tournamentAccount.id.toString(), tournamentIndex.toString());
    });

    it("Register a Tournament", async () => {
      for (const team of teamsWithParticipants) {
        // because will re-register captain
        // first index will always be the captain
        await registerTournamentOmnichain(
          program,
          admin,
          tournamentIndex,
          team.map((t) => t.publicKey),
          team[0].publicKey, // first participant of team
          ownerOperator
        );
      }
    });

    it("Handles Starting Tournament", async () => {
      const tournament = getTournamentPDA(program, tournamentIndex);

      for (const team of teamsWithParticipants) {
        await verifyAndPayoutTeamRegistration(
          program,
          admin,
          tournamentIndex,
          team[0].publicKey
        );
      }

      await startTournament(program, admin, tournamentIndex);

      const tournamentAccount = await program.account.tournament.fetch(
        tournament
      );

      assert.deepEqual(tournamentAccount.status, { started: {} });
    });
  });

  let tournamentSinglechainIndex = 1;

  describe("Tournament Happy Flow Singlechain", async () => {
    it("Creates a Tournament", async () => {
      const tournamentParams = {
        fee: new anchor.BN(500),
        sponsorPool: new anchor.BN(1000),
        startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
        playersInTeam: 4,
        minTeams: 2,
        maxTeams: 10,
        organizerRoyalty: 5,
        token: adminMint,
      };

      await createTournamentSinglechain(
        program,
        admin,
        tournamentSinglechainIndex,
        tournamentParams,
        adminMint,
        ownerOperator
      );
      const tournament = getTournamentPDA(program, tournamentSinglechainIndex);
      const tournamentAccount = await program.account.tournament.fetch(
        tournament
      );
      assert.equal(
        tournamentAccount.id.toString(),
        tournamentSinglechainIndex.toString()
      );
    });

    it("Register a Tournament", async () => {
      for (const [teamIndex, team] of teamsWithParticipants.entries()) {
        // because will re-register captain
        // first index will always be the captain
        await registerTournamentSinglechain(
          program,
          admin,
          tournamentSinglechainIndex,
          teamsWithParticipants[teamIndex][0], // first participant of team
          adminMint, //mintsTokensPDAForParticipants[teamIndex][0]
        );
        for (const [participantIndex, participant] of team.entries()) {
          if (participantIndex > 0) {
            await registerParticipantToTournamentSinglechain(
              program,
              admin,
              tournamentSinglechainIndex,
              team[0].publicKey,
              participant.publicKey,
              adminMint,
              0
            );
          }
        }
      }
    });

    it("Starts a Tournament", async () => {
      const tournament = getTournamentPDA(program, tournamentSinglechainIndex);

      for (const [teamIndex, team] of teamsWithParticipants.entries()) {
        await verifyAndPayoutTeamRegistration(
          program,
          admin,
          tournamentSinglechainIndex,
          team[0].publicKey
        );
      }

      await startTournament(program, admin, tournamentSinglechainIndex);

      const tournamentAccount = await program.account.tournament.fetch(
        tournament
      );

      assert.deepEqual(tournamentAccount.status, { started: {} });
    });

    it("Delivers the Tournament money", async () => {
      const tournament = getTournamentPDA(program, tournamentSinglechainIndex);
      const accounts = getGenomeAccounts(program, ROOT);
      const baseFee = 100;
      const feeType = 1;
      const feeMeta = getFeeMeta(program, ROOT, feeType);
      let fractions = [];
      for (let i = 0; i < numberOfBeneficiaries; i++) {
        fractions.push(new anchor.BN(baseFee / numberOfBeneficiaries));
      }

      await program.methods
        .setFeeParams(
          feeType,
          platformWallet.publicKey,
          new anchor.BN(baseFee),
          beneficiariesKeys,
          fractions,
          new anchor.BN(baseFee)
        )
        .accountsStrict({
          operator: admin.publicKey,
          operatorInfo: ownerOperator,
          feeMeta,
          config: accounts.config,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin])
        .rpc();
      const winners = [teamA[0].publicKey];
      const prizes = [100];
      await finishTournament(
        program,
        admin,
        tournamentSinglechainIndex,
        winners, // captains
        prizes,
        tournamentSinglechainIndex,
        adminMint,
        platformWallet.publicKey,
        platformWalletVault
      );

      // teamA was choosen as the winners above
      for (const participants of teamA) {
        await deliverParticipantTokensSinglechain(
          program,
          admin,
          tournamentSinglechainIndex,
          participants.publicKey,
          teamA[0].publicKey,
          adminMint,
          adminMint
        );
      }

      const b4 = await program.account.tournament.fetch(tournament);
      await deliverFinishOrganizerTokensSinglechain(
        program,
        admin,
        tournamentSinglechainIndex,
        admin.publicKey,
        teamA[0].publicKey,
        adminMint,
        adminMint
      );

      const tournamentAccount = await program.account.tournament.fetch(
        tournament
      );

      assert.deepEqual(tournamentAccount.status, { finished: {} });
    });
  });
});
