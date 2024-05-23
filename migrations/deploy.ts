import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { Photon } from "../target/types/photon";
import { utf8 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { getKeypairFromFile } from "@solana-developers/node-helpers";
import fs from "fs";
import { ethers } from "ethers";
import * as readline from "readline";

const DEVNET = false;

const EOB_CHAIN_ID = DEVNET ? 33133 : 33033;
const ROOT = DEVNET ? utf8.encode("root-0") : utf8.encode("r0");
const GOV_CONSENSUS_TARGET_RATE = 6000;
const GOV_PROTOCOL_ID = Buffer.from(
    utf8.encode(
        "photon-gov\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
    )
);
const KEYS_PATH = DEVNET ? "./keys" : "./keys-mainnet";

const MSC = DEVNET
    ? "0x3f0A86C7e1cf8883A0FbDB11B671BE849168496b"
    : "0x2C36e392d6cD4A7c24Cd1C73d5681d0863d8B025";

function hexToBytes(hex: string): number[] {
    return Array.from(Buffer.from(hex.replace("0x", ""), "hex"));
}

module.exports = async function (provider: anchor.AnchorProvider) {
    console.log("Initialize gov protocol on", provider.connection.rpcEndpoint);
    anchor.setProvider(provider);

    process.chdir(".."); // starts in .anchor by default
    if (!fs.existsSync(KEYS_PATH)) {
        fs.mkdirSync(KEYS_PATH);
    }
    const transmitters: Array<Array<number>> = require("../" +
        KEYS_PATH +
        "/transmitters.json").map(hexToBytes);
    const owner = await getKeypairFromFile(KEYS_PATH + "/owner.json");
    const govExecutor1 = await getKeypairFromFile(KEYS_PATH + "/gov-executor1.json");
    const govExecutor2 = await getKeypairFromFile(KEYS_PATH + "/gov-executor2.json");
    const program = anchor.workspace.Photon as Program<Photon>;
    const config = web3.PublicKey.findProgramAddressSync(
        [ROOT, utf8.encode("CONFIG")],
        program.programId
    )[0];
    const protocolInfo = web3.PublicKey.findProgramAddressSync(
        [ROOT, utf8.encode("PROTOCOL"), GOV_PROTOCOL_ID],
        program.programId
    )[0];
    let eob_master_contract = ethers.utils.defaultAbiCoder.encode(["address"], [MSC]);
    let eob_master_contract_buf: Buffer = new Buffer(eob_master_contract.substring(2), "hex");
    console.log(
        "Transmitters:",
        transmitters.map(
            (num: number[]) => "0x" + num.map(x => x.toString(16).padStart(2, "0")).join("")
        )
    );
    console.log("Network:", DEVNET ? "Devnet" : "Mainnet");
    console.log("Owner account:", owner.publicKey.toBase58());
    console.log("Gov executor account 1:", govExecutor1.publicKey.toBase58());
    console.log("Gov executor account 2:", govExecutor2.publicKey.toBase58());
    console.log("Config account:", config.toBase58());
    console.log("Protocol info:", protocolInfo.toBase58());
    console.log("Photon program account:", program.programId.toBase58());
    console.log("eob_master_contract:", eob_master_contract);

    //await askAndExecute();

    let tx_signature = await program.methods
        .initialize(
            new anchor.BN(EOB_CHAIN_ID),
            eob_master_contract_buf,
            new anchor.BN(GOV_CONSENSUS_TARGET_RATE),
            transmitters,
            [govExecutor1.publicKey, govExecutor2.publicKey]
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

export function askAndExecute() {
    return new Promise<void>(resolve => {
        const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout,
        });

        rl.question("Do you want to proceed? (y/n): ", answer => {
            rl.close();
            if (answer.toLowerCase() === "y") {
                console.log("You chose to proceed.\n");
                resolve();
            } else if (answer.toLowerCase() === "n") {
                console.log("You chose not to proceed. Exiting...");
                process.exit();
            } else {
                console.log('Invalid input. Please enter either "y" or "n".');
                askAndExecute().then(resolve);
            }
        });
    });
}
