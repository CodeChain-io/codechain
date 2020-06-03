import { SDK } from "codechain-sdk";
import { H256, SignedTransaction } from "codechain-sdk/lib/core/classes";
import { existsSync, readFileSync, writeFileSync } from "fs";
import {
    faucetAddress,
    faucetSecret,
    validator0Address,
    validator0Secret,
    validator1Address,
    validator1Secret,
    validator2Address,
    validator2Secret,
    validator3Address,
    validator3Secret
} from "../helper/constants";
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
    const sdks = rpcServers.map(
        server =>
            new SDK({
                server
            })
    );

    const secrets = [
        validator0Secret,
        validator1Secret,
        validator2Secret,
        validator3Secret
    ];
    const transactions: string[][] = [[], [], [], []];
    const numTransactions = 20000;

    for (let k = 0; k < 4; k++) {
        for (let i = 0; i < 2; i++) {
            const buf = readFileSync(
                `./prepared_transactions/${k}_${i * 50000}_${i * 50000 +
                    50000}.json`,
                "utf8"
            );
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }

    let txHashes: H256[] = [];

    for (let k = 0; k < 4; k++) {
        let i = numTransactions - 1;
        while (i > 0) {
            console.log(`${i}`);
            const txes = [];
            for (let j = 0; j < 2000; j++) {
                txes.push(transactions[k][i]);
                i--;
                if (i === -1) {
                    break;
                }
            }
            txHashes = txHashes.concat(
                (await sdks[
                    k
                ].rpc.sendRpcRequest("mempool_sendSignedTransactions", [txes]))!
            );
        }
    }
    console.log("Txes loaded");

    await consume_all(sdks, numTransactions * 4);

    console.log("DONE!");

    const asycnTasks = 64;
    const queryTasks = [];

    const result: SignedTransaction[] = [];

    const startTime = new Date();
    console.log(`Start at: ${startTime}`);

    for (let con = 0; con < asycnTasks; con++) {
        queryTasks.push(
            (async function(c: number) {
                const sdk = sdks[c % 4];
                for (let i = c; i < txHashes.length; i += asycnTasks) {
                    result.push(
                        (await sdk.rpc.chain.getTransaction(txHashes[i]))!
                    );
                }
            })(con)
        );
    }

    queryTasks.push(
        (async function() {
            while (result.length < 4 * numTransactions) {
                console.log(`${result.length}`);
                await delay(500);
            }
        })()
    );
    await Promise.all(queryTasks);

    const endTime = new Date();
    const totalElapsed = endTime.getTime() - startTime.getTime();

    console.log("-----------------<REPORT>-------------------");
    console.log(`Total Consumed: ${txHashes.length}`);
    console.log(`Total Elapsed: ${totalElapsed}`);
    console.log(`TPS: ${(txHashes.length / totalElapsed) * 1000}`);

    console.log("");
    console.log("-----------------<LAST 40>------------------");
    for (let i = 0; i < 40; i++) {
        const k = JSON.stringify(result[result.length - 1 - i].toJSON());
        console.log(`${k}`);
    }

    return;
})().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}

async function consume_all(sdks: SDK[], txNum: number) {
    let consumed = 0;
    let lastNum = -1;
    while (consumed < txNum) {
        const num = await sdks[0].rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            for (let b = lastNum + 1; b <= num; b++) {
                const count = (await sdks[0].rpc.sendRpcRequest(
                    "chain_getHeaderAndTxCountByNumber",
                    [b]
                ))!.transactionCount;
                consumed += count;
                console.log(`Block #: ${b}`);
                console.log(
                    `Consumed: ${count} / Total Left: ${txNum - consumed}`
                );

                if (consumed === txNum) {
                    break;
                }
            }
            lastNum = num;
        }
        await delay(100);
    }
}
