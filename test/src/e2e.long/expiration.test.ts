// Copyright 2019 Kodebox, Inc.
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

import { wait } from "../helper/promise";
import CodeChain from "../helper/spawn";

import "mocha";

import { expect } from "chai";
import { H256 } from "codechain-primitives";
import { faucetAddress } from "../helper/constants";

describe("TransferAsset expiration test", function() {
    let node: CodeChain;
    const numTx = 5;

    beforeEach(async function() {
        node = new CodeChain({
            argv: ["--force-sealing"]
        });
        await node.start();
    });

    describe(`Create ${numTx} transactions each expires in 1~${numTx} sec(s) after`, async function() {
        let trackers: H256[] = [];
        let startTime: number;

        beforeEach(async function() {
            this.timeout(10_000);

            // 1. Create an asset for TransferAsset
            let assets = [];
            for (let i = 0; i < numTx; i++) {
                const { asset } = await node.mintAsset({ supply: 1 });
                assets.push(asset);
            }

            // 2. Stop sealing
            await node.sdk.rpc.devel.stopSealing();

            // 3. Send TransferAsset transactions (which should not processed)
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            startTime = Math.round(new Date().getTime() / 1000);
            for (let i = 0; i < numTx; i++) {
                const recipient = await node.createP2PKHAddress();
                const tx = node.sdk.core.createTransferAssetTransaction({
                    expiration: startTime + numTx - i
                });
                tx.addInputs(assets[i]);
                tx.addOutputs({
                    assetType: assets[i].assetType,
                    shardId: assets[i].shardId,
                    recipient,
                    quantity: 1
                });
                await node.signTransactionInput(tx, 0);
                await node.sendAssetTransaction(tx, {
                    seq: seq + i,
                    awaitInvoice: false
                });

                trackers.push(tx.tracker());
            }
        });

        it("then create block 1 sec after", async function() {
            let prevBestBlockNum = await node.getBestBlockNumber();
            await wait(1_000);
            await node.sdk.rpc.devel.startSealing();

            await node.waitBlockNumber(prevBestBlockNum + 1);
            let bestBlockNum = await node.getBestBlockNumber();
            let bestBlock = await node.sdk.rpc.chain.getBlock(bestBlockNum);
            expect(bestBlock).not.to.be.null;
            let bestBlockTimestamp = bestBlock!.timestamp;

            for (let i = 0; i < numTx; i++) {
                let invoices = await node.sdk.rpc.chain.getInvoicesByTracker(
                    trackers[i]
                );
                expect(invoices).not.to.be.null;
                expect(invoices!.length).to.be.below(2);

                let IsInvoiceEmpty = invoices!.length === 0;
                let IsExpired = bestBlockTimestamp > startTime + numTx - i;
                expect(IsInvoiceEmpty).to.be.equal(IsExpired);
            }
        }).timeout(10_000);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
