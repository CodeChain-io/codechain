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

import { expect } from "chai";
import { H256 } from "codechain-primitives/lib";
import {
    Asset,
    SignedTransaction,
    Timelock
} from "codechain-sdk/lib/core/classes";
import "mocha";
import { faucetAddress } from "../helper/constants";
import CodeChain from "../helper/spawn";
import json = Mocha.reporters.json;

describe("Sealing test", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("stopSealing then startSealing", async function() {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendPayTx();
        expect(await node.getBestBlockNumber()).to.equal(0);
        await node.sdk.rpc.devel.startSealing();
        expect(await node.getBestBlockNumber()).to.equal(1);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

describe("Future queue", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("all pending transactions must be mined", async function() {
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;
        await node.sendPayTx({ seq: seq + 3 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendPayTx({ seq: seq + 2 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendPayTx({ seq: seq + 1 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendPayTx({ seq: seq });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(
            seq + 4
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

describe("Delete All Pending Transactions", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("all pending transactions should be deleted", async function() {
        await node.sdk.rpc.devel.stopSealing();

        const sq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;

        await node.sendPayTx({ seq: sq + 0 }); // will be in the current queue
        await node.sendPayTx({ seq: sq + 3 }); // will be in the future queue

        await node.sdk.rpc.sendRpcRequest(
            "mempool_deleteAllPendingTransactions",
            []
        );

        const {
            transactions: wholeTXs
        } = await node.sdk.rpc.sendRpcRequest(
            "mempool_getPendingTransactions",
            [null, null, true]
        );

        expect(wholeTXs.length).to.equal(0);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

describe("Get Pending Transactions", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("all transactions in both queues should be included", async function() {
        await node.sdk.rpc.devel.stopSealing();

        const sq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;
        const tx = await node.sendPayTx({ seq: sq + 3 });

        const {
            transactions: wholeTXs
        } = await node.sdk.rpc.sendRpcRequest(
            "mempool_getPendingTransactions",
            [null, null, true]
        );

        expect(wholeTXs[0].sig).to.equal(tx.toJSON().sig);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

describe("Count pending transactions", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });
    it("Counting transactions included future transactions", async function() {
        await node.sdk.rpc.devel.stopSealing();

        const sq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;
        const tx = await node.sendPayTx({ seq: sq + 3 });
        const count = await node.sdk.rpc.sendRpcRequest(
            "mempool_getPendingTransactionsCount",
            [null, null, true]
        );

        expect(count).to.equal(1);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

describe("Timelock", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            argv: ["--force-sealing", "--no-reseal-timer"]
        });
        await node.start();
    });

    async function checkTx(txhash: H256, shouldBeConfirmed: boolean) {
        const results = await node.sdk.rpc.chain.getTransactionResultsByTracker(
            txhash
        );
        expect(results).deep.equal(shouldBeConfirmed ? [true] : []);
    }

    async function sendTransferTx(
        asset: Asset,
        timelock?: Timelock,
        options: {
            fee?: number;
        } = {}
    ): Promise<H256> {
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addInputs(
            timelock
                ? asset.createTransferInput({
                      timelock
                  })
                : asset.createTransferInput()
        );
        tx.addOutputs({
            quantity: 1,
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransactionInput(tx, 0);
        const { fee } = options;
        await node.sendAssetTransaction(tx, { fee });
        return tx.tracker();
    }

    describe("The current items should move to the future queue", async function() {
        it("Minted at block 1, send transfer without timelock and then replace it with Timelock::Block(3)", async function() {
            const asset = await node.mintAsset({ supply: 1 });
            await node.sdk.rpc.devel.stopSealing();
            const txhash1 = await sendTransferTx(asset, undefined);
            const txhash2 = await sendTransferTx(
                asset,
                {
                    type: "block",
                    value: 3
                },
                {
                    fee: 20
                }
            );
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});
