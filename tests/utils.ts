import { OperationLib } from "@entangle_protocol/oracle-sdk/dist/typechain-types/contracts/EndPoint";
import { assert, expect } from "chai";
import { Wallet, ethers } from "ethers";
import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "fs";
import { IdlTypes } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";

type FunctionSelector = IdlTypes<Photon>["FunctionSelector"];

interface AnchorOpData {
    protocolId: Buffer;
    meta: anchor.BN;
    srcChainId: anchor.BN;
    srcBlockNumber: anchor.BN;
    srcOpTxId: Buffer;
    nonce: anchor.BN;
    destChainId: anchor.BN;
    protocolAddr: anchor.web3.PublicKey;
    functionSelector: FunctionSelector;
    params: Buffer;
    reserved: Buffer;
}

function opHash(opData: OperationLib.OperationDataStruct) {
    return ethers.utils.solidityKeccak256(
        [
            "bytes32",
            "uint256",
            "uint256",
            "uint256",
            "bytes32",
            "uint256",
            "uint256",
            "bytes",
            "bytes",
            "bytes",
            "bytes",
        ],
        [
            opData.protocolId,
            opData.meta,
            opData.srcChainId,
            opData.srcBlockNumber,
            opData.srcOpTxId,
            opData.nonce,
            opData.destChainId,
            opData.protocolAddr,
            opData.functionSelector,
            opData.params,
            opData.reserved,
        ],
    );
}

function convertOpData(opData: AnchorOpData): OperationLib.OperationDataStruct {
    let selector_data: Buffer;
    if (opData.functionSelector.byName) {
        let len = opData.functionSelector.byName[0].length;
        selector_data = Buffer.alloc(2 + opData.functionSelector.byName[0].length);
        selector_data.writeUInt8(1);
        selector_data.writeUInt8(len, 1);
        selector_data.write(opData.functionSelector.byName[0].toString(), 2);
    } else if (opData.functionSelector.byCode) {
        let len = opData.functionSelector.byCode[0].length;
        selector_data = Buffer.alloc(2 + opData.functionSelector.byCode[0].length);
        selector_data.writeUInt8(0);
        selector_data.writeUInt8(len, 1);
        opData.functionSelector.byCode[0].copy(selector_data, 2);
    } else {
        assert(false, "Unexpected function_selector value");
    }

    return {
        protocolId: Buffer.from(opData.protocolId),
        meta: opData.meta,
        srcChainId: opData.srcChainId.toString(),
        srcBlockNumber: opData.srcBlockNumber.toNumber(),
        srcOpTxId: Buffer.from(opData.srcOpTxId),
        nonce: opData.nonce.toNumber(),
        destChainId: opData.destChainId.toString(),
        protocolAddr: opData.protocolAddr.toBytes(),
        functionSelector: selector_data,
        params: opData.params,
        reserved: opData.reserved,
    };
}

function _opHashFull(opData: OperationLib.OperationDataStruct) {
    return ethers.utils.solidityKeccak256(
        ["string", "bytes32"],
        ["\x19Ethereum Signed Message:\n32", ethers.utils.arrayify(opHash(opData))],
    );
}

export function opHashFull(opData: AnchorOpData): Buffer {
    return Buffer.from(hexToBytes(_opHashFull(convertOpData(opData))));
}

export async function signOp(transmitter: Wallet, op: AnchorOpData) {
    const msgHash = ethers.utils.arrayify(opHash(convertOpData(op)));
    const sign = ethers.utils.splitSignature(await transmitter.signMessage(msgHash));
    const v = sign.v;
    const r = hexToBytes(sign.r);
    const s = hexToBytes(sign.s);
    expect(transmitter.address).eq(ethers.utils.verifyMessage(msgHash, sign));
    return { v, r, s };
}

export function randomSigners(amount: number): Wallet[] {
    const signers = [];
    for (let i = 0; i < amount; i++) {
        signers.push(ethers.Wallet.createRandom());
    }
    return signers;
}

export function predefinedSigners(amount: number): Wallet[] {
    const signers = [];
    assert(amount <= 3, "Unexpected number of signers");
    for (let i = 1; i <= amount; i++) {
        let signer = new Wallet(
            readFileSync("tests/accounts/transmitter_" + i, "utf-8"),
        );
        signers.push(signer);
    }
    return signers;
}

export function hexToBytes(hex: string): Buffer {
    return Buffer.from(hex.replace("0x", ""), "hex");
}

export function sleep(ms) {
    return new Promise((resolve) => {
        setTimeout(resolve, ms);
    });
}

export function addAllowedProtocol(
    protocolId: Buffer,
    transmittersRaw: number[][],
    consensusTargetRate: number,
): Buffer {
    return hexToBytes(
        ethers.utils.defaultAbiCoder.encode(
            ["tuple(bytes32, uint, address[])"],
            [[
                protocolId,
                consensusTargetRate,
                transmittersRaw.map((x) => Buffer.from(x).toString("hex")),
            ]],
        ),
    );
}

export function addAllowedProtocolAddress(
    protocolId: Buffer,
    protocolAddr: anchor.web3.PublicKey,
): Buffer {
    return hexToBytes(
        ethers.utils.defaultAbiCoder.encode(
            ["tuple(bytes32, bytes)"],
            [[protocolId, protocolAddr.toBuffer()]],
        ),
    );
}

export function addExecutor(
    protocolId: Buffer,
    executor: anchor.web3.PublicKey,
): Buffer {
    return hexToBytes(
        ethers.utils.defaultAbiCoder.encode(
            ["tuple(bytes32, bytes)"],
            [[protocolId, executor.toBuffer()]],
        ),
    );
}

export function addTransmitter(protocolId: Buffer, transmitterRaw: number[][]): Buffer {
    let hex = ethers.utils.defaultAbiCoder.encode(
        ["tuple(bytes32, address[])"],
        [[protocolId, transmitterRaw.map((x) => Buffer.from(x).toString("hex"))]],
    );
    return hexToBytes(
        hex
    );
}

export function updateTransmitter(protocolId: Buffer, to_add: number[][], to_remove: number[][]): Buffer {
    let hex = ethers.utils.defaultAbiCoder.encode(
        ["tuple(bytes32, address[], address[])"],
        [
            [protocolId,
                to_add.map((x) => Buffer.from(x).toString("hex")),
                to_remove.map((x) => Buffer.from(x).toString("hex"))],
        ],
    );
    return hexToBytes(
        hex
    );
}

export function setConsensusTargetRate(
    protocolId: Buffer,
    targetRate: number,
): Buffer {
    let hex = ethers.utils.defaultAbiCoder.encode(
        ["tuple(bytes32, uint256)"],
        [[protocolId, targetRate]],
    );
    return hexToBytes(
        hex
    );
}
