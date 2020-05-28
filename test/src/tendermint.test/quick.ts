
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
            const buf = readFileSync(`/home/junha/Desktop/txsame/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
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
    
    let observer = observe(nodes, numTransactions);
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
                    sendPromise.push(nodes[k].sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [
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
    let lastNum = 0;
    let consumed = 0;
    while(true) {
        let newTime = new Date();
        const num = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            let totalElapsed = newTime.getTime() - startTime.getTime();
            console.log("-----------------[REPORT]----------------");
            for (let b = lastNum + 1; b <= num; b++) {
                let currentBlock = (await nodes[0].sdk.rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",[b]))!;
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
                const futureTxnum = await nodes[k].
                sdk.rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
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