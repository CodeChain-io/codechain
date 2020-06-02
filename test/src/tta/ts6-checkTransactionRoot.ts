import * as assert from "assert";
import { SDK } from "codechain-sdk";
import { H256 } from "codechain-sdk/lib/core/classes";
import { blake256 } from "codechain-sdk/lib/utils";

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
        server: "http://127.0.0.1:2487",
        networkId: NETWORK_ID
    });

    const block = await sdk.rpc.chain.getBlock(blockNumber);
    if (block === null) {
        console.log(`Cannot find the block ${blockNumber}`);
        process.exit(1);
    }

    const parentBlock = await sdk.rpc.chain.getBlock(blockNumber - 1);
    const parentTransactionRoot = parentBlock!.transactionsRoot;
    const transactionsRoot = block!.transactionsRoot;
    const calculatedTxRoot = calculateTransactionRoot(
        parentTransactionRoot,
        block!.transactions.map(tx => tx.hash())
    );
    const txCount = block!.transactions.length;
    console.log(`Transaction count ${txCount}`);
    console.log(`TxRoot in Header: ${transactionsRoot}`);
    console.log(`Calculated TxRoot: ${calculatedTxRoot}`);

    console.log(
        `TxRoot in Header ${transactionsRoot} === Calculated TxRoot ${calculatedTxRoot}: ${transactionsRoot.value ===
            calculatedTxRoot.value}`
    );
}

function calculateTransactionRoot(parentRoot: H256, txHashes: H256[]) {
    const acc = parentRoot;
    for (const txHash of txHashes) {
        const xor = XOR(acc, txHash);
        const xorBuffer = Buffer.from(xor.value, "hex");
        const hashed = blake256(xorBuffer);
        acc.value = hashed;
    }
    return acc;
}

function XOR(a: H256, b: H256) {
    const aBuffer = Buffer.from(a.value, "hex");
    const bBuffer = Buffer.from(b.value, "hex");

    assert(aBuffer.length === bBuffer.length);
    const retBuffer = Buffer.alloc(aBuffer.length, 0);

    for (let i = 0; i < aBuffer.length; i += 1) {
        retBuffer[i] = aBuffer[i] ^ bBuffer[i];
    }

    const ret = H256.zero();
    ret.value = retBuffer.toString("hex");
    return ret;
}

main().catch(console.error);
