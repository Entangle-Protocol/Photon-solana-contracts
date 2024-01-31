import { OperationLib } from "@entangle_protocol/oracle-sdk/dist/typechain-types/contracts/AggregationSpotter";
import { SignerWithAddress } from "@nomiclabs/hardhat-ethers/signers";
import { expect } from "chai";
import { Wallet, ethers } from "ethers";
import * as anchor from "@coral-xyz/anchor";

interface AnchorOpData {
  protocolId: Buffer;
  srcChainId: anchor.BN;
  srcBlockNumber: anchor.BN;
  srcOpTxId: Buffer;
  nonce: anchor.BN;
  destChainId: anchor.BN;
  protocolAddr: anchor.web3.PublicKey;
  functionSelector: number[];
  params: Buffer;
}

function opHash(opData: OperationLib.OperationDataStruct) {
  return ethers.utils.solidityKeccak256(
    [
      "bytes32",
      "uint256",
      "uint256",
      "bytes32",
      "uint256",
      "uint256",
      "bytes",
      "bytes4",
      "bytes",
    ],
    [
      opData.protocolId,
      opData.srcChainId,
      opData.srcBlockNumber,
      opData.srcOpTxId,
      opData.nonce,
      opData.destChainId,
      opData.protocolAddr,
      opData.functionSelector,
      opData.params,
    ],
  );
}

function convertOpData(opData: AnchorOpData): OperationLib.OperationDataStruct {
  return {
    protocolId: Buffer.from(opData.protocolId),
    srcChainId: opData.srcChainId.toNumber(),
    srcBlockNumber: opData.srcBlockNumber.toNumber(),
    srcOpTxId: Buffer.from(opData.srcOpTxId),
    nonce: opData.nonce.toNumber(),
    destChainId: opData.destChainId.toNumber(),
    protocolAddr: opData.protocolAddr.toBytes(),
    functionSelector: Buffer.from(opData.functionSelector),
    params: opData.params,
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

export async function signOp(keeper: Wallet, op: AnchorOpData) {
  const msgHash = ethers.utils.arrayify(opHash(convertOpData(op)));
  const sign = ethers.utils.splitSignature(await keeper.signMessage(msgHash));
  const v = sign.v;
  const r = hexToBytes(sign.r);
  const s = hexToBytes(sign.s);
  expect(keeper.address).eq(ethers.utils.verifyMessage(msgHash, sign));
  return { v, r, s };
}

export function randomSigners(amount: number): Wallet[] {
  const signers = [];
  for (let i = 0; i < amount; i++) {
    signers.push(ethers.Wallet.createRandom());
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
  keepersRaw: number[][],
  consensusTargetRate: number,
): Buffer {
  return hexToBytes(
    ethers.utils.defaultAbiCoder.encode(
      ["bytes32", "uint", "uint", "address[]"],
      [
        protocolId,
        consensusTargetRate,
        1000,
        keepersRaw.map((x) => Buffer.from(x).toString("hex")),
      ],
    ),
  );
}

export function addAllowedProtocolAddress(
  protocolId: Buffer,
  protocolAddr: anchor.web3.PublicKey,
): Buffer {
  return hexToBytes(
    ethers.utils.defaultAbiCoder.encode(
      ["bytes32", "bytes"],
      [protocolId, protocolAddr.toBuffer()],
    ),
  );
}

export function addExecutor(
  protocolId: Buffer,
  executor: anchor.web3.PublicKey,
): Buffer {
  return hexToBytes(
    ethers.utils.defaultAbiCoder.encode(
      ["bytes32", "bytes"],
      [protocolId, executor.toBuffer()],
    ),
  );
}

export function addKeepers(protocolId: Buffer, keepersRaw: number[][]): Buffer {
  return hexToBytes(
    ethers.utils.defaultAbiCoder.encode(
      ["bytes32", "address[]"],
      [protocolId, keepersRaw.map((x) => Buffer.from(x).toString("hex"))],
    ),
  );
}

export function setConsensusTargetRate(
  protocolId: Buffer,
  targetRate: number,
): Buffer {
  return hexToBytes(
    ethers.utils.defaultAbiCoder.encode(
      ["bytes32", "uint256"],
      [protocolId, targetRate],
    ),
  );
}
