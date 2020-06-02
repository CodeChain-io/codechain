import { SDK } from "codechain-sdk";
import { readFileSync } from "fs";
const RLP = require("rlp");

function sealToNum(rlp: any) {
    const buffer = RLP.decode(Buffer.from([rlp]));
    if (buffer.length === 0) {
        return 0;
    } else {
        return buffer.readUInt8();
    }
}

(async () => {
    const rpcServers = [
    "http://52.53.237.116:2487",
    "http://54.67.67.96:2487",
    "http://54.176.9.137:2487",
    "http://54.151.59.22:2487"
    ];
    const sdks = rpcServers.map(server => new SDK({
        server
    }));

    const transactions: string[][] = [[], [], [], []];
    const numTransactions = 400000;

    for (let k = 0; k < 4; k++){
        for (let i = 0; i < 8; i++) {
            const buf = readFileSync(`./prepared_transactions/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }
    console.log("Txes loaded");

    /// EXPERIMENT PARAMS
    const desiredMempoolSize = 30000;
    const maxSend = 4000;
    const minSend = 1000;

    
    const observer = observe(sdks, numTransactions);
    const tasks = [];
    for (let k = 0; k < 4; k++) {
        tasks.push(async function(node_num: number) {
            let index = 0;
            let sentCount = 0;
            while (true) {
                const futureCnt = await sdks[node_num].rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ]);
                const txToSend = Math.min(maxSend, desiredMempoolSize - futureCnt[0]);
                if (txToSend > minSend) {
                    const txs = [];
                    for (let i = 0; i < txToSend; i++) {
                        txs.push(transactions[node_num][index]);
                        index += 1;
                        if (index === numTransactions) { break; }
                    }
                    await sdks[node_num].rpc.sendRpcRequest("mempool_sendSignedTransactions", [
                        txs
                    ])
                    sentCount += txToSend;
                    if (sentCount > 5000) {
                        console.log(`-----------------[TX Sent]-------------------`)
                        console.log(`Txs sent for Node ${node_num}: ${sentCount}`);
                        console.log(`Mempool for Node ${node_num}: ${futureCnt}`);
                        sentCount = 0;
                    }
                } else {
                    await delay(10);
                }
                if (index === numTransactions) { return; }
            }
        }(k));
    }
    tasks.push(observer);
    await Promise.all(tasks);

})().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}

async function observe(sdks: SDK[], txNum: number) {
    const startTime = new Date();
    console.log(`Start at: ${startTime}`);
    let lastNum = await sdks[0].rpc.chain.getBestBlockNumber();
    let consumed = 0;
    while(true) {
        const newTime = new Date();
        const num = await sdks[0].rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            const totalElapsed = newTime.getTime() - startTime.getTime();
            const blocks = [];
            for (let b = lastNum + 1; b <= num; b++) {
                blocks.push((await sdks[0].rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",[b]))!);
            }

            console.log("-----------------[REPORT]----------------");
            for (let i = 0; i < num - lastNum; i++) {
                consumed += blocks[i].transactionCount;
                console.log(`<BLOCK ${lastNum + 1 + i}>`);
                const parentBlockFinalizedView = sealToNum(blocks[i].seal[0]);
                const authorView = sealToNum(blocks[i].seal[1]);
                console.log(`parent_block_finalized_view: ${parentBlockFinalizedView}`);
                console.log(`author_view: ${authorView}`);
                console.log(`Tx included: ${blocks[i].transactionCount}`);
            }
            console.log("<STATUS>");
            console.log(`Total Consumed: ${consumed}`);
            console.log(`Total Elapsed: ${totalElapsed}`);
            console.log(`TPS: ${consumed/totalElapsed * 1000}`);

            lastNum = num;
            if (consumed === txNum * 4) {
                break;
            }
        }
        await delay(100);
    }
}