import * as anchor from "@coral-xyz/anchor";
import { AnchorError, BorshCoder, EventParser, Program, web3 } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";
import { Onefunc } from "../target/types/onefunc";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

import {
    addAllowedProtocolAddress,
    addExecutor,
    addAllowedProtocol,
    hexToBytes,
    opHashFull,
    randomSigners,
    predefinedSigners,
    signOp,
    addTransmitter,
    setConsensusTargetRate,
    sleep,
    updateTransmitter,
} from "./utils";
import { Wallet, ethers, BigNumber } from "ethers";
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
const ONE_FUNC_ID = Buffer.from(utf8.encode("onefunc_________________________"));

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
    let govCallAuthority;
    let transmitters: Wallet[];
    let transmittersRaw = [];
    let nonce = 0;
    let onefuncProtocol;

    before(async () => {
        let tx = await program.provider.connection.requestAirdrop(
            owner.publicKey,
            anchor.web3.LAMPORTS_PER_SOL
        );
        await program.provider.connection.confirmTransaction(tx);
        tx = await program.provider.connection.requestAirdrop(
            executor.publicKey,
            anchor.web3.LAMPORTS_PER_SOL
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
            console.debug("execute_operation:", signature);
        }
        console.log("Proposal", nonce, "executed");
        nonce++;
    }


    it("Initialize", async () => {
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
        let params = addAllowedProtocol(ONE_FUNC_ID, [], CONSENSUS_TARGET_RATE);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x45a004b9, params, ONE_FUNC_ID);
    });

    it("setConsensusTargetRate", async () => {
        let params = setConsensusTargetRate(ONE_FUNC_ID, 6000);
        await executeOperation(GOV_PROTOCOL_ID, program.programId, 0x970b6109, params, ONE_FUNC_ID);
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
        }

        let protocolInfo = await program.account.protocolInfo.fetch(onefuncProtocol);
        let actual = protocolInfo.transmitters.slice(0, 3).map(x => "0x" + Buffer.from(x).toString("hex"));
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
        let actual = protocolInfo.transmitters.slice(0, 3).map(x => "0x" + Buffer.from(x).toString("hex"));
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
        actual = protocolInfo.transmitters.slice(0, 3).map(x => "0x" + Buffer.from(x).toString("hex"));
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
        expect((event.data.nonce as anchor.BN).toNumber()).eq(1, "Unexpected nonce");
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
