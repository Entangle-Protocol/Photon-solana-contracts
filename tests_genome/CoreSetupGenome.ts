import * as anchor from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { Program } from "@coral-xyz/anchor";
import { Genome } from "../target/types/genome";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  approveOperator,
  getOperatorInfo,
  getGenomeAccounts,
  init,
} from "../genome_test_setup/genome";
import { assert } from "chai";

describe("zs-single-solana: Setup", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  // ------------ ROOT ----------------
  const ROOT = utf8.encode("genome-root");

  // ------------ PROGRAMS ----------------
  const program = anchor.workspace.Genome as Program<Genome>;

  // ------------ KEYS ----------------
  const admin = anchor.web3.Keypair.generate();
  const operator = anchor.web3.Keypair.generate();
  const messengerOperator = anchor.web3.Keypair.generate();

  // ------------ TOKENS AND VAULTS ----------------

  let mint: anchor.web3.PublicKey;
  let operatorVault: anchor.web3.PublicKey;
  let messengerOperatorVault: anchor.web3.PublicKey;

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
      admin.publicKey,
      1000000000
    );
  });

  it("Is initialized!", async () => {
    await init(ROOT, program, admin);
    // Validate the config account
    const accounts = getGenomeAccounts(program, ROOT);

    const adminOperatorInfo = getOperatorInfo(program, ROOT, admin.publicKey);

    const configAccount = await program.account.genomeConfig.fetch(
      accounts.config
    );
    assert.equal(Number(configAccount.gamesConfig.gamesCounter), Number(0));
    assert.equal(
      Number(configAccount.tournamentConfig.tournamentCount),
      Number(0)
    );

    const adminAccount = await program.account.operatorInfo.fetch(
      adminOperatorInfo
    );

    assert.deepEqual(adminAccount.role, { owner: {} });

    assert.equal(adminAccount.approved, true);
  });

  it("Approve operator", async () => {
    const role = { backend: {} };
    await approveOperator(program, ROOT, admin, operator.publicKey, role);
    const messengerRole = { messenger: {} };
    await approveOperator(
      program,
      ROOT,
      admin,
      messengerOperator.publicKey,
      messengerRole
    );
  });
});
