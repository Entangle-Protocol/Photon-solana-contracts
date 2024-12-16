import * as anchor from "@coral-xyz/anchor";
import { AnchorError, BN, BorshCoder, EventParser, Program, web3 } from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import {
    AuthorityType,
    createMint,
    getOrCreateAssociatedTokenAccount,
    setAuthority,
    TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Metaplex, PublicKey } from "@metaplex-foundation/js";
import { PROGRAM_ADDRESS } from "@metaplex-foundation/mpl-token-metadata";
import { Photon } from "../target/types/photon";
import { Onefunc } from "../target/types/onefunc";
import { Genome } from "../target/types/genome";
import { NglCore } from "../target/types/ngl_core";
import { bs58, utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

import {
    addAllowedProtocol,
    addAllowedProtocolAddress,
    addExecutor,
    addTransmitter,
    hexToBytes,
    opHashFull,
    predefinedSigners,
    randomSigners,
    setConsensusTargetRate,
    signOp,
    sleep,
    updateTransmitter,
} from "./utils";
import { ethers, Wallet } from "ethers";
import { assert, expect } from "chai";

const TEST_REMOVE_FUNCS = true;
const ROOT = utf8.encode("r0");
const EOB_CHAIN_ID = 33133;
const SOLANA_CHAIN_ID = "100000000000000000000";
const CONSENSUS_TARGET_RATE = 6000;
const TRANSMITTERS = 3;
const TRANSMITTERS_PER_CALL = 4;
const GOV_PROTOCOL_ID = Buffer.from(
    utf8.encode(
        "photon-gov\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
    )
);
const GENOME_ID = Buffer.from(utf8.encode("Genome--------------------------"));
const ONE_FUNC_ID = Buffer.from(utf8.encode("onefunc_________________________"));

type FunctionSelector = anchor.IdlTypes<Photon>["FunctionSelector"];

describe("photon", () => {
    // Configure the client to use the local cluster.
    anchor.setProvider(anchor.AnchorProvider.env());
    const program = anchor.workspace.Photon as Program<Photon>;
    const onefunc = anchor.workspace.Onefunc as Program<Onefunc>;
    const genome = anchor.workspace.Genome as Program<Genome>;
    const nglCore = anchor.workspace.NglCore as Program<NglCore>;

    let owner_keypair = Uint8Array.from(require("../keys/owner.json"));
    let owner = anchor.web3.Keypair.fromSecretKey(owner_keypair);

    let executor_keypair = Uint8Array.from(require("../keys/gov-executor.json"));
    const executor = anchor.web3.Keypair.fromSecretKey(executor_keypair);
    let zsExecutor = executor;
    console.log("Executor:", executor.publicKey);

    let config;
    let govProtocolInfo;
    let counter;
    let proposer;
    let callAuthority;
    let govCallAuthority;
    let transmitters: Wallet[];
    let transmittersRaw = [];
    let nonce = 0;
    let onefuncProtocol;

    const zsRoot = utf8.encode("genome-root");
    let zsConfig;
    let zsPhotonOperatorInfo;
    let zsProposer;
    let zsTreasuryAuthority = web3.PublicKey.findProgramAddressSync(
        [zsRoot, utf8.encode("AUTHORITY")],
        genome.programId
    )[0];
    let zsCallAuthority;
    let zsGameWinner;
    let nglTokenConfig;
    let nglVault;
    let nglAuthority;
    let nglMint;

    before(async () => {
        let tx = await program.provider.connection.requestAirdrop(
            owner.publicKey,
            5 * anchor.web3.LAMPORTS_PER_SOL
        );
        await program.provider.connection.confirmTransaction(tx);
        tx = await program.provider.connection.requestAirdrop(
            executor.publicKey,
            5 * anchor.web3.LAMPORTS_PER_SOL
        );
        await program.provider.connection.confirmTransaction(tx);

        config = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CONFIG")],
            program.programId
        )[0];
        govProtocolInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
            program.programId
        )[0];
        console.log("Gov protocol info", govProtocolInfo.toBase58());
        console.log("Executor", executor.publicKey.toBase58());
        transmitters = predefinedSigners(TRANSMITTERS);
        for (var i = 0; i < transmitters.length; i++) {
            console.log("Transmitter", i, transmitters[i].address);
            transmittersRaw.push(hexToBytes(transmitters[i].address));
        }
        callAuthority = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CALL_AUTHORITY"), ONE_FUNC_ID],
            program.programId
        )[0];
        zsCallAuthority = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CALL_AUTHORITY"), GENOME_ID],
            program.programId
        )[0];
        govCallAuthority = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CALL_AUTHORITY"), GOV_PROTOCOL_ID],
            program.programId
        )[0];
        counter = web3.PublicKey.findProgramAddressSync(
            [utf8.encode("COUNTER")],
            onefunc.programId
        )[0];
        onefuncProtocol = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), ONE_FUNC_ID],
            program.programId
        )[0];

        proposer = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROPOSER")],
            onefunc.programId
        )[0];

        zsProposer = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROPOSER")],
            genome.programId
        )[0];

        console.log("NGL", nglCore.programId, "GEN", genome.programId, "Photon", program.programId);
    });

    async function executeOperation(
        protocolId: Buffer,
        protocolAddr: anchor.web3.PublicKey,
        functionSelector: number | string | Buffer,
        params: Buffer,
        targetProtocol: Buffer,
        remainingAccounts?: anchor.web3.AccountMeta[]
    ) {
        let fs: FunctionSelector;
        if (typeof functionSelector == "number") {
            let functionSelectorBuf = Buffer.alloc(4);
            functionSelectorBuf.writeUInt32BE(functionSelector);
            fs = { byCode: [functionSelectorBuf] };
        } else if (typeof functionSelector == "string") {
            fs = { byName: [functionSelector] };
        } else {
            assert(false, "Unexpected functionSelector");
        }
        let meta = new anchor.BN(
            "0100000000000000000000000000000000000000000000000000000000000000",
            16
        ).toBuffer();
        let op = {
            protocolId,
            meta,
            srcChainId: new anchor.BN(EOB_CHAIN_ID),
            srcBlockNumber: new anchor.BN(1),
            srcOpTxId: hexToBytes(
                "ce25f58a7fd8625deadc00a59b67c530c7d92acec1e5753c588269ade6ebf99f"
            ),
            nonce: new anchor.BN(nonce),
            destChainId: new anchor.BN(SOLANA_CHAIN_ID),
            protocolAddr,
            functionSelector: fs,
            params,
            reserved: Buffer.from([]),
        };
        let op_hash = opHashFull(op);
        let opInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("OP"), op_hash],
            program.programId
        )[0];
        let protocolInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), op.protocolId],
            program.programId
        )[0];
        // Load
        let signature = await program.methods
            .loadOperation(op, op_hash)
            .accounts({
                executor: executor.publicKey,
                protocolInfo,
                opInfo,
                config,
                systemProgram: web3.SystemProgram.programId,
            })
            .signers([executor])
            .rpc();
        console.log("load_operation:", signature);
        // Sign
        const chunkSize = TRANSMITTERS_PER_CALL;
        // console.debug("load_operation:", signature);
        let signatures = [];
        for (let i = 0; i < transmitters.length; i++) {
            const sig = await signOp(transmitters[i], op);
            signatures.push(sig);
        }
        for (let i = 0; i < signatures.length; i += chunkSize) {
            const chunk = signatures.slice(i, i + chunkSize);
            let signature = await program.methods
                .signOperation(op_hash, chunk)
                .accounts({
                    executor: executor.publicKey,
                    opInfo,
                    protocolInfo,
                })
                .signers([executor])
                .rpc();
            console.debug("sign_operation:", signature);
        }
        // Execute
        if (protocolId.equals(GOV_PROTOCOL_ID)) {
            let target_protocol_info_pda = web3.PublicKey.findProgramAddressSync(
                [ROOT, utf8.encode("PROTOCOL"), targetProtocol],
                program.programId
            )[0];
            let config_pda = web3.PublicKey.findProgramAddressSync(
                [ROOT, utf8.encode("CONFIG")],
                program.programId
            )[0];

            let signature = await program.methods
                .executeOperation(op_hash)
                .accounts({
                    executor: executor.publicKey,
                    opInfo,
                    protocolInfo: govProtocolInfo,
                    callAuthority: govCallAuthority,
                })
                .remainingAccounts([
                    { pubkey: program.programId, isSigner: false, isWritable: false },
                    { pubkey: config_pda, isSigner: false, isWritable: true },
                    { pubkey: govProtocolInfo, isSigner: false, isWritable: true },
                    { pubkey: target_protocol_info_pda, isSigner: false, isWritable: true },
                    { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: true },
                ])
                .signers([executor])
                .rpc();
            console.debug("execute_gov_operation:", signature);
        } else if (protocolId.equals(ONE_FUNC_ID)) {
            let signature = await program.methods
                .executeOperation(op_hash)
                .accounts({
                    executor: executor.publicKey,
                    opInfo,
                    protocolInfo,
                    callAuthority,
                })
                .signers([executor])
                .remainingAccounts(remainingAccounts)
                .rpc();
            console.debug("execute_operation:", signature);
        } else if (protocolId.equals(GENOME_ID)) {
            const modifyComputeUnits =
                anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
                    units: 300_000,
                });
            let signature = await program.methods
                .executeOperation(op_hash)
                .preInstructions([modifyComputeUnits])
                .accountsStrict({
                    executor: executor.publicKey,
                    opInfo,
                    protocolInfo,
                    callAuthority: zsCallAuthority,
                })
                .signers([executor])
                .remainingAccounts(remainingAccounts)
                .rpc();
            console.debug("execute_operation:", signature);
        }
        console.log("Proposal", nonce, "executed");
        nonce++;
    }

    it("Initialize NGL Token", async () => {
        // Find PDAs
        [nglAuthority] = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("AUTHORITY")],
            nglCore.programId
        );
        [nglTokenConfig] = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CONFIG")],
            nglCore.programId
        );

        // Create a mint with decimals = 6
        nglMint = await createMint(
            program.provider.connection,
            owner, // fee payer
            nglAuthority, // mint authority
            null, // freeze authority
            6 // decimals
        );

        // Derive metadata PDA for the mint
        const metaplex = Metaplex.make(program.provider.connection);
        const metadataPDA = metaplex.nfts().pdas().metadata({ mint: nglMint });

        // Prepare transaction to initialize the contract
        await nglCore.methods
            .initialize(
                executor.publicKey,
                "MyToken",
                "MTK",
                "https://example.com/metadata.json" // dummy URI
            )
            .accounts({
                admin: executor.publicKey,
                authority: nglAuthority,
                mint: nglMint,
                config: nglTokenConfig,
                metadataAccount: metadataPDA,
                rent: web3.SYSVAR_RENT_PUBKEY,
                tokenMetadataProgram: new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"), // This is the Metaplex metadata program ID
                tokenProgram: TOKEN_PROGRAM_ID,
                systemProgram: web3.SystemProgram.programId,
            })
            .signers([executor])
            .rpc();

        // Create or get associated token account for admin
        const vaultAccount = await getOrCreateAssociatedTokenAccount(
            program.provider.connection,
            owner, // payer
            nglMint,
            executor.publicKey,
            true
        );

        nglVault = vaultAccount.address;

        // Call `mint_token` instruction
        // We'll use the `bridgeAuthority` as the authorized contract for minting.
        // In practice, ensure `bridgeAuthority` is one of the allowed contracts.
        await nglCore.methods
            .mintToken(
                new BN(1) // amount, e.g. 1 token with 6 decimals = 1,000,000
            )
            .accounts({
                mintAuthority: executor.publicKey,
                authority: nglAuthority,
                config: nglTokenConfig,
                mint: nglMint,
                vault: nglVault,
                tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([executor])
            .rpc();

        // At this point, `mint_token` has been called and tokens have been minted into `vault`.
        const vaultInfo = await program.provider.connection.getTokenAccountBalance(nglVault);
        console.log("Vault balance:", vaultInfo.value.amount); // Should show 1000000
    });

    it("Initialize", async () => {
        zsConfig = anchor.web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CONFIG")],
            genome.programId
        )[0];
        const operatorInfo = anchor.web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("OPERATOR"), owner.publicKey.toBuffer()],
            genome.programId
        )[0];
        await genome.methods
            .initialize()
            .accountsStrict({
                admin: owner.publicKey,
                config: zsConfig,
                operatorInfo,
                systemProgram: web3.SystemProgram.programId,
            })
            .signers([owner])
            .rpc();

        console.log("Successfully initialized GENOME!");

        await onefunc.methods
            .initialize()
            .accounts({ owner: owner.publicKey, callAuthority, counter })
            .signers([owner])
            .rpc();

        let eob_master_contract = ethers.utils.defaultAbiCoder.encode(
            ["address"],
            ["0xe981b4f9580cce1ff1b87d63a9b68e53110b9aa7"]
        );
        let eob_master_contract_buf: Buffer = new Buffer(eob_master_contract.substring(2), "hex");

        await program.methods
            .initialize(
                new anchor.BN(EOB_CHAIN_ID),
                eob_master_contract_buf,
                new anchor.BN(CONSENSUS_TARGET_RATE),
                [transmittersRaw[0]],
                [executor.publicKey]
            )
            .accounts({
                admin: owner.publicKey,
                protocolInfo: govProtocolInfo,
                config,
                systemProgram: web3.SystemProgram.programId,
            })
            .signers([owner])
            .rpc();
        const chunkSize = TRANSMITTERS_PER_CALL;
        for (let i = 1; i < transmittersRaw.length; i += chunkSize) {
            const chunk = transmittersRaw.slice(i, i + chunkSize);
            const params = addTransmitter(GOV_PROTOCOL_ID, chunk);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x6c5f5666,
                params,
                GOV_PROTOCOL_ID
            );
        }
    });

    it("addAllowedProtocol", async () => {
        // 2: Genome to be added as allowed protocol
        let params = addAllowedProtocol(ONE_FUNC_ID, [], CONSENSUS_TARGET_RATE);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x45a004b9, params, ONE_FUNC_ID);

        // Genome add allowed protocol
        let zsParams = addAllowedProtocol(GENOME_ID, [], CONSENSUS_TARGET_RATE);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x45a004b9, zsParams, GENOME_ID);
    });

    it("setConsensusTargetRate", async () => {
        // 3: Set consensus target rate for Genome
        let params = setConsensusTargetRate(ONE_FUNC_ID, 6000);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x970b6109, params, ONE_FUNC_ID);

        let zsParams = setConsensusTargetRate(GENOME_ID, 6000);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x970b6109, zsParams, GENOME_ID);
    });

    it("addAllowedProtocolAddress", async () => {
        // 4th: Add Genome address as allowed protocol
        if (TEST_REMOVE_FUNCS) {
            let addr = anchor.web3.Keypair.generate().publicKey;
            let params = addAllowedProtocolAddress(ONE_FUNC_ID, addr);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xd296a0ff,
                params,
                ONE_FUNC_ID
            );
            // removeAllowedProtocolAddress(bytes)
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x0b0a4ca98,
                params,
                ONE_FUNC_ID
            );
        }
        let params = addAllowedProtocolAddress(ONE_FUNC_ID, onefunc.programId);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xd296a0ff, params, ONE_FUNC_ID);
        // Add Genome Protocol
        let zsParams = addAllowedProtocolAddress(GENOME_ID, genome.programId);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xd296a0ff, zsParams, GENOME_ID);
    });

    it("addExecutor", async () => {
        let addr = anchor.web3.Keypair.generate().publicKey;
        let params = addExecutor(ONE_FUNC_ID, addr);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xe0aafb68, params, ONE_FUNC_ID);
        try {
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x04fa384a, // removeExecutor(bytes)
                params,
                ONE_FUNC_ID
            );
        } catch (error) {
            const errMsg = "TryingToRemoveLastGovExecutor";
            assert.equal((error as AnchorError).error.errorMessage, errMsg);
        }

        // Add Genome executor
        let zsParams = addExecutor(GENOME_ID, executor.publicKey);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xe0aafb68, zsParams, GENOME_ID);

        try {
            let params = addExecutor(ONE_FUNC_ID, executor.publicKey);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xe0aafb68,
                params,
                ONE_FUNC_ID
            );
        } catch (error) {
            const errMsg = "ExecutorIsAlreadyAllowed";
            assert.equal((error as AnchorError).error.errorMessage, errMsg);
        }
    });

    it("addProposer", async () => {
        // 5th: Add Genome as a proposer
        if (TEST_REMOVE_FUNCS) {
            let addr = anchor.web3.Keypair.generate().publicKey;
            let params = addExecutor(ONE_FUNC_ID, addr);

            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xce0940a5,
                params,
                ONE_FUNC_ID
            );
            // Add GENOME
            let zsParams = addExecutor(GENOME_ID, addr);
            await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xce0940a5, zsParams, GENOME_ID);
            // removeAllowedProposerAddress(bytes)
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xb8e5f3f4,
                params,
                ONE_FUNC_ID
            );
        }
        let params = addExecutor(ONE_FUNC_ID, proposer);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xce0940a5, params, ONE_FUNC_ID);

        // Add Genome executor
        let zsParams = addExecutor(GENOME_ID, zsProposer);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0xce0940a5, zsParams, GENOME_ID);
    });

    it("approveOperator", async () => {
        // Finish init Genome contract
        // Approve executor as operator and set role = Messenger
        console.log("Executor is", zsExecutor, "owner is", owner);
        zsPhotonOperatorInfo = anchor.web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("OPERATOR"), executor.publicKey.toBuffer()],
            genome.programId
        )[0];

        await genome.methods
            .approveOperator({ messenger: {} }) // TODO: suppress warning?
            .accountsStrict({
                admin: owner.publicKey,
                config: zsConfig,
                operator: zsExecutor.publicKey,
                operatorInfo: zsPhotonOperatorInfo,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .signers([owner])
            .rpc();
    });

    it("addTransmitters", async () => {
        if (TEST_REMOVE_FUNCS) {
            let transmitters2 = randomSigners(3);
            let transmittersRaw2 = [];
            for (var i = 0; i < transmitters2.length; i++) {
                transmittersRaw2.push(hexToBytes(transmitters2[i].address));
            }
            let params = addTransmitter(ONE_FUNC_ID, transmittersRaw2);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x6c5f5666, // addTransmitters
                params,
                ONE_FUNC_ID
            );
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x5206da70, // removeTransmitters
                params,
                ONE_FUNC_ID
            );
        }
        const chunkSize = TRANSMITTERS_PER_CALL;
        for (let i = 0; i < transmittersRaw.length; i += chunkSize) {
            const chunk = transmittersRaw.slice(i, i + chunkSize);
            let params = addTransmitter(ONE_FUNC_ID, chunk);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x6c5f5666,
                params,
                ONE_FUNC_ID
            );
            // Add Genome transmitter
            let zsParams = addTransmitter(GENOME_ID, chunk);
            await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x6c5f5666, zsParams, GENOME_ID);
        }

        let protocolInfo = await program.account.protocolInfo.fetch(onefuncProtocol);
        let actual = protocolInfo.transmitters
            .slice(0, 3)
            .map(x => "0x" + Buffer.from(x).toString("hex"));
        let expected = transmittersRaw.map(x => "0x" + Buffer.from(x).toString("hex"));
        assert.deepEqual(actual, expected);
    });

    it("updateTransmitters", async () => {
        let tempTransmitters = randomSigners(3);
        let tempTransmittersRaw = [];
        for (var i = 0; i < tempTransmitters.length; i++) {
            tempTransmittersRaw.push(hexToBytes(tempTransmitters[i].address));
        }

        let params = updateTransmitter(ONE_FUNC_ID, tempTransmittersRaw, transmittersRaw);

        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0x654b46e1, // updateTransmitters
            params,
            ONE_FUNC_ID
        );

        let protocolInfo = await program.account.protocolInfo.fetch(onefuncProtocol);
        let actual = protocolInfo.transmitters
            .slice(0, 3)
            .map(x => "0x" + Buffer.from(x).toString("hex"));
        let expected = tempTransmittersRaw.map(x => "0x" + Buffer.from(x).toString("hex"));

        assert.deepEqual(actual, expected);

        params = updateTransmitter(ONE_FUNC_ID, transmittersRaw, tempTransmittersRaw);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0x654b46e1, // updateTransmitters
            params,
            ONE_FUNC_ID
        );
        protocolInfo = await program.account.protocolInfo.fetch(onefuncProtocol);
        actual = protocolInfo.transmitters
            .slice(0, 3)
            .map(x => "0x" + Buffer.from(x).toString("hex"));
        expected = transmittersRaw.map(x => "0x" + Buffer.from(x).toString("hex"));
        assert.deepEqual(actual, expected);
    });

    it("executeOperation by name", async () => {
        let params = hexToBytes(ethers.utils.defaultAbiCoder.encode(["uint256"], [3]));
        let keys = [{ isSigner: false, isWritable: true, pubkey: counter }];
        await executeOperation(
            ONE_FUNC_ID,
            onefunc.programId,
            "increment",
            params,
            null,
            [{ pubkey: onefunc.programId, isSigner: false, isWritable: false }].concat(keys)
        );
        const state = await onefunc.account.counter.fetch(counter);
        expect(state.count.toNumber()).eq(3);
    });

    it("executeOperation by code", async () => {
        let keys = [{ isSigner: false, isWritable: true, pubkey: counter }];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            ONE_FUNC_ID,
            onefunc.programId,
            0x01020304,
            new Buffer([]),
            null,
            [{ pubkey: onefunc.programId, isSigner: false, isWritable: false }].concat(keys)
        );
        const state = await onefunc.account.counter.fetch(counter);
        expect(state.count.toNumber()).eq(3);
    });

    it("execute Genome start_game by code", async () => {
        const zsConfig = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CONFIG")],
            genome.programId
        )[0];
        // The function signature exactly as in the contract:
        const functionSignature = "startGameOmnichain(uint256,uint256,bytes32[],bool)";
        const gameCounterBytes = new BN(0).toArrayLike(Buffer, "le", 8);
        const game = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("GAME"), gameCounterBytes],
            genome.programId
        )[0];
        const gameVault = await getOrCreateAssociatedTokenAccount(
            program.provider.connection,
            owner,
            nglMint,
            game,
            true
        );

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["uint256", "uint256", "bytes32[]", "bool"];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");
        zsGameWinner = bs58.decode("HprSAkYDxheSb6CCNhBVJXPAUkGJ1xmGrTnbABvuZZLh");
        const values = [0, 2000, ["0x" + genRanHex(64), "0x" + genRanHex(64)], false];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                /*
                 *       [
                 *          bytes32 to - address on which tokens will be minted
                 *          bytes32 token - address of the token
                 *          bytes32 srcToken - address of token for rollback
                 *          uint256 amount - amount of tokens to mint
                 *          bytes32 provider - address of the Genome mesenger from the source chain for rollback
                 *          uint256 chainId - identificator of the source chain for rollback
                 *          bytes32 target - address of the contract on which a function must be called
                 *          bytes memory data - encoded data to provide in .call()
                 *       ]
                 */
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    4000,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: game },
            { isSigner: false, isWritable: true, pubkey: gameVault.address },
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("execute Genome register_game_participants by code", async () => {
        const zsConfig = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CONFIG")],
            genome.programId
        )[0];
        // The function signature exactly as in the contract:
        const functionSignature = "registerGameParticipantsOmnichain(uint256,bytes32[],bool)";
        const gameCounterBytes = new BN(0).toArrayLike(Buffer, "le", 8);
        const game = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("GAME"), gameCounterBytes],
            genome.programId
        )[0];
        const gameVault = await getOrCreateAssociatedTokenAccount(
            program.provider.connection,
            owner,
            nglMint,
            game,
            true
        );

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["uint256", "bytes32[]", "bool"];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");
        zsGameWinner = bs58.decode("HprSAkYDxheSb6CCNhBVJXPAUkGJ1xmGrTnbABvuZZLh");
        const values = [0, ["0x" + zsGameWinner.toString("hex")], true];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    4000,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: game },
            { isSigner: false, isWritable: true, pubkey: gameVault.address },
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("execute Genome finish_game by code", async () => {
        const zsConfig = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CONFIG")],
            genome.programId
        )[0];
        // The function signature exactly as in the contract:
        const functionSignature = "finishGame(uint256,uint16,bytes32)";
        const gameCounterBytes = new BN(0).toArrayLike(Buffer, "le", 8);
        const game = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("GAME"), gameCounterBytes],
            genome.programId
        )[0];
        const gameVault = await getOrCreateAssociatedTokenAccount(
            program.provider.connection,
            owner,
            nglMint,
            game,
            true
        );

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["uint256", "uint16", "bytes32"];
        // Corresponding values
        const values = [0, 0, "0x" + zsGameWinner.toString("hex")];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    0,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        owner,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("FEE_META"), new BN(0).toArrayLike(Buffer, "le", 2)],
                    genome.programId
                )[0],
            },
            { isSigner: false, isWritable: true, pubkey: game },
            { isSigner: false, isWritable: true, pubkey: gameVault.address },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [
                        zsRoot,
                        utf8.encode("USER"),
                        new PublicKey("HprSAkYDxheSb6CCNhBVJXPAUkGJ1xmGrTnbABvuZZLh").toBuffer(),
                    ],
                    genome.programId
                )[0],
            },
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    async function createTournamentOmnichain(tournamentIndex: number) {
        const tournamentId = new BN(tournamentIndex).toArrayLike(Buffer, "le", 8);
        const tournament = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TOURNAMENT"), tournamentId],
            genome.programId
        )[0];
        const claimableUserInfo = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("USER"), executor.publicKey.toBuffer()],
            genome.programId
        )[0];
        const ix = genome.methods
            .createTournamentOmnichain(executor.publicKey, {
                fee: new anchor.BN(0),
                sponsorPool: new anchor.BN(0),
                startTime: new anchor.BN(Math.floor(Date.now() / 1000)),
                playersInTeam: 4,
                minTeams: 2,
                maxTeams: 10,
                organizerRoyalty: 5,
                token: nglMint,
            })
            .accountsStrict({
                sponsor: executor.publicKey,
                config: zsConfig,
                tournament,
                operatorInfo: zsPhotonOperatorInfo,
                claimableUserInfo,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .signers([executor]);

        const instruction = await ix.instruction();
        const transaction = new anchor.web3.Transaction().add(instruction);

        const { blockhash } = await program.provider.connection.getLatestBlockhash();
        transaction.feePayer = executor.publicKey;
        transaction.recentBlockhash = blockhash;

        transaction.sign(executor);

        await ix.rpc();
    }

    it("execute Genome create tournament by code", async () => {
        // TODO: Once transaction limit is fixed on Photon's side, remove this if
        if (true) {
            await createTournamentOmnichain(0);
            return;
        }

        const zsConfig = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CONFIG")],
            genome.programId
        )[0];
        // The function signature exactly as in the contract:
        const functionSignature =
            "createTournament(bytes32,bytes32,(uint64,uint64,uint64,uint8,uint8,uint8,uint16,bytes32,uint8),bytes32[],uint8)";
        const tournamentId = new BN(0).toArrayLike(Buffer, "le", 8);
        const tournament = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TOURNAMENT"), tournamentId],
            genome.programId
        )[0];

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = [
            "bytes32",
            "bytes32",
            "(uint64,uint64,uint64,uint8,uint8,uint8,uint16,bytes32,uint8)",
            "bytes32[]",
            "uint8",
        ];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");

        const values = [
            "0x" + genRanHex(64),
            "0x" + genRanHex(64),
            [0, 4, 0, 3, 2, 4, 5, "0x" + genRanHex(64), 0],
            [],
            0,
        ];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    500,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        const claimableUserInfo = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("USER"), executor.publicKey.toBuffer()],
            genome.programId
        )[0];
        console.log("Genome program ID", genome.programId.toBase58());
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: claimableUserInfo },
            { isSigner: false, isWritable: true, pubkey: tournament },
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("execute Genome register first team in tournament by code", async () => {
        // The function signature exactly as in the contract:
        const functionSignature = "register(uint256,bytes32,bytes32[],uint8)";
        const tournamendIdBytes = new BN(0).toArrayLike(Buffer, "le", 8);

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["uint256", "bytes32", "bytes32[]", "uint8"];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");
        const captain = executor;
        const teammate = owner;
        const values = [0, "0x" + captain.publicKey.toBuffer().toString("hex"), ["0x" + teammate.publicKey.toBuffer().toString("hex")], 0];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    500,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const tournament = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TOURNAMENT"), tournamendIdBytes],
            genome.programId
        )[0];
        const team = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TEAM"), tournamendIdBytes, executor.publicKey.toBuffer()],
            genome.programId
        )[0];
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: tournament },
            { isSigner: false, isWritable: true, pubkey: executor.publicKey },
            { isSigner: false, isWritable: true, pubkey: team },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("USER"), executor.publicKey.toBuffer()],
                    genome.programId
                )[0],
            },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("TEAM_PARTICIPANT"), tournamendIdBytes, teammate.publicKey.toBuffer()],
                    genome.programId
                )[0]
            },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("TEAM_PARTICIPANT"), tournamendIdBytes, captain.publicKey.toBuffer()],
                    genome.programId
                )[0]
            }
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("execute Genome register second team in tournament by code", async () => {
        // The function signature exactly as in the contract:
        const functionSignature = "register(uint256,bytes32,bytes32[],uint8)";
        const tournamendIdBytes = new BN(0).toArrayLike(Buffer, "le", 8);

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["uint256", "bytes32", "bytes32[]", "uint8"];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");
        const captain = web3.Keypair.generate().publicKey;
        const teammate = web3.Keypair.generate().publicKey;
        const values = [0, "0x" + captain.toBuffer().toString("hex"), ["0x" + teammate.toBuffer().toString("hex")], 0];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        console.log("Pubkey bytes are", bytes);
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    500,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const tournament = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TOURNAMENT"), tournamendIdBytes],
            genome.programId
        )[0];
        const team = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TEAM"), tournamendIdBytes, captain.toBuffer()],
            genome.programId
        )[0];
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: true, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: tournament },
            { isSigner: false, isWritable: true, pubkey: captain },
            { isSigner: false, isWritable: true, pubkey: team },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("USER"), executor.publicKey.toBuffer()],
                    genome.programId
                )[0],
            },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("TEAM_PARTICIPANT"), tournamendIdBytes, teammate.toBuffer()],
                    genome.programId
                )[0]
            },
            {
                isSigner: false,
                isWritable: true,
                pubkey: web3.PublicKey.findProgramAddressSync(
                    [zsRoot, utf8.encode("TEAM_PARTICIPANT"), tournamendIdBytes, captain.toBuffer()],
                    genome.programId
                )[0]
            }
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("execute Genome make bet in tournament by code", async () => {
        // The function signature exactly as in the contract:
        const functionSignature = "makeBetOmnichain(bytes32,uint256[])";
        const tournamendIdBytes = new BN(0).toArrayLike(Buffer, "le", 8);

        // Compute the function selector (4-byte hash)
        // First 4 bytes in hex form
        const functionSelector = ethers.utils
            .keccak256(ethers.utils.toUtf8Bytes(functionSignature))
            .substring(0, 10);
        const types = ["bytes32", "uint256[]"];
        // Corresponding values
        const genRanHex = size =>
            [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join("");

        const values = [
            "0x" + executor.publicKey.toBuffer().toString("hex"),
            [0, "0x" + executor.publicKey.toBuffer().toString("hex"), 0, 100],
        ];
        // Encode just the parameters
        const encodedParams = ethers.utils.defaultAbiCoder.encode(types, values);
        const functionCallPayload = functionSelector + encodedParams.slice(2);
        console.log("Function call payload is", functionCallPayload);
        const bytes = Buffer.from(nglVault.toBytes()).toString("hex");
        const params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(
                ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "uint256", "bytes32", "bytes"],
                [
                    "0x" + bytes,
                    "0x" + nglMint.toBuffer().toString("hex"),
                    100,
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    ethers.utils.formatBytes32String(""),
                    1,
                    ethers.utils.formatBytes32String(""),
                    functionCallPayload,
                ]
            )
        );
        console.log("Genome program ID", genome.programId.toBase58());
        const tournament = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TOURNAMENT"), tournamendIdBytes],
            genome.programId
        )[0];
        const tournamentBook = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("BOOK"), tournamendIdBytes],
            genome.programId
        )[0];
        const bookVault = (
            await getOrCreateAssociatedTokenAccount(
                program.provider.connection,
                executor,
                nglMint,
                tournamentBook,
                true
            )
        ).address;
        const captainBet = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("CAPTAIN_BET"), tournamendIdBytes, executor.publicKey.toBuffer()],
            genome.programId
        )[0];
        const gamblerInfo = web3.PublicKey.findProgramAddressSync(
            [
                zsRoot,
                utf8.encode("GAMBLER"),
                tournamendIdBytes,
                executor.publicKey.toBuffer(),
                executor.publicKey.toBuffer(),
            ],
            genome.programId
        )[0];
        const team = web3.PublicKey.findProgramAddressSync(
            [zsRoot, utf8.encode("TEAM"), tournamendIdBytes, executor.publicKey.toBuffer()],
            genome.programId
        )[0];
        const keys = [
            { isSigner: false, isWritable: false, pubkey: zsPhotonOperatorInfo },
            { isSigner: false, isWritable: true, pubkey: nglVault },
            { isSigner: false, isWritable: true, pubkey: zsConfig },
            { isSigner: false, isWritable: false, pubkey: nglAuthority },
            { isSigner: false, isWritable: true, pubkey: nglMint },
            { isSigner: false, isWritable: false, pubkey: nglTokenConfig },
            { isSigner: false, isWritable: false, pubkey: nglCore.programId },
            { isSigner: false, isWritable: false, pubkey: zsTreasuryAuthority },
            {
                isSigner: false,
                isWritable: true,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        zsTreasuryAuthority,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: executor.publicKey },
            {
                isSigner: false,
                isWritable: false,
                pubkey: (
                    await getOrCreateAssociatedTokenAccount(
                        program.provider.connection,
                        executor,
                        nglMint,
                        executor.publicKey,
                        true
                    )
                ).address,
            },
            { isSigner: false, isWritable: false, pubkey: token.TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: token.ASSOCIATED_TOKEN_PROGRAM_ID },
            { isSigner: false, isWritable: false, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: web3.SystemProgram.programId },
            { isSigner: false, isWritable: true, pubkey: program.programId },
            { isSigner: false, isWritable: true, pubkey: tournament },
            { isSigner: false, isWritable: true, pubkey: tournamentBook },
            { isSigner: false, isWritable: true, pubkey: bookVault },
            { isSigner: false, isWritable: true, pubkey: captainBet },
            { isSigner: false, isWritable: true, pubkey: gamblerInfo },
        ];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            GENOME_ID,
            genome.programId,
            0x67b8fb72, // receiveAndCall(bytes) bytes -> startGame(...)
            params,
            null,
            [{ pubkey: genome.programId, isSigner: false, isWritable: false }].concat(keys)
        );
    });

    it("propose", async () => {
        let signature = await onefunc.methods
            .proposeToOtherChain()
            .accounts({
                owner: owner.publicKey,
                proposer,
                photonProgram: program.programId,
                config,
                protocolInfo: onefuncProtocol,
            })
            .signers([owner])
            .rpc();
        let [tx, counter] = [null, 0];
        while (tx == null) {
            await sleep(10);
            tx = await anchor.getProvider().connection.getParsedTransaction(signature, {
                commitment: "confirmed",
            });
            expect((counter += 1)).lte(30, "Propose transaction has not been found in time");
        }
        const eventParser = new EventParser(program.programId, new BorshCoder(program.idl));
        const events = Array.from(eventParser.parseLogs(tx.meta.logMessages));
        expect(events.length).eq(1, "Expected exact one ProposeEvent");
        let event = events[0];
        expect(event.name).eq("ProposeEvent");
        expect((event.data.protocolId as Buffer).compare(ONE_FUNC_ID)).eq(
            0,
            "Unexpected protocolId"
        );
        expect(EOB_CHAIN_ID).eq(
            (event.data.dstChainId as anchor.BN).toNumber(),
            "Unexpected dst_chain_id"
        );
        expect((event.data.protocolAddress as Buffer).compare(Buffer.alloc(20, 1))).eq(
            0,
            "Unexpected protocolAddress"
        );
        expect(
            (event.data.functionSelector as Buffer).compare(
                Buffer.concat([
                    Buffer.from([1, 32]),
                    Buffer.from(utf8.encode("ask1234mkl;1mklasdfasm;lkasdmf__")),
                ])
            )
        ).eq(0, "Unexpected params");
        expect((event.data.params as Buffer).compare(utf8.encode("an arbitrary data"))).eq(
            0,
            "Unexpected data"
        );
        expect((event.data.nonce as anchor.BN).toNumber()).eq(2, "Unexpected nonce");
    });

    it("propose with selector too big", async () => {
        try {
            await onefunc.methods
                .proposeToOtherChainBigSelector()
                .accounts({
                    owner: owner.publicKey,
                    proposer,
                    photonProgram: program.programId,
                    config,
                    protocolInfo: onefuncProtocol,
                })
                .signers([owner])
                .rpc();
            assert.ok(false, "Selector too big should fail");
        } catch (_err) {
            assert.isTrue(_err instanceof AnchorError);
            const err: AnchorError = _err;
            const errMsg = "SelectorTooBig";
            assert.strictEqual(err.error.errorMessage, errMsg);
        }
    });
});
