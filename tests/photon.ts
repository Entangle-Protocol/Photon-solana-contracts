import * as anchor from "@coral-xyz/anchor";
import {BorshCoder, EventParser, Program, web3} from "@coral-xyz/anchor";
import {Photon} from "../target/types/photon";
import {Onefunc} from "../target/types/onefunc";
import {utf8} from "@coral-xyz/anchor/dist/cjs/utils/bytes";

import {
    addAllowedProtocolAddress,
    addExecutor,
    addAllowedProtocol,
    hexToBytes,
    opHashFull,
    randomSigners,
    predefinedSigners,
    signOp,
    addKeepers,
    setConsensusTargetRate,
    sleep,
} from "./utils";
import {Wallet, ethers} from "ethers";
import {assert, expect} from "chai";

const TEST_REMOVE_FUNCS = true;
const ROOT = utf8.encode("root-0");
const EOB_CHAIN_ID = 33133;
const SOLANA_CHAIN_ID = "100000000000000000000";
const CONSENSUS_TARGET_RATE = 10000;
const KEEPERS = 3;
const KEEPERS_PER_CALL = 4;
const GOV_PROTOCOL_ID = Buffer.from(
    utf8.encode("aggregation-gov_________________"),
);
const ONE_FUNC_ID = Buffer.from(
    utf8.encode("onefunc_________________________"),
);

type FunctionSelector = anchor.IdlTypes<Photon>["FunctionSelector"];

