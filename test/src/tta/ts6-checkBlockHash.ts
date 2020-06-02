import { SDK } from "codechain-sdk";
import {Block} from "codechain-sdk/lib/core/classes";
import {blake256} from "codechain-sdk/lib/utils";

const RLP = require("rlp");
const NETWORK_ID = "tc";

async function main() {
    if (!process.argv[2]) {
        console.log(
            "No argument error. Please use ts6-checkBlockHash <blockNumber>"
        );
        process.exit(1);
    }

    const blockNumber = Number.parseInt(process.argv[2], 10);

    const sdk = new SDK({
        //server: "http://127.0.0.1:2487",
        server: "https://corgi-rpc.codechain.io",
        networkId: NETWORK_ID
    });

    const parentBlock = await sdk.rpc.chain.getBlock(blockNumber - 1);
    printBlockDefault(parentBlock!);
    const block = await sdk.rpc.chain.getBlock(blockNumber);
    printBlockDefault(block!);

    console.log();
    const serializedHeader = serializeHeader(block!).toString("hex");
    console.log(`Serialized header ${blockNumber} ${serializedHeader}\n`);

    const calculatedBlockHash = calculateBlockHash(block!);
    console.log(`Calculated block hash ${calculatedBlockHash}`);

    console.log();
    console.log(`Serialized header ${blockNumber} contains block ${blockNumber - 1}'s hash at index ${serializedHeader.indexOf(parentBlock!.hash.toString())}`);
    console.log(`Calculated block hash ${calculatedBlockHash} === Received block hash ${block!.hash}: ${calculatedBlockHash === block!.hash.toString()}`);
}

main().catch(console.error);

function serializeHeader(block: Block) {
    const {
        parentHash,
        timestamp,
        number,
        author,
        extraData,
        transactionsRoot,
        stateRoot,
        score,
        seal
    } = block;

    let blockHeader: any[] = [];
    blockHeader.push(hex2Buf(parentHash.toEncodeObject()));
    blockHeader.push(hex2Buf(author.getAccountId().toEncodeObject()));
    blockHeader.push(hex2Buf(stateRoot.toEncodeObject()));
    blockHeader.push(hex2Buf(transactionsRoot.toEncodeObject()))
    blockHeader.push(hex2Buf(score.toEncodeObject() as any));
    blockHeader.push(number);
    blockHeader.push(timestamp);
    blockHeader.push(Buffer.from(extraData));
    blockHeader = blockHeader.concat(seal.map(s => {
        const buffer = Buffer.from(s)
        return RLP.decode(buffer)
    }));

    const encoded: Buffer = RLP.encode(
        blockHeader
    );

    return encoded;
}

function calculateBlockHash(block: Block) {
    const encoded: Buffer = serializeHeader(block);
    return blake256(encoded);
}

function hex2Buf(hexString: string) {
    let stripped = hexString.startsWith("0x") ? hexString.slice(2) : hexString;
    return Buffer.from(stripped, "hex");
};

function printBlockDefault(block: Block) {
    console.group(`Block number ${block.number}`);
    console.log(`block hash ${block.hash}`);
    console.log(`Parent block hash ${block.parentHash}`);
    console.groupEnd();
}
