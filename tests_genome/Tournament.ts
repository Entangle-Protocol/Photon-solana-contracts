import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  cancelTournamentSinglechain,
  createTournamentSinglechain,
  getTournamentPDA,
  registerParticipantToTournamentSinglechain,
  registerTournamentSinglechain,
  setTournamentParams,
  startTournament,
  TournamentParams,
  verifyAndPayoutTeamRegistration,
} from "../../solana_contracts/genome_test_setup/tournament";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";
import * as token from "@solana/spl-token";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  buildAndSendApproveTransaction,
  getGenomeAccounts,
  init,
} from "../../solana_contracts/genome_test_setup/genome";
import { Genome } from "../../solana_contracts/target/types/genome";

describe("Tournament Program Tests", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const ROOT = utf8.encode("genome-root");
  let tournamentIndex = 0;
  const program = anchor.workspace.Genome as Program<Genome>;
  const admin = anchor.web3.Keypair.generate();
  let adminMint: PublicKey;
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

  let mintsTokensPDAForParticipants: PublicKey[][] = [];

  before(async () => {
    const provider = anchor.getProvider();

    const tx = await provider.connection.requestAirdrop(
      admin.publicKey,
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
  });

  describe("Initializes Tournament Configuration", async () => {
    it("Initializes", async () => {
      const { operatorInfo } = await init(ROOT, program, admin);
      ownerOperator = operatorInfo;
    });

    it("Sets Initial Params", async () => {
      const minimalAdmisionFee = 100;
      const minimalSponsorPool = 200;
      const accounts = getGenomeAccounts(program, ROOT);
      await setTournamentParams(
        program,
        admin,
        minimalAdmisionFee,
        minimalSponsorPool,
        ownerOperator
      );
      const configAccount = await program.account.genomeConfig.fetch(
        accounts.config
      );
    });
  });

  describe("Tournament Handling", () => {
    describe("Handles Tournament Creation", () => {
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
          tournamentIndex,
          tournamentParams,
          adminMint,
          ownerOperator
        );
        const tournament = getTournamentPDA(program, tournamentIndex);
        const tournamentAccount = await program.account.tournament.fetch(
          tournament
        );
        assert.equal(
          tournamentAccount.id.toString(),
          tournamentIndex.toString()
        );
      });

      describe("On Receive Error", () => {
        it("throws InvalidAdmissionFeeOrSponsorPool", async () => {
          const invalidParams: TournamentParams = {
            fee: new anchor.BN(50), // Below the minimal admission fee
            sponsorPool: new anchor.BN(50), // Below the minimal sponsor pool
            startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
            playersInTeam: 5,
            minTeams: 2,
            maxTeams: 10,
            organizerRoyalty: 5,
            token: adminMint,
          };

          try {
            await createTournamentSinglechain(
              program,
              admin,
              tournamentIndex + 1,
              invalidParams,
              adminMint,
              ownerOperator
            );
            assert.ok(false);
          } catch (_err) {
            assert.ok(_err instanceof anchor.AnchorError);
            const errMsg = "InvalidAdmissionFeeOrSponsorPool";
            assert.equal(_err.error.errorMessage, errMsg);
          }
        });

        it("throws InvalidAmountOfPlayers", async () => {
          const invalidParams: TournamentParams = {
            fee: new anchor.BN(500),
            sponsorPool: new anchor.BN(1000),
            startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
            playersInTeam: 0,
            minTeams: 2,
            maxTeams: 10,
            organizerRoyalty: 5,
            token: adminMint,
          };

          try {
            await createTournamentSinglechain(
              program,
              admin,
              tournamentIndex + 1,
              invalidParams,
              adminMint,
              ownerOperator
            );
            assert.ok(false);
          } catch (_err) {
            assert.ok(_err instanceof anchor.AnchorError);
            const errMsg = "InvalidAmountOfPlayers";
            assert.equal(_err.error.errorMessage, errMsg);
          }
        });

        it("throws InvalidRoyalty", async () => {
          const invalidParams: TournamentParams = {
            fee: new anchor.BN(500),
            sponsorPool: new anchor.BN(1000),
            startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
            playersInTeam: 5,
            minTeams: 2,
            maxTeams: 10,
            organizerRoyalty: 1001, // Royalty exceeds 100
            token: adminMint,
          };

          try {
            await createTournamentSinglechain(
              program,
              admin,
              tournamentIndex + 1,
              invalidParams,
              adminMint,
              ownerOperator
            );
            assert.ok(false);
          } catch (_err) {
            assert.ok(_err instanceof anchor.AnchorError);
            const errMsg = "InvalidRoyalty";
            assert.equal(_err.error.errorMessage, errMsg);
          }
        });
      });
    });

    describe("Register a Tournament", () => {
      it("Register a Tournament", async () => {
        for (const [teamIndex, team] of teamsWithParticipants.entries()) {
          // because will re-register captain
          // first index will always be the captain
          await registerTournamentSinglechain(
            program,
            admin,
            tournamentIndex,
            teamsWithParticipants[teamIndex][0], // first participant of team
            adminMint, //mintsTokensPDAForParticipants[teamIndex][0]
            ownerOperator
          );

          for (const [participantIndex, participant] of team.entries()) {
            if (participantIndex > 0) {
              await registerParticipantToTournamentSinglechain(
                program,
                admin,
                tournamentIndex,
                team[0].publicKey,
                participant.publicKey,
                adminMint,
                0,
                ownerOperator
              );
            }
          }
        }
      });

      it("throws NotExistingTeam", async () => {
        // because fakeAccountsToValidate is not registered yet
        try {
          await registerParticipantToTournamentSinglechain(
            program,
            admin,
            tournamentIndex,
            fakeAccountsToValidate[0].publicKey,
            fakeAccountsToValidate[0].publicKey,
            adminMint,
            0,
            ownerOperator
          );
          assert.ok(false);
        } catch (_err) {
          assert.ok(_err instanceof anchor.AnchorError);
          const errMsg =
            "The program expected this account to be already initialized";
          assert.equal(_err.error.errorMessage, errMsg);
        }
      });
    });

    describe("Start Tournament", () => {
      it("throws TeamsValidationCheckNotCompleted", async () => {
        try {
          await startTournament(program, admin, tournamentIndex);

          assert.ok(false);
        } catch (_err) {
          assert.ok(_err instanceof anchor.AnchorError);
          const errMsg = "TeamsValidationCheckNotCompleted";
          assert.equal(_err.error.errorMessage, errMsg);
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

    describe("Cancel a Tournament", () => {
      it("Cancel a Tournament", async () => {
        const tx = await cancelTournamentSinglechain(
          program,
          admin,
          tournamentIndex,
          adminMint,
          ownerOperator
        );

        const tournament = getTournamentPDA(program, tournamentIndex);
        const tournamentAccount = await program.account.tournament.fetch(
          tournament
        );
        assert.deepEqual(tournamentAccount.status, { preCancel: {} });
      });

      it("throws InvalidTournamentStatus", async () => {
        // should throw because status should be already "PreCancel"
        try {
          const tx = await cancelTournamentSinglechain(
            program,
            admin,
            tournamentIndex,
            adminMint,
            ownerOperator
          );

          assert.ok(false);
        } catch (_err) {
          assert.ok(_err instanceof anchor.AnchorError);
          const errMsg = "InvalidTournamentStatus";
          assert.equal(_err.error.errorMessage, errMsg);
        }
      });
    });
  });
});
