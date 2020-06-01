
import { SignedTransaction, H256 } from "codechain-sdk/lib/core/classes";
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
import { SDK } from "codechain-sdk";
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
    await Promise.all(nodes.map(node => node.start({ argv: ["--no-tx-relay"] })));

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
    const numTransactions = 10000;

    for (let k = 0; k < 4; k++){
        for (let i = 0; i < 2; i++) {
            const buf = readFileSync(`./prepared_transactions/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }

    let txHashes: H256[] = [];
    
    for (let k = 0; k < 4; k++) {
        let i = numTransactions - 1;
        while(i > 0) {
            console.log(`${i}`);
            const txes = [];
            for (let j = 0; j < 2000; j++) {
                txes.push(transactions[k][i]);
                i--;
                if (i ===-1) {
                    break;
                }
            }
            txHashes = txHashes.concat((await nodes[k].sdk.rpc.sendRpcRequest("mempool_sendSignedTransactions", [
                txes
            ]))!);
        }
    }
    console.log("Txes loaded");

    await consume_all(nodes[0].sdk, numTransactions * 4);

    console.log("DONE!");

    let concurrency = 64;
    const queryTasks = []; 

    const totalCount = [0];

    const startTime = new Date();
    console.log(`Start at: ${startTime}`);

    for (let con = 0; con < concurrency; con++) 
    {
        queryTasks.push(async function(c: number) {
            const sdk = nodes[c % 4].sdk;
            for (let i = c; i < txHashes.length; i+= concurrency) {
                await sdk.rpc.chain.getTransaction(txHashes[i]);
                totalCount[0] += 1;
            }
        }(con));
    }

    queryTasks.push(async function() {
        while(totalCount[0] < 4 * numTransactions) {
            console.log(`${totalCount[0]}`);
            await delay(500);
        }
    }());   
    await Promise.all(queryTasks);

    let endTime = new Date();
    let totalElapsed = endTime.getTime() - startTime.getTime();

    console.log("<STATUS>");
    console.log(`Total Consumed: ${txHashes.length}`);
    console.log(`Total Elapsed: ${totalElapsed}`);
    console.log(`TPS: ${txHashes.length/totalElapsed * 1000}`);

    return;

})().catch(console.error);

async function delay(m: number) {
    return new Promise(resolve => {
        setTimeout(resolve, m);
    });
}

async function consume_all(sdk: SDK, txNum: number ){ 
    let consumed = 0;
    let lastNum = 0;
    while(consumed < txNum ) {
        const num = await sdk.rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            for (let b = lastNum + 1; b <= num; b++) {

                const futureTxnum = await 
                sdk.rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ]);
                let count = (await sdk.rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",[b]))!.transactionCount;
                consumed += count;
                console.log(`Consumed: ${count} / Total Left: ${txNum - consumed}`);
            }
            lastNum = num;
        }
        await delay(100);
    }
}
