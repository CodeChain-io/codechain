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
    AssetTransferAddress,
    Timelock
} from "codechain-sdk/lib/core/classes";
import "mocha";
import { wait } from "../helper/promise";
import CodeChain from "../helper/spawn";

const BASE = 250;

describe("Timelock", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            argv: ["--force-sealing", "--no-reseal-timer"],
            base: BASE
        });
        await node.start();
    });

    async function sendTxWithTimelock(timelock: Timelock): Promise<H256> {
        const { asset } = await node.mintAsset({ supply: 1 });
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addInputs(
            asset.createTransferInput({
                timelock
            })
        );
        tx.addOutputs({
            quantity: 1,
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransactionInput(tx, 0);
        await node.sendAssetTransaction(tx, { awaitInvoice: false });
        return tx.tracker();
    }

    async function checkTx(tracker: H256, shouldBeConfirmed: boolean) {
        const invoices = await node.sdk.rpc.chain.getInvoicesByTracker(tracker);
        if (shouldBeConfirmed) {
            expect(invoices.length).to.equal(1);
            expect(invoices[0].error).to.be.undefined;
            expect(invoices[0].success).to.be.true;
        } else {
            expect(invoices.length).to.equal(0);
        }
    }

    describe("Transaction should go into the current queue", async function() {
        [1, 2].forEach(function(target) {
            it(`Minted at block 1, send transfer with Timelock::Block(${target})`, async function() {
                const tracker = await sendTxWithTimelock({
                    type: "block",
                    value: target
                });
                await checkTx(tracker, true);
            });
        });

        [0, 1].forEach(function(target) {
            it(`Minted at block 1, send transfer with Timelock::BlockAge(${target})`, async function() {
                const tracker = await sendTxWithTimelock({
                    type: "blockAge",
                    value: target
                });
                await checkTx(tracker, true);
            });
        });

        it("send transfer with Timelock::Time(0)", async function() {
            const tracker = await sendTxWithTimelock({
                type: "time",
                value: 0
            });
            await checkTx(tracker, true);
        });

        it("send transfer with Timelock::TimeAge(0)", async function() {
            const tracker = await sendTxWithTimelock({
                type: "timeAge",
                value: 0
            });
            await checkTx(tracker, true);
        });
    });

    it("A relative timelock for failed transaction's output", async function() {
        const { asset } = await node.mintAsset({ supply: 1 });
        const failedTx = node.sdk.core.createTransferAssetTransaction();
        failedTx.addInputs(asset);
        failedTx.addOutputs({
            quantity: 1,
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHAddress()
        });
        const invoices1 = await node.sendAssetTransaction(failedTx);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0].success).to.be.false;

        const output0 = failedTx.getTransferredAsset(0);
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addInputs(
            output0.createTransferInput({
                timelock: {
                    type: "blockAge",
                    value: 2
                }
            })
        );
        tx.addOutputs({
            quantity: 1,
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransactionInput(tx, 0);
        try {
            await node.sendAssetTransaction(tx, { awaitInvoice: false });
            expect.fail();
        } catch (e) {
            expect(e.data).to.have.string("Timelocked");
            expect(e.data).to.have.string("BlockAge(2)");
            expect(e.data).to.have.string("18446744073709551615");
        }
        await checkTx(tx.tracker(), false);
        await node.sdk.rpc.devel.startSealing();
    });

    describe("Transactions should go into the future queue and then move to current", async function() {
        it("Minted at block 1, send transfer with Timelock::Block(3)", async function() {
            const tracker = await sendTxWithTimelock({
                // available from block 3
                type: "block",
                value: 3
            });

            expect(await node.getBestBlockNumber()).to.equal(1);
            await checkTx(tracker, false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(tracker, true);
        });

        it("Minted at block 1, send transfer with Timelock::BlockAge(3)", async function() {
            const tracker = await sendTxWithTimelock({
                // available from block 4, since mintTx is at block 1.
                type: "blockAge",
                value: 3
            });

            for (let i = 1; i <= 3; i++) {
                expect(await node.getBestBlockNumber()).to.equal(i);
                await checkTx(tracker, false);

                await node.sdk.rpc.devel.startSealing();
            }

            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tracker, true);
        });
    });

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
        await node.sendAssetTransaction(tx, { awaitInvoice: false, fee });
        return tx.tracker();
    }

    describe("The future items should move to the current queue", async function() {
        it("Minted at block 1, send transfer with Timelock::Block(10) and then replace it with no timelock", async function() {
            const { asset } = await node.mintAsset({ supply: 1 });
            await node.sdk.rpc.devel.stopSealing();
            const tracker1 = await sendTransferTx(asset, {
                type: "block",
                value: 10
            });
            const tracker2 = await sendTransferTx(asset, undefined, {
                fee: 20
            });
            await checkTx(tracker1, false);
            await checkTx(tracker2, false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(tracker1, false);
            await checkTx(tracker2, true);
        });
    });

    describe("Multiple timelocks", async function() {
        let recipient: AssetTransferAddress;

        beforeEach(async function() {
            recipient = await node.createP2PKHAddress();
        });

        async function createUTXOs(count: number): Promise<Asset[]> {
            const { asset } = await node.mintAsset({ supply: count });
            const transferTx = node.sdk.core.createTransferAssetTransaction();
            transferTx.addInputs(asset);
            transferTx.addOutputs(
                Array.from(Array(count)).map(_ => ({
                    assetType: asset.assetType,
                    shardId: asset.shardId,
                    quantity: 1,
                    recipient
                }))
            );
            await node.signTransactionInput(transferTx, 0);
            await node.sendAssetTransaction(transferTx);
            return transferTx.getTransferredAssets();
        }

        it("2 inputs [Block(4), Block(6)] => Block(6)", async function() {
            const assets = await createUTXOs(2);
            const { assetType, shardId } = assets[0];
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs([
                assets[0].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 4
                    }
                }),
                assets[1].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 6
                    }
                })
            ]);
            tx.addOutputs({ quantity: 2, recipient, assetType, shardId });
            await node.signTransactionInput(tx, 0);
            await node.signTransactionInput(tx, 1);
            await node.sendAssetTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.tracker(), true);
        }).timeout(10_000);

        it("2 inputs [Block(6), Block(4)] => Block(4)", async function() {
            const assets = await createUTXOs(2);
            const { assetType, shardId } = assets[0];
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs([
                assets[0].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 6
                    }
                }),
                assets[1].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 4
                    }
                })
            ]);
            tx.addOutputs({ quantity: 2, recipient, assetType, shardId });
            await node.signTransactionInput(tx, 0);
            await node.signTransactionInput(tx, 1);
            await node.sendAssetTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.tracker(), true);
        }).timeout(10_000);

        it("2 inputs [Time(0), Block(4)] => Block(4)", async function() {
            const assets = await createUTXOs(2);
            const { assetType, shardId } = assets[0];
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs([
                assets[0].createTransferInput({
                    timelock: {
                        type: "time",
                        value: 0
                    }
                }),
                assets[1].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 4
                    }
                })
            ]);
            tx.addOutputs({ quantity: 2, recipient, assetType, shardId });
            await node.signTransactionInput(tx, 0);
            await node.signTransactionInput(tx, 1);
            await node.sendAssetTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.tracker(), true);
        }).timeout(10_000);

        it("2 inputs [Time(now + 3 seconds), Block(4)] => Time(..)", async function() {
            const assets = await createUTXOs(2);
            const { assetType, shardId } = assets[0];
            const tx = node.sdk.core.createTransferAssetTransaction();
            tx.addInputs([
                assets[0].createTransferInput({
                    timelock: {
                        type: "time",
                        value: Math.ceil(Date.now() / 1000) + 3
                    }
                }),
                assets[1].createTransferInput({
                    timelock: {
                        type: "block",
                        value: 4
                    }
                })
            ]);
            tx.addOutputs({ quantity: 2, recipient, assetType, shardId });
            await node.signTransactionInput(tx, 0);
            await node.signTransactionInput(tx, 1);
            await node.sendAssetTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(tx.tracker(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.tracker(), false);

            await wait(3_000);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.tracker(), true);
        }).timeout(10_000);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
