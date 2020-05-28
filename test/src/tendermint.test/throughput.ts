// Copyright 2018-2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

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
import { makeRandomH256 } from "../helper/random";
import CodeChain from "../helper/spawn";
const {
    Worker,
    isMainThread,
    parentPort,
    workerData
} = require("worker_threads");
const path = require("path");

const RLP = require("rlp");

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

    const startBlockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
    const startBlock = await nodes[0].sdk.rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber", [startBlockNumber]);

    sendTransactionLoop({ nodes }).catch(console.error);

    let currentBlockNumber = startBlockNumber;
    let currentBlock = startBlock;
    let curTime = new Date();
    let totalTime = 0;
    let totalTransactionCount = 0;
    while (true) {
        const newBlockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
        if (currentBlockNumber === newBlockNumber) {
            await delay();
            continue;
        }

        currentBlockNumber = newBlockNumber;
        currentBlock = (await nodes[0].sdk.rpc.sendRpcRequest("chain_getHeaderAndTxCountByNumber",
            [currentBlockNumber]
        ))!;
        const prevTime = curTime;
        curTime = new Date();
        const txCount = currentBlock.transactionCount;
        const usedSeconds = (curTime.getTime() - prevTime.getTime()) / 1000;

        console.log(`New block ${currentBlockNumber}`);
        console.log(`Tx count: ${txCount}`);
        console.log(`Used time: ${usedSeconds}`);
        console.log(`TPS: ${txCount / usedSeconds}`);
        const parentBlockFinalizedView = sealToNum(currentBlock.seal[0]);
        const authorView = sealToNum(currentBlock.seal[1]);
        console.log(`parent_block_finalized_view: ${parentBlockFinalizedView}`);
        console.log(`author_view: ${authorView}`);
        totalTransactionCount += txCount;
        totalTime += usedSeconds;
        console.log(
            `Average: ${totalTransactionCount /
                totalTime} = ${totalTransactionCount} / ${totalTime}`
        );
        const pendingCounts = await Promise.all(
            nodes.map(node =>
                node.sdk.rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ])
            )
        );
        console.log(`Pending counts: ${JSON.stringify(pendingCounts)}`);
        console.log("");
    }
})().catch(console.error);

function sealToNum(rlp: any) {
    const buffer = RLP.decode(Buffer.from([rlp]));
    if (buffer.length === 0) {
        return 0;
    } else {
        return buffer.readUInt8();
    }
}

async function delay() {
    return new Promise(resolve => {
        setTimeout(resolve, 10);
    });
}

async function sendTransactionLoop({ nodes }: any) {
    const promises = [];
    const validatorSecrets = [
        validator0Secret,
        validator1Secret,
        validator2Secret,
        validator3Secret
    ];

    for (let index = 0; index < 4; index += 1) {
        const worker = new Worker(
            path.resolve(__dirname, "./throughput-worker.js"),
            {
                workerData: {
                    index,
                    port: nodes[index].rpcPort,
                    validatorSecrets
                }
            }
        );

        const workerPromise = new Promise((resolve, reject) => {
            worker.on("error", reject);
            worker.on("exit", (code: any) => {
                if (code !== 0) {
                    reject(new Error(`Worker stopped with exit code ${code}`));
                }
            });
        });
        promises.push(workerPromise);
    }
}
