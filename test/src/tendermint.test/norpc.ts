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
                "--enable-devel-api"
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
    const numTransactions = 100000;

    for (let k = 0; k < 4; k++){
        for (let i = 0; i < 2; i++) {
            const buf = readFileSync(`./txes/${k}_${i * 50000}_${i * 50000 + 50000}.json`, "utf8");
            const txRaw: string[] = JSON.parse(buf);
            for (let j = 0; j < 50000; j++) {
                transactions[k].push(txRaw[j]);
            }
        }
    }

    console.log("Txes prepared");

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

    const startTime = new Date();
    console.log(`Start at: ${startTime}`);

    for (let k = 0; k < 4; k++) {
        await nodes[k].sdk.rpc.sendRpcRequest("mempool_sendSignedTransaction", [
            "0x" + transactions[k][0]
        ]);
    }

    const bnStart = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
    console.log(`BLOCK_START: ${bnStart}`);

    let lastNum = 0;
    let consumed = 0;

    while (true) {
        const num = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
        if (lastNum !== num) {
            lastNum = num;
            console.log("---------------------");
            console.log(`Block ${lastNum}`);
            const block = (await nodes[0].sdk.rpc.chain.getBlock(lastNum))!;
            const txnum = block!.transactions.length!;
            consumed += txnum;
            console.log(`Txs: ${txnum}`);

            const parentBlockFinalizedView = sealToNum(block.seal[0]);
            const authorView = sealToNum(block.seal[1]);
            console.log(`parent_block_finalized_view: ${parentBlockFinalizedView}`);
            console.log(`author_view: ${authorView}`);

            for (let i = 0; i < 4; i++) {
                const pendingTxnum = await nodes[
                    i
                ].sdk.rpc.chain.getPendingTransactionsCount();
                const futureTxnum = await nodes[
                    i
                ].sdk.rpc.sendRpcRequest("mempool_getCurrentFuturueCount", [
                    null,
                    null
                ]);
                console.log(`Txs in ${i}: ${pendingTxnum} ${futureTxnum}`);
            }

            console.log(`Consumed TX: ${consumed}`);
            if (consumed === numTransactions * 4) {
                break;
            }
        }
        await wait(100);
    }
    const endTime = new Date();
    console.log(`End at: ${endTime}`);
    const tps =
        (numTransactions * 1000.0 * 4) /
        (endTime.getTime() - startTime.getTime());
    console.log(
        `Elapsed time (ms): ${endTime.getTime() - startTime.getTime()}`
    );

    const bnEnd = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
    console.log(`BLOCK: ${bnEnd}`);

    for (let i = 0; i <= bnEnd; i++) {
        const block = await nodes[0].sdk.rpc.chain.getBlock(i);
        if (block != null) {
            console.log(`BLOCK${i} : ${block.transactions.length}`);
        } else {
            console.log(`BLOCK${i} : null`);
        }
    }

    console.log(`TPS: ${tps}`);

    await Promise.all(nodes.map(node => node.clean()));
})().catch(console.error);
