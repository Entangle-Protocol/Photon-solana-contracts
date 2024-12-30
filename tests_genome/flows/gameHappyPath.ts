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
import { getTreasuryAccounts, getUserInfo } from "../../genome_test_setup/treasury";
import { getFeeMeta } from "../../genome_test_setup/feeProvider";
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
  const platformWallet = anchor.web3.Keypair.generate();
  const numberOfBeneficiaries = 5;
  const beneficiaries = Array.from({ length: numberOfBeneficiaries }, (_, i) =>
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
  let platformWalletVault: anchor.web3.PublicKey;
  let beneficieriesVaults = [];
  let treasuryVault;

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
      admin,
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
      admin,
      1000000000
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

  it("Create Game Omnichain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(
      program,
      ROOT,
      messengerOperator.publicKey
    );
    const game = getGamePDA(program, ROOT, 0);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    const participantsKeys = participants.map(
      (participant) => participant.publicKey
    );
    // Add the participant
    participantsKeys.push(participant.publicKey);

    const ix = await program.methods
      .createGameOmnichain(
        new anchor.BN(0),
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
    assert.equal(Number(gameAccount.id), Number(0));
    assert.deepEqual(gameAccount.status, { started: {} });
    assert.equal(Number(gameAccount.wager), 10000);

    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(
      Number(gameVaultAfter.amount),
      10000 * (numberOfParticipants + 1)
    );

    const actualParticipants = gameAccount.participants;

    for (let i = 0; i < actualParticipants.length; i++) {
      assert.deepEqual(
        actualParticipants[i].toBase58(),
        participantsKeys[i].toString()
      );
    }
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

  it("Finish Game Omnichain with fee_type == 1", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const game = getGamePDA(program, ROOT, 0);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    const operatorAccountBefore = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    const platformWalletAccountBefore = await token.getAccount(
      program.provider.connection,
      platformWalletVault
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
          platformWalletVault: platformWalletVault,
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
      platformWalletVault
    );
    assert.equal(
      Number(platformWalletAccountAfter.amount),
      Number(platformWalletAccountBefore.amount)
    );

    // the beneficiaries should have 120 pending to claim
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

    const expectedPendingToClaim = [120, 120, 120, 120, 120];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);

    // Validate the user info of the winner
    const winnerInfoAfter = await program.account.claimableUserInfo.fetch(
      winnerInfo
    );
    assert.equal(Number(winnerInfoAfter.claimable), 59400);
  });

  it("Winner claims his reward", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);

    const winnerInfoBefore = await program.account.claimableUserInfo.fetch(
      winnerInfo
    );
    assert.equal(Number(winnerInfoBefore.claimable), 59400);
    const participantVaultBefore = await token.getAccount(
      program.provider.connection,
      participantVault
    );

    try {
      const ix = await program.methods
        .withdrawRewards()
        .accountsStrict({
          operator: operator.publicKey,
          authority: treasuryAccounts.authority,
          operatorInfo,
          mint,
          vault: treasuryVault.address,
          operatorVault,
          user: participant.publicKey,
          claimableUserInfo: winnerInfo,
          userVault: participantVault,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
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

    // Validate the user info of the winner
    const winnerInfoAfter = await program.account.claimableUserInfo.fetch(
      winnerInfo
    );
    assert.equal(Number(winnerInfoAfter.claimable), 0);

    // Validate the user vault of the winner
    const participantVaultAfter = await token.getAccount(
      program.provider.connection,
      participantVault
    );
    assert.equal(
      Number(participantVaultAfter.amount),
      Number(participantVaultBefore.amount) + 59400
    );
  });

  it("Beneficiary (fee_type == 1) claims his reward", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const beneficiary = beneficiaries[0];
    const beneficiaryVault = await token.getOrCreateAssociatedTokenAccount(
      program.provider.connection,
      admin,
      mint,
      beneficiary.publicKey,
      true
    );

    try {
      const ix = await program.methods
        .claimBeneficiaryFees(feeType)
        .accountsStrict({
          operator: operator.publicKey,
          authority: treasuryAccounts.authority,
          treasuryVault: treasuryVault.address,
          operatorInfo,
          mint,
          beneficiary: beneficiary.publicKey,
          beneficiaryVault: beneficiaryVault.address,
          feeMeta: feeMeta,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
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

    // Validate the beneficiary vault
    const beneficiaryVaultAfter = await token.getAccount(
      program.provider.connection,
      beneficiaryVault.address
    );
    assert.equal(Number(beneficiaryVaultAfter.amount), 120);

    // Validate the feeMeta
    const feeMetaAfter = await program.account.feeMeta.fetch(feeMeta);
    const actualPendingToClaim = feeMetaAfter.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );
    const expectedPendingToClaim = [0, 120, 120, 120, 120];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);
  });

  it("Beneficiary (fee_type == 1) claims his reward again fails", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const beneficiary = beneficiaries[0];
    const beneficiaryVault = await token.getOrCreateAssociatedTokenAccount(
      program.provider.connection,
      admin,
      mint,
      beneficiary.publicKey,
      true
    );

    try {
      const ix = await program.methods
        .claimBeneficiaryFees(feeType)
        .accountsStrict({
          operator: operator.publicKey,
          authority: treasuryAccounts.authority,
          treasuryVault: treasuryVault.address,
          operatorInfo,
          mint,
          beneficiary: beneficiary.publicKey,
          beneficiaryVault: beneficiaryVault.address,
          feeMeta: feeMeta,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
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
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof anchor.AnchorError);
    }
  });

  it("The rest of beneficiaries (fee_type == 1) claim their reward except for the last one", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const selectedBeneficiaries = beneficiaries.slice(1, 4);
    for (const beneficiary of selectedBeneficiaries) {
      const beneficiaryVault = await token.getOrCreateAssociatedTokenAccount(
        program.provider.connection,
        admin,
        mint,
        beneficiary.publicKey,
        true
      );

      try {
        const ix = await program.methods
          .claimBeneficiaryFees(feeType)
          .accountsStrict({
            operator: operator.publicKey,
            authority: treasuryAccounts.authority,
            treasuryVault: treasuryVault.address,
            operatorInfo,
            mint,
            beneficiary: beneficiary.publicKey,
            beneficiaryVault: beneficiaryVault.address,
            feeMeta: feeMeta,
            tokenProgram: token.TOKEN_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
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

      // Validate the beneficiary vault
      const beneficiaryVaultAfter = await token.getAccount(
        program.provider.connection,
        beneficiaryVault.address
      );
      assert.equal(Number(beneficiaryVaultAfter.amount), 120);
    }

    // Validate the feeMeta
    const feeMetaAfter = await program.account.feeMeta.fetch(feeMeta);
    const actualPendingToClaim = feeMetaAfter.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );
    const expectedPendingToClaim = [0, 0, 0, 0, 120];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);
  });

  it("Set fee params again: fee_type == 1 with new beneficiaries except for the last one (his pending claim remains the same)", async () => {
    const baseFee = 100;
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const numberNewOfBeneficiaries = 4;
    const newBeneficiaries = Array.from(
      { length: numberNewOfBeneficiaries },
      (_, i) => anchor.web3.Keypair.generate()
    );
    newBeneficiaries.push(beneficiaries[5]);
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

    // Validate the feeMeta
    const feeMetaAfter = await program.account.feeMeta.fetch(feeMeta);
    const actualPendingToClaim = feeMetaAfter.pendingToClaim.map(
      (pending: anchor.BN) => pending.toNumber()
    );
    const expectedPendingToClaim = [0, 0, 0, 0, 120];

    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);
  });

  it("Create Game Singlechain", async () => {
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

    // Validate the game params
    const gameAccount = await program.account.game.fetch(game);

    assert.equal(Number(gameAccount.id), Number(1));
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

  it("Register more participants in the Game Singlechain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 1);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

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
  });

  it("Start the Game Singlechain", async () => {
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

  it("Set fee params: fee_type == 0", async () => {
    const baseFee = 100;
    const feeType = 0;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const beneficiariesKeys = [];
    let fractions = [];

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

  it("Finish Game SingleChain with fee_type == 0", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    // get the User info of the winner
    const winnerInfo = getUserInfo(program, ROOT, participant.publicKey);
    const feeType = 0;
    const feeMeta = getFeeMeta(program, ROOT, feeType);

    const game = getGamePDA(program, ROOT, 1);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);

    const operatorAccountBefore = await token.getAccount(
      program.provider.connection,
      operatorVault
    );
    const platformWalletAccountBefore = await token.getAccount(
      program.provider.connection,
      platformWalletVault
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
          platformWalletVault: platformWalletVault,
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
      platformWalletVault
    );
    assert.equal(
      Number(platformWalletAccountAfter.amount),
      Number(platformWalletAccountBefore.amount) + 700
    );

    // Validate the user info of the winner
    const winnerInfoAfter = await program.account.claimableUserInfo.fetch(
      winnerInfo
    );
    assert.equal(Number(winnerInfoAfter.claimable), 69300);
  });

  it("Start Another Game SingleChain", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 2);
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
      ])
      .signers([operator])
      .rpc();

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
  });

  it("Pre cancel the game", async () => {
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 2);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);
    const participantInfo = getUserInfo(program, ROOT, participant.publicKey);

    const participantInfoBefore = await program.account.claimableUserInfo.fetch(
      participantInfo
    );

    const remainingAcountsObj = [];

    for (let i = 0; i < participants.length; i++) {
      const participantsVecInfo = getUserInfo(
        program,
        ROOT,
        participants[i].publicKey
      );
      remainingAcountsObj.push({
        pubkey: participantsVecInfo,
        isSigner: false,
        isWritable: true,
      });
    }

    const gameVaultBefore = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    // Cancel the game
    const ix = await program.methods
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
      .remainingAccounts(remainingAcountsObj);

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

    const gameAccount = await program.account.game.fetch(game);
    assert.deepEqual(gameAccount.status, { preCanceled: {} });

    // Game Vault should be the same
    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(Number(gameVaultAfter.amount), Number(gameVaultBefore.amount));

    // Validate the participant info: should have the same claimable
    const participantInfoAfter = await program.account.claimableUserInfo.fetch(
      participantInfo
    );
    assert.equal(
      Number(participantInfoAfter.claimable),
      Number(participantInfoBefore.claimable)
    );

    // Validate the other participants info: should have the same claimable
    for (let i = 0; i < participants.length; i++) {
      const participantInfo = getUserInfo(
        program,
        ROOT,
        participants[i].publicKey
      );
      const participantInfoAfter =
        await program.account.claimableUserInfo.fetch(participantInfo);
      assert.equal(
        Number(participantInfoAfter.claimable),
        Number(gameAccount.wager)
      );
    }
  });

  it("Cancel the game", async () => {
    const treasuryAccounts = getTreasuryAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const game = getGamePDA(program, ROOT, 2);
    const gameVault = token.getAssociatedTokenAddressSync(mint, game, true);
    const participantInfo = getUserInfo(program, ROOT, participant.publicKey);

    const participantInfoBefore = await program.account.claimableUserInfo.fetch(
      participantInfo
    );

    const remainingAcountsObj = [];

    remainingAcountsObj.push({
      pubkey: participantInfo,
      isSigner: false,
      isWritable: true,
    });

    // Cancel the game
    const ix = await program.methods
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
      .remainingAccounts(remainingAcountsObj);

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

    const gameAccount = await program.account.game.fetch(game);
    assert.deepEqual(gameAccount.status, { canceled: {} });

    // Game Vault should be the empty
    const gameVaultAfter = await token.getAccount(
      program.provider.connection,
      gameVault
    );

    assert.equal(Number(gameVaultAfter.amount), 0);

    // Validate the participant info: should have the same claimable
    const participantInfoAfter = await program.account.claimableUserInfo.fetch(
      participantInfo
    );
    assert.equal(
      Number(participantInfoAfter.claimable),
      Number(participantInfoBefore.claimable) + Number(gameAccount.wager)
    );
  });
});
