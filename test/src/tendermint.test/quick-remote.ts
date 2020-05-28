
import { readFileSync } from "fs";
import { SDK } from "codechain-sdk";
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
    const servers = ["http://192.168.1.42:2487", "http://192.168.1.101:2487", "http://192.168.1.102:2487", "http://192.168.1.103:2487"];
    const sdks = servers.map(server => new SDK({
        server,
    }));
    //{ argv: ["--no-tx-relay"] }

    const transactions: string[][] = [[], [], [], []];
    const numTransactions = 400000;

    for (let k = 0; k < 4; k++){
        for (let i = 0; i < 8; i++) {
            const buf = readFileSync(`/home/spkim2/Downloads/txsame/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }
    console.log("Txes loaded");

    /// EXPERIMENT PARAMS
    const goalTps = 1000; // per Node
    const bulkSize = 1000;
    
    let observer = observe(sdks, numTransactions);
    let sender = async function() {
        let txIndex = [0, 0, 0, 0];
        const startTime = new Date();
        console.log(`Start at: ${startTime}`);
        let totalSent = 0;
        let txToSend = 0;
        let lastTime = new Date();

        while(totalSent < numTransactions) {
            let newTime = new Date();
            let elapsed = newTime.getTime() - lastTime.getTime();
            const txsNum = Math.round(goalTps * elapsed * 0.001);
            if (txsNum > 10) {
                lastTime = newTime;
                txToSend += txsNum;
            }
            let sendPromise = [];
            if (txToSend > bulkSize) {
                for(let k = 0; k < 4; k++) {
                    if (txIndex[k] === numTransactions) break;
                    const txs = [];
                    for (let i = 0; i < bulkSize; i++) {
                        txs.push(transactions[k][txIndex[k]]);
                        txIndex[k] += 1;
                    }
                    sendPromise.push(sdks[k].rpc.sendRpcRequest("mempool_sendSignedTransactions", [
                        txs
                    ]));
                }
                await Promise.all(sendPromise);
                console.log(`Tx sent: ${bulkSize * 4}`);
                console.log(`Tx left: ${txToSend}`);
                totalSent += bulkSize;
                txToSend -= bulkSize;
            } 
            await delay(10);
        }
    }();
    await Promise.all([observer, sender]);

})().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}


async function observe(sdks: SDK[], txNum: number) {
    const startTime = new Date();
    console.log(`Start at: ${startTime}`);
    let lastNum = 0;
    let consumed = 0;
    while(true) {
        let newTime = new Date();
        const num = await sdks[0].rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            let totalElapsed = newTime.getTime() - startTime.getTime();
            console.log("-----------------[REPORT]----------------");
            for (let b = lastNum + 1; b <= num; b++) {
                let currentBlock = (await sdks[0].rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",[b]))!;
                consumed += currentBlock.transactionCount;
                console.log(`<BLOCK ${b}>`);
                const parentBlockFinalizedView = sealToNum(currentBlock.seal[0]);
                const authorView = sealToNum(currentBlock.seal[1]);
                console.log(`parent_block_finalized_view: ${parentBlockFinalizedView}`);
                console.log(`author_view: ${authorView}`);
                console.log(`Tx included: ${currentBlock.transactionCount}`);
                console.log("");
            }
            console.log("<Status>");
            for (let k = 0; k < 4; k++) {
                const futureTxnum = await sdks[k].rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ]);
                console.log(`Mempool for Node ${k}: ${futureTxnum}`);
            }
            console.log(`Total Consumed: ${consumed}`);
            console.log(`Total Elapsed: ${totalElapsed}`);
            console.log(`TPS: ${consumed/totalElapsed * 1000}`);

            lastNum = num;

            if (consumed === txNum * 4) {
                break;
            }
        }
        await delay(50);
    }
}