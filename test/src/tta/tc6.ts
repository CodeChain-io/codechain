import { SDK } from "codechain-sdk";
import { H256 } from "codechain-sdk/lib/core/classes";
import { readFileSync  } from "fs";

async function main() {
    const transactions: string[][] = [[], [], [], []];
    const numTransactions = 1000;
    const sdk = new SDK({
        server: "http://127.0.0.1:2487",
        networkId: "tc"
    });

    await delay(1000);

    for (let k = 0; k < 4; k++){
        for (let i = 0; i < 2; i++) {
            const buf = readFileSync(`./prepared_transactions/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }

    findBlock(sdk).catch(console.error);
    const txHashes: H256[] = [];

    for (let k = 0; k < 4; k++) {
        let i = numTransactions - 1;
        while(i > 0) {
            const txes = [];
            for (let j = 0; j < 2000; j++) {
                txes.push(transactions[k][i]);
                i--;
                if (i ===-1) {
                    break;
                }
            }
            for (const tx of txes) {
                const txHash = await sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [[tx]]);
                txHashes.push(txHash)
            }
        }
    }

    return;
};

main().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}

async function findBlock(sdk: SDK) {
    let prevBestBlockNumber = await sdk.rpc.chain.getBestBlockNumber();
    while (true) {
        await delay(10);
        const bestBlockNumber = await sdk.rpc.chain.getBestBlockNumber();
        if (bestBlockNumber === prevBestBlockNumber) {
            continue;
        }

        for (; prevBestBlockNumber < bestBlockNumber; prevBestBlockNumber += 1) {
            const block = await sdk.rpc.chain.getBlock(bestBlockNumber);
            if (block!.transactions.length > 0) {
                await printBlock(sdk, bestBlockNumber);
                process.exit(0);
            }
        }
    }
}

async function printBlock(sdk: SDK, blockNumber: number) {
    const block = await sdk.rpc.chain.getBlock(blockNumber);
    console.group(`Block ${block!.number}`);
    console.log(`hash: ${block!.hash.toString()}`);
    console.log(`transactionCount: ${block!.transactions.length}`);
    console.groupEnd();
}
