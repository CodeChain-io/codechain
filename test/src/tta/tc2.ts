
import { SignedTransaction } from "codechain-sdk/lib/core/classes";
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
import { wait } from "../helper/promise";
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";
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
    let nodes: CodeChain[];

    const validatorAddresses = [
        validator0Address,
        validator1Address,
        validator2Address,
        validator3Address
    ];
    const futureGapInMS = 360 * 24 * 60 * 60 * 1000;
    nodes = validatorAddresses.map(address => {
        return new CodeChain({
            chain: `${__dirname}/../scheme/tendermint-tps.json`,
            argv: [
                "--engine-signer",
                address.toString(),
                "--password-path",
                "test/tendermint/password.json",
                "--force-sealing",
                "--no-discovery",
                "--enable-devel-api",
                "--allowed-future-gap",
                String(futureGapInMS)
            ],
            additionalKeysPath: "tendermint/keys"
        });
    });
    //{ argv: ["--no-tx-relay"] }
    await Promise.all(nodes.map(node => node.start()));

    await Promise.all([
        nodes[0].connect(nodes[1]),
        nodes[0].connect(nodes[2]),
        nodes[0].connect(nodes[3]),
        nodes[1].connect(nodes[2]),
        nodes[1].connect(nodes[3]),
        nodes[2].connect(nodes[3])
    ]);
    await Promise.all([
        nodes[0].waitPeers(4 - 1),
        nodes[1].waitPeers(4 - 1),
        nodes[2].waitPeers(4 - 1),
        nodes[3].waitPeers(4 - 1)
    ]);

    const secrets = [
        validator0Secret,
        validator1Secret,
        validator2Secret,
        validator3Secret
    ];
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

    for (let k = 0; k < 4; k++) {
        let i = numTransactions - 1;
        while(i > 0) {
            console.log(`${i}`);
            const txes = [];
            for (let j = 0; j < 2000; j++) {
                txes.push( "0x" + transactions[k][i]);
                i--;
                if (i ===0) {
                    break;
                }
            }
            await nodes[k].sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [
                txes
            ]);
        }
    }

    /// EXPERIMENT PARAMS
    const desiredMempoolSize = 30000;
    const maxSend = 4000;
    const minSend = 1000;

    
    let observer = observe(nodes, numTransactions);
    let tasks = [];
    for (let k = 0; k < 4; k++) {
        tasks.push(async function(k: number) {
            let index = 0;
            let sentCount = 0;
            while (true) {
                const futureTxnum = await nodes[k].
                sdk.rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ]);
                const txToSend = Math.min(maxSend, desiredMempoolSize - futureTxnum[0]);
                if (txToSend > minSend) {
                    const txs = [];
                    for (let i = 0; i < txToSend; i++) {
                        txs.push(transactions[k][index]);
                        index += 1;
                        if (index == numTransactions) break;
                    }
                    await nodes[k].sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [
                        txs
                    ])
                    sentCount += txToSend;
                    if (sentCount > 5000) {
                        console.log(`-----------------[TX Sent]-------------------`)
                        console.log(`Txs sent for Node ${k}: ${sentCount}`);
                        console.log(`Mempool for Node ${k}: ${futureTxnum}`);
                        sentCount = 0;
                    }
                } else {
                    await delay(10);
                }
                if (index == numTransactions) return;
            }
        }(k));
    }
    tasks.push(observer);
    await Promise.all(tasks);
    await Promise.all(nodes.map(node => node.clean()));


})().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}

async function observe(nodes: CodeChain[], txNum: number) {
    const startTime = new Date();
    console.log(`Start at: ${startTime}`);
    let lastNum = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
    let consumed = 0;
    while(true) {
        let newTime = new Date();
        const num = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            let totalElapsed = newTime.getTime() - startTime.getTime();
            let blocks = [];
            for (let b = lastNum + 1; b <= num; b++) {
                blocks.push((await nodes[0].sdk.rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",[b]))!);
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