describe("photon", () => {
    // Configure the client to use the local cluster.
    anchor.setProvider(anchor.AnchorProvider.env());
    const program = anchor.workspace.Photon as Program<Photon>;
    const onefunc = anchor.workspace.Onefunc as Program<Onefunc>;

    let owner_keypair = Uint8Array.from(require("../keys/owner.json"));
    let owner = anchor.web3.Keypair.fromSecretKey(owner_keypair);
    let executor_keypair = Uint8Array.from(require("../keys/gov-executor.json"));
    const executor = anchor.web3.Keypair.fromSecretKey(executor_keypair);

    let config;
    let govProtocolInfo;
    let counter;
    let proposer;
    let callAuthority;
    let callAuthorityBump;
    let keepers: Wallet[];
    let keepersRaw = [];
    let nonce = 0;
    let onefuncProtocol;

    before(async () => {
        let tx = await program.provider.connection.requestAirdrop(
            owner.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await program.provider.connection.confirmTransaction(tx);
        tx = await program.provider.connection.requestAirdrop(
            executor.publicKey,
            anchor.web3.LAMPORTS_PER_SOL,
        );
        await program.provider.connection.confirmTransaction(tx);

        config = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CONFIG")],
            program.programId,
        )[0];
        govProtocolInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
            program.programId,
        )[0];
        console.log("Executor", executor.publicKey.toBase58());
        keepers = predefinedSigners(KEEPERS);
        for (var i = 0; i < keepers.length; i++) {
            console.log("Keeper", i, keepers[i].address);
            keepersRaw.push(hexToBytes(keepers[i].address));
        }
        [callAuthority, callAuthorityBump] = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("CALL_AUTHORITY"), ONE_FUNC_ID],
            program.programId,
        );
        counter = web3.PublicKey.findProgramAddressSync(
            [utf8.encode("COUNTER")],
            onefunc.programId,
        )[0];
        onefuncProtocol = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), ONE_FUNC_ID],
            program.programId,
        )[0];

        proposer = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROPOSER")],
            onefunc.programId,
        )[0];
    });

    async function executeOperation(
        protocolId: Buffer,
        protocolAddr: anchor.web3.PublicKey,
        functionSelector: number | string | Buffer,
        params: Buffer,
        targetProtocol: Buffer,
        remainingAccounts?: anchor.web3.AccountMeta[],
    ) {
        let fs: FunctionSelector;
        if (typeof functionSelector == "number") {
            let functionSelectorBuf = Buffer.alloc(4);
            functionSelectorBuf.writeUInt32BE(functionSelector);
            fs = {byCode: [functionSelectorBuf]}
        } else if (typeof functionSelector == "string") {
            fs = {byName: [functionSelector]}
        } else {
            assert(false, "Unexpected functionSelector")
        }
        let meta = new anchor.BN("0100000000000000000000000000000000000000000000000000000000000000", 16).toBuffer();
        let op = {
            protocolId,
            meta,
            srcChainId: new anchor.BN(EOB_CHAIN_ID),
            srcBlockNumber: new anchor.BN(1),
            srcOpTxId: hexToBytes(
                "ce25f58a7fd8625deadc00a59b67c530c7d92acec1e5753c588269ade6ebf99f",
            ),
            nonce: new anchor.BN(nonce),
            destChainId: new anchor.BN(SOLANA_CHAIN_ID),
            protocolAddr,
            functionSelector: fs,
            params,
            reserved: Buffer.from([])
        };
        let op_hash = opHashFull(op);
        let opInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("OP"), op_hash],
            program.programId,
        )[0];
        let protocolInfo = web3.PublicKey.findProgramAddressSync(
            [ROOT, utf8.encode("PROTOCOL"), op.protocolId],
            program.programId,
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
        const chunkSize = KEEPERS_PER_CALL;
        // console.debug("load_operation:", signature);
        let signatures = [];
        for (let i = 0; i < keepers.length; i++) {
            const sig = await signOp(keepers[i], op);
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
            console.debug("sign_operation:", signature)
        }
        // Execute
        if (protocolId == GOV_PROTOCOL_ID) {
            let signature = await program.methods
                .executeGovOperation(op_hash, targetProtocol)
                .accounts({
                    executor: executor.publicKey,
                    opInfo,
                    govInfo: govProtocolInfo,
                    protocolInfo: web3.PublicKey.findProgramAddressSync(
                        [ROOT, utf8.encode("PROTOCOL"), targetProtocol],
                        program.programId,
                    )[0],
                    systemProgram: web3.SystemProgram.programId,
                })
                .signers([executor])
                .rpc();
            console.debug("execute_gov_operation:", signature)
        } else {
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
            console.debug("execute_operation:", signature)
        }
        console.log("Proposal", nonce, "executed");
        nonce++;
    }

    it("Initialize", async () => {
        await onefunc.methods
            .initialize()
            .accounts({owner: owner.publicKey, callAuthority, counter})
            .signers([owner])
            .rpc();
        await program.methods
            .initialize(
                new anchor.BN(EOB_CHAIN_ID),
                new anchor.BN(CONSENSUS_TARGET_RATE),
                [keepersRaw[0]],
                [executor.publicKey],
            )
            .accounts({
                admin: owner.publicKey,
                protocolInfo: govProtocolInfo,
                config,
                systemProgram: web3.SystemProgram.programId,
            })
            .signers([owner])
            .rpc();
        const chunkSize = KEEPERS_PER_CALL;
        for (let i = 1; i < keepersRaw.length; i += chunkSize) {
            const chunk = keepersRaw.slice(i, i + chunkSize);
            const params = addKeepers(GOV_PROTOCOL_ID, chunk);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xa8da4c51,
                params,
                GOV_PROTOCOL_ID,
            );
        }
    });

    it("addAllowedProtocol", async () => {
        let params = addAllowedProtocol(ONE_FUNC_ID, [], CONSENSUS_TARGET_RATE);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0x45a004b9,
            params,
            ONE_FUNC_ID,
        );
    });

    it("setConsensusTargetRate", async () => {
        let params = setConsensusTargetRate(ONE_FUNC_ID, 6000);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0x970b6109,
            params,
            ONE_FUNC_ID,
        );
    });

    it("addAllowedProtocolAddress", async () => {
        if (TEST_REMOVE_FUNCS) {
            let addr = anchor.web3.Keypair.generate().publicKey;
            let params = addAllowedProtocolAddress(ONE_FUNC_ID, addr);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xd296a0ff,
                params,
                ONE_FUNC_ID,
            );
            // removeAllowedProtocolAddress(bytes)
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x0b0a4ca98,
                params,
                ONE_FUNC_ID,
            );
        }
        let params = addAllowedProtocolAddress(ONE_FUNC_ID, onefunc.programId);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0xd296a0ff,
            params,
            ONE_FUNC_ID,
        );
    });

    it("addExecutor", async () => {
        if (TEST_REMOVE_FUNCS) {
            let addr = anchor.web3.Keypair.generate().publicKey;
            let params = addExecutor(ONE_FUNC_ID, addr);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xe0aafb68,
                params,
                ONE_FUNC_ID,
            );
            // removeExecutor(bytes)
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x04fa384a,
                params,
                ONE_FUNC_ID,
            );
        }
        let params = addExecutor(ONE_FUNC_ID, executor.publicKey);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0xe0aafb68,
            params,
            ONE_FUNC_ID,
        );
    });

    it("addProposer", async () => {
        if (TEST_REMOVE_FUNCS) {
            let addr = anchor.web3.Keypair.generate().publicKey;
            let params = addExecutor(ONE_FUNC_ID, addr);

            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xce0940a5,
                params,
                ONE_FUNC_ID,
            );
            // removeAllowedProposerAddress(bytes)
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xb8e5f3f4,
                params,
                ONE_FUNC_ID,
            );
        }
        let params = addExecutor(ONE_FUNC_ID, proposer);
        await executeOperation(
            GOV_PROTOCOL_ID,
            program.programId,
            0xce0940a5,
            params,
            ONE_FUNC_ID,
        );
    });

    it("addKeepers", async () => {
        if (TEST_REMOVE_FUNCS) {
            let keepers2 = randomSigners(3);
            let keepersRaw2 = [];
            for (var i = 0; i < keepers2.length; i++) {
                keepersRaw2.push(hexToBytes(keepers2[i].address));
            }
            let params = addKeepers(ONE_FUNC_ID, keepersRaw2);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xa8da4c51,
                params,
                ONE_FUNC_ID,
            );
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0x80936851,
                params,
                ONE_FUNC_ID,
            );
        }
        const chunkSize = KEEPERS_PER_CALL;
        for (let i = 0; i < keepersRaw.length; i += chunkSize) {
            const chunk = keepersRaw.slice(i, i + chunkSize);
            let params = addKeepers(ONE_FUNC_ID, chunk);
            await executeOperation(
                GOV_PROTOCOL_ID,
                program.programId,
                0xa8da4c51,
                params,
                ONE_FUNC_ID,
            );
        }
    });

    it("executeOperation by name", async () => {
        let params = hexToBytes(
            ethers.utils.defaultAbiCoder.encode(["uint256"], [3]),
        );
        let keys = [{isSigner: false, isWritable: true, pubkey: counter}];
        await executeOperation(
            ONE_FUNC_ID,
            onefunc.programId,
            "increment",
            params,
            null,
            [
                {pubkey: onefunc.programId, isSigner: false, isWritable: false},
            ].concat(keys),
        );
        const state = await onefunc.account.counter.fetch(counter);
        expect(state.count.toNumber()).eq(3);
    });


    it("executeOperation by code", async () => {
        let keys = [{isSigner: false, isWritable: true, pubkey: counter}];
        // This operation results in the `receive_photon_msg` method invocation with a PhotonMsgWithSelector
        await executeOperation(
            ONE_FUNC_ID,
            onefunc.programId,
            0x01020304,
            new Buffer([]),
            null,
            [
                {pubkey: onefunc.programId, isSigner: false, isWritable: false},
            ].concat(keys),
        );
        const state = await onefunc.account.counter.fetch(counter);
        expect(state.count.toNumber()).eq(3);
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
            tx = await anchor
                .getProvider()
                .connection.getParsedTransaction(signature, {
                    commitment: "confirmed",
                });
            expect(counter += 1).lte(30, "Propose transaction has not been found in time");
        }
        const eventParser = new EventParser(
            program.programId,
            new BorshCoder(program.idl),
        );
        const events = Array.from(eventParser.parseLogs(tx.meta.logMessages));
        expect(events.length).eq(1, "Expected exact one ProposeEvent")
        let event = events[0];
        expect(event.name).eq("ProposeEvent");
        expect((event.data.protocolId as Buffer).compare(ONE_FUNC_ID)).eq(0, "Unexpected protocolId");
        expect(EOB_CHAIN_ID).eq((event.data.dstChainId as anchor.BN).toNumber(), "Unexpected dst_chain_id");
        expect((event.data.protocolAddress as Buffer).compare(Buffer.alloc(20, 1))).eq(0, "Unexpected protocolAddress");
        expect((event.data.functionSelector as Buffer).compare(utf8.encode("ask1234mkl;1mklasdfasm;lkasdmf__"))).eq(0, "Unexpected params");
        expect((event.data.params as Buffer).compare(utf8.encode("an arbitrary data"))).eq(0, "Unexpected data");
        expect((event.data.nonce as anchor.BN).toNumber()).eq(0, "Unexpected nonce");
    });
});
