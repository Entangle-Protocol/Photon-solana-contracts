import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { Program } from "@coral-xyz/anchor";
import { Genome } from "../../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { getGamePDA, getParticipantPDA } from "../../genome_test_setup/gameProvider";
import {
  getGenomeAccounts,
  getOperatorInfo,
  buildAndSendApproveTransaction,
  approveOperator,
  init,
} from "../../genome_test_setup/genome";
import {
  createTournamentOmnichain,
  createTournamentSinglechain,
  getTournamentPDA,
  registerParticipantToTournamentSinglechain,
  registerTournamentOmnichain,
  registerTournamentSinglechain,
  setTournamentParams,
  startTournament,
  verifyAndPayoutTeamRegistration,
  finishTournament,
} from "../../genome_test_setup/tournament";
import {
  getTournamentBookPDA,
  getCaptainBetPDA,
  getGamblerInfoPDA,
} from "../../genome_test_setup/bookmaker";
import { getFeeMeta } from "../../genome_test_setup/feeProvider";
import { getTreasuryAccounts } from "../../genome_test_setup/treasury";
import { assert } from "chai";

describe("zs-single-solana: BookMaker", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  // ------------ ROOTS ----------------
  const ROOT = utf8.encode("genome-root");

  // ------------ PROGRAMS ----------------
  const program = anchor.workspace.Genome as Program<Genome>;

  // ------------ KEYS ----------------
  const admin = anchor.web3.Keypair.generate();
  const operator = anchor.web3.Keypair.generate();
  const developerOperator = anchor.web3.Keypair.generate();
  const gambler = anchor.web3.Keypair.generate();
  const otherGambler = anchor.web3.Keypair.generate();
  const numberOfGamblers = 5;
  let gamblers = [];
  gamblers = Array.from({ length: numberOfGamblers }, (_, i) =>
    anchor.web3.Keypair.generate()
  );
  const platformWallet = anchor.web3.Keypair.generate();
  const numberOfBeneficiaries = 5;
  const beneficiaries = Array.from({ length: numberOfBeneficiaries }, (_, i) =>
    anchor.web3.Keypair.generate()
  );

  // ------------ TOKENS AND VAULTS ----------------

  let mint: anchor.web3.PublicKey;
  let operatorVault: anchor.web3.PublicKey;
  let developerOperatorVault: anchor.web3.PublicKey;
  let gamblerVault: anchor.web3.PublicKey;
  let otherGamblerVault: anchor.web3.PublicKey;
  let gamblerVaults = [];
  let platformWalletVault: anchor.web3.PublicKey;
  let treasuryVault;
  let beneficieriesVaults = [];

  // ------------ TOURNAMENT ----------------
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

  const teamsWithParticipants = [teamA, teamB];

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
      developerOperator.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);
    tx = await provider.connection.requestAirdrop(
      gambler.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);
    tx = await provider.connection.requestAirdrop(
      otherGambler.publicKey,
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
    // ------------ Setup operator ----------------------
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
    // ------------ Setup developer operator ----------------------
    developerOperatorVault = await token.createAccount(
      provider.connection,
      admin,
      mint,
      developerOperator.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      developerOperatorVault,
      admin.publicKey,
      1000000000
    );
    //-------------- Setup gambler ----------------------
    gamblerVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      gambler.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      gamblerVault,
      admin.publicKey,
      1000000000
    );

    //-------------- Setup other gambler ----------------------
    otherGamblerVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      otherGambler.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      otherGamblerVault,
      admin.publicKey,
      1000000000
    );
    //-------------- Setup treasury vault ----------------------
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    treasuryVault = await token.getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      treasuryAccounts.authority,
      true
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      treasuryVault.address,
      admin.publicKey,
      1000000000
    );

    //-------------- Setup platform wallet vault ----------------------
    platformWalletVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      platformWallet.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      platformWalletVault,
      admin.publicKey,
      1000000000
    );

    // --------- Multiple Gamblers Array ----------------
    for (const gambler of gamblers) {
      const vault = await token.createAssociatedTokenAccount(
        provider.connection,
        admin,
        mint,
        gambler.publicKey
      );
      gamblerVaults.push(vault);

      await token.mintTo(
        provider.connection,
        admin,
        mint,
        vault,
        admin.publicKey,
        1000000000
      );
    }

    // --------- Setup vaults for tournaments ----------------
    const vault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      admin.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
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
          mint,
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
          mint,
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
    }
  });

  it("Is initialized!", async () => {
    await init(ROOT, program, admin);
  });

  it("Approve operator", async () => {
    const developerRole = { developer: {} };
    await approveOperator(
      program,
      ROOT,
      admin,
      developerOperator.publicKey,
      developerRole
    );
    const backenRole = { backend: {} };
    await approveOperator(program, ROOT, admin, operator.publicKey, backenRole);
  });

  it("Set Minimal Bet", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);

    await program.methods
      .setMinimalBet(new anchor.BN(100))
      .accountsStrict({
        admin: admin.publicKey,
        config: accounts.config,
        operatorInfo,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([admin])
      .rpc();
  });

  it("Creates a Tournament", async () => {
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const tournamentParams = {
      fee: new anchor.BN(500),
      sponsorPool: new anchor.BN(1000),
      startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
      playersInTeam: 4,
      minTeams: 2,
      maxTeams: 10,
      organizerRoyalty: 5,
      token: mint,
    };

    await createTournamentSinglechain(
      program,
      operator,
      0,
      tournamentParams,
      mint,
      operatorInfo
    );
    const tournament = getTournamentPDA(program, 0);
    const tournamentAccount = await program.account.tournament.fetch(
      tournament
    );
    assert.equal(tournamentAccount.id.toString(), "0");
  });

  it("Register a Tournament", async () => {
    for (const [teamIndex, team] of teamsWithParticipants.entries()) {
      // because will re-register captain
      // first index will always be the captain
      await registerTournamentSinglechain(
        program,
        admin,
        0,
        teamsWithParticipants[teamIndex][0],
        mint
      );

      for (const [participantIndex, participant] of team.entries()) {
        if (participantIndex > 0) {
          await registerParticipantToTournamentSinglechain(
            program,
            admin,
            0,
            team[0].publicKey,
            participant.publicKey,
            mint
          );
        }
      }
    }
  });

  it("Make a bet", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamA[0].publicKey);
    const gamblerInfo = getGamblerInfoPDA(
      program,
      ROOT,
      0,
      teamA[0].publicKey,
      gambler.publicKey
    );
    const gamblerVaultBefore = await token.getAccount(
      program.provider.connection,
      gamblerVault
    );

    await program.methods
      .makeBet(
        gambler.publicKey,
        teamA[0].publicKey,
        new anchor.BN(0),
        new anchor.BN(5000)
      ) // winners, prizes
      .accountsStrict({
        payer: gambler.publicKey,
        payerVault: gamblerVault,
        tournament,
        tournamentBook,
        tournamentBookVault,
        captainBet,
        gamblerInfo,
        mint,
        config: accounts.config,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gambler])
      .rpc();

    const gamblerVaultAfter = await token.getAccount(
      program.provider.connection,
      gamblerVault
    );

    assert.equal(
      Number(gamblerVaultAfter.amount),
      Number(gamblerVaultBefore.amount) - 5000
    );

    const gamblerInfoAccount = await program.account.gamblerInfo.fetch(
      gamblerInfo
    );
    assert.equal(Number(gamblerInfoAccount.bet), 5000);

    const captainBetAccount = await program.account.captainBet.fetch(
      captainBet
    );
    assert.equal(Number(captainBetAccount.sum), 5000);

    const tournamentBookAccount = await program.account.tournamentBook.fetch(
      tournamentBook
    );
    assert.equal(Number(tournamentBookAccount.totalSum), 5000);

    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(Number(tournamentBookVaultAccount.amount), 5000);
  });

  it("Other Gambler makes a bet on the same team", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamA[0].publicKey);
    const otherGamblerInfo = getGamblerInfoPDA(
      program,
      ROOT,
      0,
      teamA[0].publicKey,
      otherGambler.publicKey
    );
    const otherGamblerVaultBefore = await token.getAccount(
      program.provider.connection,
      otherGamblerVault
    );

    await program.methods
      .makeBet(
        otherGambler.publicKey,
        teamA[0].publicKey,
        new anchor.BN(0),
        new anchor.BN(5000)
      ) // winners, prizes
      .accountsStrict({
        payer: otherGambler.publicKey,
        payerVault: otherGamblerVault,
        tournament,
        tournamentBook,
        tournamentBookVault,
        captainBet,
        gamblerInfo: otherGamblerInfo,
        mint,
        config: accounts.config,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([otherGambler])
      .rpc();

    const otherGamblerVaultAfter = await token.getAccount(
      program.provider.connection,
      otherGamblerVault
    );

    assert.equal(
      Number(otherGamblerVaultAfter.amount),
      Number(otherGamblerVaultBefore.amount) - 5000
    );

    const otherGamblerInfoAccount = await program.account.gamblerInfo.fetch(
      otherGamblerInfo
    );
    assert.equal(Number(otherGamblerInfoAccount.bet), 5000);

    const captainBetAccount = await program.account.captainBet.fetch(
      captainBet
    );
    assert.equal(Number(captainBetAccount.sum), 10000);

    const tournamentBookAccount = await program.account.tournamentBook.fetch(
      tournamentBook
    );
    assert.equal(Number(tournamentBookAccount.totalSum), 10000);

    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(Number(tournamentBookVaultAccount.amount), 10000);
  });

  it("Make bets on the other team", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamB[0].publicKey);
    for (const gambler of gamblers) {
      const gamblerInfo = getGamblerInfoPDA(
        program,
        ROOT,
        0,
        teamB[0].publicKey,
        gambler.publicKey
      );

      const operatorVaultBefore = await token.getAccount(
        program.provider.connection,
        operatorVault
      );

      await program.methods
        .makeBet(
          gambler.publicKey,
          teamB[0].publicKey,
          new anchor.BN(0),
          new anchor.BN(100)
        ) // winners, prizes
        .accountsStrict({
          payer: operator.publicKey,
          payerVault: operatorVault,
          tournament,
          tournamentBook,
          tournamentBookVault,
          captainBet,
          gamblerInfo,
          mint,
          config: accounts.config,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([operator])
        .rpc();

      const operatorVaultAfter = await token.getAccount(
        program.provider.connection,
        operatorVault
      );

      assert.equal(
        Number(operatorVaultAfter.amount),
        Number(operatorVaultBefore.amount) - 100
      );
    }

    const captainBetAccount = await program.account.captainBet.fetch(
      captainBet
    );
    assert.equal(Number(captainBetAccount.sum), numberOfGamblers * 100);

    const tournamentBookAccount = await program.account.tournamentBook.fetch(
      tournamentBook
    );
    assert.equal(
      Number(tournamentBookAccount.totalSum),
      10000 + numberOfGamblers * 100
    );

    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(
      Number(tournamentBookVaultAccount.amount),
      10000 + numberOfGamblers * 100
    );
  });

  it("Handles Starting Tournament", async () => {
    const tournament = getTournamentPDA(program, 0);

    for (const [teamIndex, team] of teamsWithParticipants.entries()) {
      await verifyAndPayoutTeamRegistration(
        program,
        admin,
        0,
        team[0].publicKey
      );
    }
    await startTournament(program, admin, 0);

    const tournamentAccount = await program.account.tournament.fetch(
      tournament
    );

    assert.deepEqual(tournamentAccount.status, { started: {} });
  });

  // To have an overbook:
  // captainSum * minimalCoef > totalSum * BP_DEC
  // where minimalCoef = 2 * BP_DEC - BP_DEC / teamsCount = 3/2 BP_DEC
  // captainSum_1 = 10000
  // captainSum_2 = 100 * numberOfGamblers = 500
  // totalSum = 10000 + 500 = 10500
  // For captainSum_1:
  // 3/2 BP_DEC * 10000 > 10500 * BP_DEC
  // 15000 > 10500
  // For captainSum_2:
  // 3/2 BP_DEC * 500 > 10500 * BP_DEC
  // 750 > 10500
  // captainSum_1 is overbooked, captainSum_2 is not
  it("Gambler claims overbook", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamA[0].publicKey);
    const gamblerInfo = getGamblerInfoPDA(
      program,
      ROOT,
      0,
      teamA[0].publicKey,
      gambler.publicKey
    );
    const gamblerVaultBefore = await token.getAccount(
      program.provider.connection,
      gamblerVault
    );

    await program.methods
      .claimOverbookTokens(
        gambler.publicKey,
        teamA[0].publicKey,
        new anchor.BN(0)
      ) // winners, prizes
      .accountsStrict({
        gambler: gambler.publicKey,
        gamblerVault: gamblerVault,
        tournament,
        tournamentBook,
        tournamentBookVault,
        captainBet,
        gamblerInfo,
        mint,
        config: accounts.config,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gambler])
      .rpc();

    // The overbookClaimable should be 9000 - 5000 = 4000 now

    const captainBetAccount = await program.account.captainBet.fetch(
      captainBet
    );
    assert.equal(Number(captainBetAccount.overbookClaimable), 4000);
    assert.equal(captainBetAccount.overbooked, true);

    const gamblerVaultAfter = await token.getAccount(
      program.provider.connection,
      gamblerVault
    );

    // Gambler can claim his entire bet
    assert.equal(
      Number(gamblerVaultAfter.amount),
      Number(gamblerVaultBefore.amount) + 5000
    );

    // GamblerInfo should be reset to 0
    const gamblerInfoAccount = await program.account.gamblerInfo.fetch(
      gamblerInfo
    );
    assert.equal(Number(gamblerInfoAccount.bet), 0);

    const tournamentBookAccount = await program.account.tournamentBook.fetch(
      tournamentBook
    );
    assert.equal(Number(tournamentBookAccount.totalSum), 5500);
    assert.equal(Number(tournamentBookAccount.totalOverbookClaimable), 4000);

    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(Number(tournamentBookVaultAccount.amount), 5500);
  });

  it("Set fee params: fee_type == 1", async () => {
    const baseFee = 100;
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const beneficiariesKeys = beneficiaries.map(
      (beneficiary) => beneficiary.publicKey
    );
    let fractions = [];
    for (let i = 0; i < numberOfBeneficiaries; i++) {
      fractions.push(new anchor.BN(baseFee / numberOfBeneficiaries));
    }

    const developerOperatorInfo = getOperatorInfo(
      program,
      ROOT,
      developerOperator.publicKey
    );
    const accounts = getGenomeAccounts(program, ROOT);

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
        operator: developerOperator.publicKey,
        operatorInfo: developerOperatorInfo,
        feeMeta,
        config: accounts.config,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([developerOperator])
      .rpc();
  });

  // TODO: Tournament Finishes before otherGambler can claim his bet
  // The teamB wins
  it("Set Tournament to PreFinish", async () => {
    const tournament = getTournamentPDA(program, 0);
    const accounts = getGenomeAccounts(program, ROOT);
    const baseFee = 100;
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    await finishTournament(
      program,
      admin,
      0,
      [teamB[0].publicKey],
      [100],
      1,
      mint,
      platformWallet.publicKey,
      platformWalletVault
    );

    const tournamentAccount = await program.account.tournament.fetch(
      tournament
    );

    assert.deepEqual(tournamentAccount.status, { preFinish: {} });
  });

  it("Gamblers claim their finish tokens", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamB[0].publicKey);

    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const feeMetaBefore = await program.account.feeMeta.fetch(feeMeta);

    const beforePendingToClaim = feeMetaBefore.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );

    const platformWalletAccountBefore = await token.getAccount(
      program.provider.connection,
      platformWalletVault
    );

    // Total sum is 5500
    // Overbook claimable is 4000
    // prize_pool = 1500 - 1500 * 100 / 10000 = 1485
    // Beneficiaries should have 15 / 5 = 3
    // Gamblers should have 1485 * 100 / 500 = 297

    for (const gambler of gamblers) {
      const gamblerInfo = getGamblerInfoPDA(
        program,
        ROOT,
        0,
        teamB[0].publicKey,
        gambler.publicKey
      );

      const gamblerVault = await token.getOrCreateAssociatedTokenAccount(
        program.provider.connection,
        admin,
        mint,
        gambler.publicKey,
        true
      );

      const gamblerVaultBefore = await token.getAccount(
        program.provider.connection,
        gamblerVault.address
      );

      await program.methods
        .claimFinishTokens(
          gambler.publicKey,
          teamB[0].publicKey,
          new anchor.BN(0),
          feeType
        ) // winners, prizes
        .accountsStrict({
          gambler: gambler.publicKey,
          gamblerVault: gamblerVault.address,
          tournament,
          tournamentBook,
          tournamentBookVault,
          captainBet,
          feeMeta,
          platformWallet: platformWallet.publicKey,
          platformWalletVault,
          treasuryAuthority: treasuryAccounts.authority,
          treasuryVault: treasuryVault.address,
          gamblerInfo,
          mint,
          config: accounts.config,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gambler])
        .rpc();

      const gamblerVaultAfter = await token.getAccount(
        program.provider.connection,
        gamblerVault.address
      );

      // Gambler can claim his entire bet
      assert.equal(
        Number(gamblerVaultAfter.amount),
        Number(gamblerVaultBefore.amount) + 297
      );

      // GamblerInfo should be updated
      const gamblerInfoAccount = await program.account.gamblerInfo.fetch(
        gamblerInfo
      );

      assert.equal(gamblerInfoAccount.hasClaimedFinish, true);
    }

    // The platform wallet should remain the same because the feeType is not 0
    // and the fractions sum feeMeta.baseFee
    const platformWalletAccountAfter = await token.getAccount(
      program.provider.connection,
      platformWalletVault
    );
    assert.equal(
      Number(platformWalletAccountAfter.amount),
      Number(platformWalletAccountBefore.amount)
    );

    // the beneficiaries should have 30 more pending to claim
    const feeMetaAfter = await program.account.feeMeta.fetch(feeMeta);
    const actualFractions = feeMetaAfter.fractions.map(
      (fraction: anchor.BN) => fraction.toNumber()
    );

    const actualPendingToClaim = feeMetaAfter.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );

    const expectedPendingToClaim = [13, 13, 13, 13, 13];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);

    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(Number(tournamentBookVaultAccount.amount), 4000);
  });

  it("The other gambler claims overbook", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const tournament = getTournamentPDA(program, 0);
    const tournamentBook = getTournamentBookPDA(program, ROOT, 0);
    const tournamentBookVault = token.getAssociatedTokenAddressSync(
      mint,
      tournamentBook,
      true
    );
    const captainBet = getCaptainBetPDA(program, ROOT, 0, teamA[0].publicKey);
    const otherGamblerInfo = getGamblerInfoPDA(
      program,
      ROOT,
      0,
      teamA[0].publicKey,
      otherGambler.publicKey
    );
    const otherGamblerVaultBefore = await token.getAccount(
      program.provider.connection,
      otherGamblerVault
    );

    await program.methods
      .claimOverbookTokens(
        otherGambler.publicKey,
        teamA[0].publicKey,
        new anchor.BN(0)
      ) // winners, prizes
      .accountsStrict({
        gambler: otherGambler.publicKey,
        gamblerVault: otherGamblerVault,
        tournament,
        tournamentBook,
        tournamentBookVault,
        captainBet,
        gamblerInfo: otherGamblerInfo,
        mint,
        config: accounts.config,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([otherGambler])
      .rpc();

    // The overbookClaimable should be 0 now

    const captainBetAccount = await program.account.captainBet.fetch(
      captainBet
    );
    assert.equal(Number(captainBetAccount.overbookClaimable), 0);
    assert.equal(captainBetAccount.overbooked, true);

    const otherGamblerVaultAfter = await token.getAccount(
      program.provider.connection,
      otherGamblerVault
    );

    // Gambler can claim 4000 of his bet
    assert.equal(
      Number(otherGamblerVaultAfter.amount),
      Number(otherGamblerVaultBefore.amount) + 4000
    );

    // GamblerInfo should be reset to 1000
    const otherGamblerInfoAccount = await program.account.gamblerInfo.fetch(
      otherGamblerInfo
    );
    assert.equal(Number(otherGamblerInfoAccount.bet), 1000);

    const tournamentBookAccount = await program.account.tournamentBook.fetch(
      tournamentBook
    );
    assert.equal(Number(tournamentBookAccount.totalSum), 1500);

    // The tournamentBookVault should be empty since the other gambler already claimed their bet
    const tournamentBookVaultAccount = await token.getAccount(
      program.provider.connection,
      tournamentBookVault
    );
    assert.equal(Number(tournamentBookVaultAccount.amount), 0);
  });
});
