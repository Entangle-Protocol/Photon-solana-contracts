import * as anchor from "@coral-xyz/anchor";
import {Program, web3} from "@coral-xyz/anchor";
import {Photon} from "../target/types/photon";
import {utf8} from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {getKeypairFromFile} from "@solana-developers/node-helpers";
import fs, {readFileSync} from "fs";
import {ethers} from "ethers";


const EOB_CHAIN_ID = 33133;
const ROOT = utf8.encode("root-0");
const GOV_CONSENSUS_TARGET_RATE = 6000;
const GOV_PROTOCOL_ID = Buffer.from(
    utf8.encode("gov-protocol\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
);
const KEYS_PATH = "./keys";

function hexToBytes(hex: string): number[] {
    return Array.from(Buffer.from(hex.replace("0x", ""), "hex"));
}

module.exports = async function (provider: anchor.AnchorProvider) {
    console.log("Initialize gov protocol on", provider.connection.rpcEndpoint);
    anchor.setProvider(provider);

    process.chdir('..'); // starts in .anchor by default
    if (!fs.existsSync(KEYS_PATH)) {
        fs.mkdirSync(KEYS_PATH);
    }
    const transmitters: Array<Array<number>> = require("../" + KEYS_PATH + "/transmitters.json").map(hexToBytes);
    const owner = await getKeypairFromFile(KEYS_PATH + "/owner.json");
    const govExecutor = await getKeypairFromFile(KEYS_PATH + "/gov-executor.json");
    const program = anchor.workspace.Photon as Program<Photon>;
    const config = web3.PublicKey.findProgramAddressSync(
        [ROOT, utf8.encode("CONFIG")],
        program.programId,
    )[0];
    const protocolInfo = web3.PublicKey.findProgramAddressSync(
        [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
        program.programId,
    )[0];
    let eob_master_contract = ethers.utils.defaultAbiCoder.encode(["address"], ["0xe981b4f9580cce1ff1b87d63a9b68e53110b9aa7"]);
    let eob_master_contract_buf: Buffer = new Buffer(eob_master_contract.substring(2), "hex");
    console.log("Transmitters:", transmitters.map((num: number[]) =>
        "0x" + num.map((x) => x.toString(16).padStart(2, '0')).join("")));
    console.log("Owner account:", owner.publicKey.toBase58());
    console.log("Gov executor account:", govExecutor.publicKey.toBase58());
    console.log("Config account:", config.toBase58());
    console.log("Protocol info:", protocolInfo.toBase58());
    console.log("Photon program account:", program.programId.toBase58());
    console.log("eob_master_contract:", eob_master_contract);

    let tx_signature = await program.methods
        .initialize(
            new anchor.BN(EOB_CHAIN_ID),
            eob_master_contract_buf,
            new anchor.BN(GOV_CONSENSUS_TARGET_RATE),
            transmitters,
            [govExecutor.publicKey],
        )
        .accounts({
            admin: owner.publicKey,
            protocolInfo,
            config,
            systemProgram: web3.SystemProgram.programId,
        })
        .signers([owner])
        .rpc();
    console.log("initialize tx:", tx_signature);
};
