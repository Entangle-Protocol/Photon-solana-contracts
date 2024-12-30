import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { Program } from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { getGamePDA, getParticipantPDA } from "../genome_test_setup/gameProvider";
import {
  getGenomeAccounts,
  getOperatorInfo,
  buildAndSendApproveTransaction,
  approveOperator,
  init,
} from "../genome_test_setup/genome";
import { getTreasuryAccounts, getUserInfo } from "../genome_test_setup/treasury";
import { getFeeMeta } from "../genome_test_setup/feeProvider";
import { assert } from "chai";

describe("zs-single-solana: Game", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  // ------------ ROOTS ----------------
  const ROOT = utf8.encode("genome-root");

  // ------------ PROGRAMS ----------------
  const program = anchor.workspace.Genome as Program<Genome>;

  // ------------ KEYS ----------------
  const admin = anchor.web3.Keypair.generate();
  const operator = anchor.web3.Keypair.generate();
  const notOperator = anchor.web3.Keypair.generate();
  const messengerOperator = anchor.web3.Keypair.generate();
  const developerOperator = anchor.web3.Keypair.generate();
  const participant = anchor.web3.Keypair.generate();
  const otherParticipant = anchor.web3.Keypair.generate();
  const numberOfParticipants = 5;
  let participants = [];
  participants = Array.from({ length: numberOfParticipants }, (_, i) =>
    anchor.web3.Keypair.generate()
  );

  // ------------ TOKENS AND VAULTS ----------------

  let mint: anchor.web3.PublicKey;
  let operatorVault: anchor.web3.PublicKey;
  let messengerOperatorVault: anchor.web3.PublicKey;
  let developerOperatorVault: anchor.web3.PublicKey;
  let participantVault: anchor.web3.PublicKey;
  let otherParticipantVault: anchor.web3.PublicKey;
  let participantVaults = [];

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
      messengerOperator.publicKey,
      anchor.web3.LAMPORTS_PER_SOL * 100
    );
    await provider.connection.confirmTransaction(tx);
    tx = await provider.connection.requestAirdrop(
      developerOperator.publicKey,
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
    // ------------ Setup operator ----------------------
    operatorVault = await token.createAccount(
      program.provider.connection,
      admin,
      mint,
      operator.publicKey
    );
    console.log(admin, mint, operator, operatorVault);
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      operatorVault,
      admin,
      1000000000
    );
    // ------------ Setup messenger operator ----------------------
    messengerOperatorVault = await token.createAccount(
      provider.connection,
      admin,
      mint,
      messengerOperator.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      messengerOperatorVault,
      admin,
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
      admin,
      1000000000
    );
    //-------------- Setup participant ----------------------
    participantVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      participant.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      participantVault,
      admin,
      1000000000
    );
    // Delegate some tokens to the operator and messenger operator
    await buildAndSendApproveTransaction(
      provider,
      participantVault,
      operator.publicKey,
      participant,
      500000000
    );

    //-------------- Setup other participant ----------------------
    otherParticipantVault = await token.createAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      otherParticipant.publicKey
    );
    await token.mintTo(
      provider.connection,
      admin,
      mint,
      otherParticipantVault,
      admin,
      1000000000
    );
    // Delegate some tokens to the operator and messenger operator
    await buildAndSendApproveTransaction(
      provider,
      otherParticipantVault,
      operator.publicKey,
      otherParticipant,
      500000000
    );

    // --------- Multiple Participants Array ----------------
    for (const participant of participants) {
      const vault = await token.createAssociatedTokenAccount(
        provider.connection,
        admin,
        mint,
        participant.publicKey
      );
      participantVaults.push(vault);

      await token.mintTo(
        provider.connection,
        admin,
        mint,
        vault,
        admin,
        1000000000
      );

      await buildAndSendApproveTransaction(
        provider,
        vault,
        operator.publicKey,
        participant,
        500000000
      );
    }
  });

  it("Is initialized!", async () => {
    await init(ROOT, program, admin);
  });

  it("Approve operator", async () => {
    const role = { backend: {} };
    await approveOperator(program, ROOT, admin, operator.publicKey, role);
    const developerRole = { developer: {} };
    await approveOperator(
      program,
      ROOT,
      admin,
      developerOperator.publicKey,
      developerRole
    );
    const messengerRole = { messenger: {} };
    await approveOperator(
      program,
      ROOT,
      admin,
      messengerOperator.publicKey,
      messengerRole
    );
  });

  it("Create Game Singlechain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 0);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    await program.methods
      .createGameSinglechain(new anchor.BN(10000)) // Wager
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        config: accounts.config,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: participantVault, isSigner: false, isWritable: true },
        { pubkey: otherParticipantVault, isSigner: false, isWritable: true },
      ])
      .signers([operator])
      .rpc();

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);

    assert.equal(Number(gameAccount.id), Number(0));
    assert.deepEqual(gameAccount.status, { created: {} });
    assert.equal(Number(gameAccount.wager), 10000);

    const actualParticipants = gameAccount.participants;
    const expectedParticipants = [
      participant.publicKey,
      otherParticipant.publicKey,
    ];

    const actualParticipantsSet = new Set(actualParticipants);
    const participantsKeysSet = new Set(expectedParticipants);

    assert.deepEqual(actualParticipantsSet, participantsKeysSet);

    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(Number(gameVaultAfter.amount), 20000);
  });

  it("Register more participants in a Game Singlechain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 1);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    await program.methods
      .createGameSinglechain(new anchor.BN(10000)) // Wager
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        config: accounts.config,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: participantVault, isSigner: false, isWritable: true },
        { pubkey: otherParticipantVault, isSigner: false, isWritable: true },
      ])
      .signers([operator])
      .rpc();

    // Register 15 participants

    const remainingAcountsObj = [];

    for (let i = 0; i < participants.length; i++) {
      const participantVault = participantVaults[i];
      remainingAcountsObj.push({
        pubkey: participantVault,
        isSigner: false,
        isWritable: true,
      });
    }

    await program.methods
      .registerGameParticipantsSinglechain(false)
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAcountsObj)
      .signers([operator])
      .rpc();

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);

    assert.equal(Number(gameAccount.id), Number(1));
    assert.deepEqual(gameAccount.status, { created: {} });
    assert.equal(Number(gameAccount.wager), 10000);

    const actualParticipants = gameAccount.participants;
    const participantsKeys = participants.map(
      (participant) => participant.publicKey
    );
    participantsKeys.push(participant.publicKey);
    participantsKeys.push(otherParticipant.publicKey);

    const actualParticipantsSet = new Set(actualParticipants);
    const participantsKeysSet = new Set(participantsKeys);

    assert.deepEqual(actualParticipantsSet, participantsKeysSet);

    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(
      Number(gameVaultAfter.amount),
      10000 * (numberOfParticipants + 2)
    );

    // Obtain the transaction details
    // const txDetails = await program.provider.connection.getParsedTransaction(signature, {
    //   commitment: "confirmed",
    // });
    // console.log("Logs:", txDetails.meta.logMessages);

    // // Analize the Compute Units used
    // const computeUnitsUsed = txDetails.meta.computeUnitsConsumed;
    // console.log("Compute Units Consumidas:", computeUnitsUsed);
  });

  it("Start a Game Singlechain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 1);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    await program.methods
      .startGameSinglechain()
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);

    assert.equal(Number(gameAccount.id), Number(1));
    assert.deepEqual(gameAccount.status, { started: {} });
  });

  it("Create Game Omnichain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(
      program,
      ROOT,
      messengerOperator.publicKey
    );
    const game = getGamePDA(program, ROOT, 2);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    // Delegate some tokens to the messenger operator
    await buildAndSendApproveTransaction(
      anchor.getProvider(),
      participantVault,
      messengerOperator.publicKey,
      participant,
      500000000
    );
    await buildAndSendApproveTransaction(
      anchor.getProvider(),
      otherParticipantVault,
      messengerOperator.publicKey,
      otherParticipant,
      500000000
    );

    const ix = await program.methods
      .createGameOmnichain(new anchor.BN(2), new anchor.BN(10000), [
        participant.publicKey,
        otherParticipant.publicKey,
      ], true) // Wager, participants
      .accountsStrict({
        operator: messengerOperator.publicKey,
        operatorInfo,
        operatorVault: messengerOperatorVault,
        config: accounts.config,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([messengerOperator]);

    const instruction = await ix.instruction();
    const transaction = new anchor.web3.Transaction().add(instruction);

    const { blockhash } =
      await program.provider.connection.getLatestBlockhash();
    transaction.feePayer = messengerOperator.publicKey;
    transaction.recentBlockhash = blockhash;

    transaction.sign(messengerOperator);

    const serializedTransaction = transaction.serialize();
    const transactionSize = serializedTransaction.length;

    console.log("Serialized Tx Size:", transactionSize, "bytes");

    await ix.rpc();

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);
    assert.equal(Number(gameAccount.id), Number(2));
    assert.deepEqual(gameAccount.status, { started: {} });
    assert.equal(Number(gameAccount.wager), 10000);

    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(Number(gameVaultAfter.amount), 20000);
  });

  it("Create Game Omnichain with huge amount of participants", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(
      program,
      ROOT,
      messengerOperator.publicKey
    );
    const game = getGamePDA(program, ROOT, 3);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    const participantsKeys = participants.map(
      (participant) => participant.publicKey
    );

    const ix = await program.methods
      .createGameOmnichain(
        new anchor.BN(3),
        new anchor.BN(10000),
        participantsKeys,
        true
      ) // Wager, participants
      .accountsStrict({
        operator: messengerOperator.publicKey,
        operatorInfo,
        operatorVault: messengerOperatorVault,
        config: accounts.config,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([messengerOperator]);

    const instruction = await ix.instruction();
    const transaction = new anchor.web3.Transaction().add(instruction);

    const { blockhash } =
      await program.provider.connection.getLatestBlockhash();
    transaction.feePayer = messengerOperator.publicKey;
    transaction.recentBlockhash = blockhash;

    transaction.sign(messengerOperator);

    const serializedTransaction = transaction.serialize();
    const transactionSize = serializedTransaction.length;

    console.log("Serialized Tx Size:", transactionSize, "bytes");

    await ix.rpc();

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);
    assert.equal(Number(gameAccount.id), Number(3));
    assert.deepEqual(gameAccount.status, { started: {} });
    assert.equal(Number(gameAccount.wager), 10000);

    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(Number(gameVaultAfter.amount), 10000 * numberOfParticipants);

    const actualParticipants = gameAccount.participants;

    for (let i = 0; i < actualParticipants.length; i++) {
      assert.deepEqual(
        actualParticipants[i].toBase58(),
        participantsKeys[i].toString()
      );
    }
  });

  it("Create game from unapproved operator fails", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const notOperatorInfo = getOperatorInfo(
      program,
      ROOT,
      notOperator.publicKey
    );
    const game = getGamePDA(program, ROOT, 1);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    try {
      await program.methods
        .createGameSinglechain(new anchor.BN(10000)) // Wager
        .accountsStrict({
          operator: notOperator.publicKey,
          operatorInfo: notOperatorInfo,
          config: accounts.config,
          mint,
          game,
          gameVault,
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

  it("Finish Game Omnichain", async () => {
    // First init Treasury
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    const provider = anchor.getProvider();
    const treasuryVault = await token.getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      treasuryAccounts.authority,
      true
    );
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);

    // Second init Fees
    const platformWallet = anchor.web3.Keypair.generate();
    const platformWalletVault = await token.getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      platformWallet.publicKey,
      true
    );
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const numberOfBeneficiaries = 5;
    const beneficiaries = Array.from(
      { length: numberOfBeneficiaries },
      (_, i) => anchor.web3.Keypair.generate()
    );
    const beneficiariesKeys = beneficiaries.map(
      (beneficiary) => beneficiary.publicKey
    );
    let fractions = [];
    for (let i = 0; i < numberOfBeneficiaries; i++) {
      fractions.push(new anchor.BN(100 / numberOfBeneficiaries));
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
        new anchor.BN(100),
        beneficiariesKeys,
        fractions,
        new anchor.BN(100)
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

    const game = getGamePDA(program, ROOT, 2);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    const operatorAccountBefore = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    const platformWalletAccountBefore = await token.getAccount(
      program.provider.connection,
      platformWalletVault.address
    );

    try {
      const ix = await program.methods
        .finishGame(feeType, [participant.publicKey], [new anchor.BN(10000)]) // winners, prizes
        .accountsStrict({
          operator: operator.publicKey,
          operatorInfo,
          config: accounts.config,
          treasuryAuthority: treasuryAccounts.authority,
          treasuryVault: treasuryVault.address,
          platformWallet: platformWallet.publicKey,
          platformWalletVault: platformWalletVault.address,
          feeMeta,
          gameVault: gameVault,
          game,
          mint,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: winnerInfo, isSigner: false, isWritable: true },
        ])
        .signers([operator]);

      const instruction = await ix.instruction();
      const transaction = new anchor.web3.Transaction().add(instruction);

      const { blockhash } =
        await program.provider.connection.getLatestBlockhash();
      transaction.feePayer = operator.publicKey;
      transaction.recentBlockhash = blockhash;

      transaction.sign(operator);

      const serializedTransaction = transaction.serialize();
      const transactionSize = serializedTransaction.length;

      console.log("Serialized Tx Size:", transactionSize, "bytes");

      await ix.rpc();
    } catch (err) {
      if (err instanceof anchor.AnchorError) {
        console.error("Transaction failed with AnchorError:", err.errorLogs);
      } else {
        console.error("Transaction failed with error:", err);
      }
    }

    // Game Vault should be empty
    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );
    assert.equal(Number(gameVaultAfter.amount), 0);

    // The operator should have the same amount as before
    const operatorAccountAfter = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    assert.equal(
      Number(operatorAccountAfter.amount),
      Number(operatorAccountBefore.amount)
    );

    // The platform wallet should remain the same because the feeType is not 0
    // and the fractions sum feeMeta.baseFee
    const platformWalletAccountAfter = await token.getAccount(
      program.provider.connection,
      platformWalletVault.address
    );
    assert.equal(
      Number(platformWalletAccountAfter.amount),
      Number(platformWalletAccountBefore.amount)
    );

    // the beneficiaries should have 40 pending to claim
    const feeMetaAfter = await program.account.feeMeta.fetch(feeMeta);
    const actualBeneficiaries = feeMetaAfter.beneficiaries.map(
      (pubkey: anchor.web3.PublicKey) => pubkey.toBase58()
    );
    const actualFractions = feeMetaAfter.fractions.map((fraction: anchor.BN) =>
      fraction.toNumber()
    );
    const actualPendingToClaim = feeMetaAfter.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );

    const expectedPendingToClaim = [40, 40, 40, 40, 40];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);

    // Validate the user info of the winner
    const winnerInfoAfter = await program.account.claimableUserInfo.fetch(
      winnerInfo
    );
    assert.equal(Number(winnerInfoAfter.claimable), 19800);
  });

  it("Cancel Game", async () => {
    // Create a new game
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    const provider = anchor.getProvider();
    const treasuryVault = await token.getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      treasuryAccounts.authority,
      true
    );
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 4);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    // Delegate some tokens to the operator
    await buildAndSendApproveTransaction(
      anchor.getProvider(),
      participantVault,
      operator.publicKey,
      participant,
      500000000
    );
    await buildAndSendApproveTransaction(
      anchor.getProvider(),
      otherParticipantVault,
      operator.publicKey,
      otherParticipant,
      500000000
    );

    const tx = await program.methods
      .createGameSinglechain(new anchor.BN(10000)) // Wager
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        config: accounts.config,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts([
        { pubkey: participantVault, isSigner: false, isWritable: true },
        { pubkey: otherParticipantVault, isSigner: false, isWritable: true },
      ])
      .signers([operator])
      .rpc();

    await program.methods
      .startGameSinglechain()
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        mint,
        game,
        gameVault,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();

    // get the User info of the winner
    const participantInfo = getUserInfo(program, ROOT, participant.publicKey);
    const otherParticipantInfo = getUserInfo(
      program,
      ROOT,
      otherParticipant.publicKey
    );

    const participantInfoBefore = await program.account.claimableUserInfo.fetch(
      participantInfo
    );

    // Cancel the game
    await program.methods
      .cancelGame()
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        treasuryAuthority: treasuryAccounts.authority,
        treasuryVault: treasuryVault.address,
        game,
        gameVault,
        mint,
        tokenProgram: token.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .remainingAccounts([
        { pubkey: participantInfo, isSigner: false, isWritable: true },
        { pubkey: otherParticipantInfo, isSigner: false, isWritable: true },
      ])
      .rpc();

    const gameAccount = await program.account.game.fetch(game);
    assert.deepEqual(gameAccount.status, { canceled: {} });

    // Game Vault should be empty
    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );
    assert.equal(Number(gameVaultAfter.amount), 0);

    // Validate the user info of the winner
    const participantInfoAfter = await program.account.claimableUserInfo.fetch(
      participantInfo
    );
    assert.equal(
      Number(participantInfoAfter.claimable),
      Number(participantInfoBefore.claimable) + 10000
    );
    const otherParticipantInfoAfter =
      await program.account.claimableUserInfo.fetch(otherParticipantInfo);
    assert.equal(Number(otherParticipantInfoAfter.claimable), 10000);
  });
});
