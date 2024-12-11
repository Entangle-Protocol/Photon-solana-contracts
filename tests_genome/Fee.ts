import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { Program } from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { getFeeMeta } from "../genome_test_setup/feeProvider";
import { getGenomeAccounts, init, approveOperator, getOperatorInfo } from "../genome_test_setup/genome";
import { assert } from "chai";

describe("zs-single-solana: Fees", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const ROOT = utf8.encode("genome-root");
  const program = anchor.workspace.Genome as Program<Genome>;

  const admin = anchor.web3.Keypair.generate();
  const operator = anchor.web3.Keypair.generate();
  const notOperator = anchor.web3.Keypair.generate();
  const wallet = anchor.web3.Keypair.generate();
  const numberOfBeneficiaries = 5;
  const beneficiaries = Array.from({ length: numberOfBeneficiaries }, (_, i) => anchor.web3.Keypair.generate());

  let mint: anchor.web3.PublicKey;
  let operatorVault: anchor.web3.PublicKey;
  let platformWalletVault: anchor.web3.PublicKey;
  let participantVaults: anchor.web3.PublicKey[] = [];

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
    platformWalletVault = await token.createAccount(
      provider.connection,
      admin,
      mint,
      wallet.publicKey
    );
    // --------- Multiple Beneficiaries Array ----------------
    for (const beneficiary of beneficiaries) {
      const vault = await token.createAssociatedTokenAccount(
        provider.connection,
        admin,
        mint,
        beneficiary.publicKey
      );
      participantVaults.push(vault);
    }

  });
    
  it("Is initialized!", async () => {
    await init(ROOT, program, admin);
  });

  it("Approve operator", async () => {
    const role = {developer:{}};
    await approveOperator(program, ROOT, admin, operator.publicKey, role);
    const developerRole = { developer: {} };
    await approveOperator(program, ROOT, admin, notOperator.publicKey, developerRole);
  });

  it("Set Fee Params with feeType != 0", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const feeType = 1;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const beneficiariesKeys = beneficiaries.map((beneficiary) => beneficiary.publicKey);
    let fractions = [];
    for (let i = 0; i < numberOfBeneficiaries; i++) {
      fractions.push(new anchor.BN(100/numberOfBeneficiaries));
    }

    await program.methods
      .setFeeParams(feeType, wallet.publicKey, new anchor.BN(100), beneficiariesKeys, fractions, new anchor.BN(100))
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        feeMeta,
        config: accounts.config,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();

    // Validate the fee params
    const feeMetaAccount = await program.account.feeMeta.fetch(feeMeta);

    assert.equal(
      feeMetaAccount.baseFee.toNumber(),
      100
    );

    const actualBeneficiaries = feeMetaAccount.beneficiaries.map((beneficiary) => beneficiary.toBase58());
    const expectedBeneficiaries = beneficiaries.map((beneficiary) => beneficiary.publicKey.toBase58());
    assert.deepEqual(actualBeneficiaries, expectedBeneficiaries);

    const actualFractions = feeMetaAccount.fractions.map((fraction) => fraction.toNumber());
    const expectedFractions = fractions.map((fraction) => fraction.toNumber());
    assert.deepEqual(actualFractions, expectedFractions);

    const actualPendingToClaim = feeMetaAccount.pendingToClaim.map((pending) => pending.toNumber());
    const expectedPendingToClaim = Array.from({ length: numberOfBeneficiaries }, () => 0);
    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);
  });

  it("Set Fee Params with feeType == 0", async () => {
    const accounts = getGenomeAccounts(program, ROOT);
    const operatorInfo = getOperatorInfo(program, ROOT, operator.publicKey);
    const feeType = 0;
    const feeMeta = getFeeMeta(program, ROOT, feeType);
    const beneficiariesKeys = beneficiaries.map((beneficiary) => beneficiary.publicKey);
    let fractions = [];
    for (let i = 0; i < numberOfBeneficiaries; i++) {
      fractions.push(new anchor.BN(100/numberOfBeneficiaries));
    }

    await program.methods
      .setFeeParams(feeType, wallet.publicKey, new anchor.BN(100), beneficiariesKeys, fractions, new anchor.BN(100))
      .accountsStrict({
        operator: operator.publicKey,
        operatorInfo,
        feeMeta,
        config: accounts.config,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([operator])
      .rpc();

    // Validate the fee params
    const feeMetaAccount = await program.account.feeMeta.fetch(feeMeta);

    assert.equal(
      feeMetaAccount.baseFee.toNumber(),
      0
    );

    const actualBeneficiaries = feeMetaAccount.beneficiaries.map((beneficiary) => beneficiary.toBase58());
    const expectedBeneficiaries = [];
    assert.deepEqual(actualBeneficiaries, expectedBeneficiaries);

    const actualFractions = feeMetaAccount.fractions.map((fraction) => fraction.toNumber());
    const expectedFractions = [];
    assert.deepEqual(actualFractions, expectedFractions);

    const actualPendingToClaim = feeMetaAccount.pendingToClaim.map((pending) => pending.toNumber());
    const expectedPendingToClaim = [];
    assert.deepEqual(actualPendingToClaim, expectedPendingToClaim);

    const configAccount = await program.account.genomeConfig.fetch(accounts.config);
    assert.equal(
      configAccount.feesConfig.platformWallet.toBase58().toString(),
      wallet.publicKey.toBase58().toString()
    );
    assert.equal(
      Number(configAccount.feesConfig.baseFee),
      100
    );
  });
});
