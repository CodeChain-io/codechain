// Copyright 2018 Kodebox, Inc.
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

import CodeChain from "../helper/spawn";
import { wait } from "../helper/promise";

import "mocha";
import { expect } from "chai";

describe("sync 2 nodes", function() {
    const BASE = 600;
    let nodeA: CodeChain;
    let nodeB: CodeChain;

    describe("2 nodes", function() {
        beforeEach(async function() {
            nodeA = new CodeChain({ base: BASE });
            nodeB = new CodeChain({ base: BASE });

            await Promise.all([nodeA.start(), nodeB.start()]);
        });

        describe("A-B connected", function() {
            beforeEach(async function() {
                await nodeA.connect(nodeB);
            });

            it("It should be synced when nodeA created a block", async function() {
                expect(
                    await nodeA.sdk.rpc.network.isConnected(
                        "127.0.0.1",
                        nodeB.port
                    )
                ).to.be.true;
                const transaction = await nodeA.sendPayTx({
                    awaitInvoice: true
                });
                await nodeB.waitBlockNumberSync(nodeA);
                expect(await nodeB.getBestBlockHash()).to.deep.equal(
                    transaction.blockHash
                );
            }).timeout(10_000);

            describe("A-B diverged", function() {
                beforeEach(async function() {
                    await nodeA.sendPayTx();
                    await nodeB.sendPayTx();
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                });

                it("It should be synced when nodeA becomes ahead", async function() {
                    await nodeA.sendPayTx();
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeA.getBestBlockHash()).to.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                }).timeout(10_000);
            });
        });

        describe("nodeA becomes ahead", function() {
            beforeEach(async function() {
                await nodeA.sendPayTx();
            });

            it("It should be synced when A-B connected", async function() {
                await nodeA.connect(nodeB);
                await nodeB.waitBlockNumberSync(nodeA);
                expect(await nodeA.getBestBlockHash()).to.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            }).timeout(10_000);
        });

        describe("A-B diverged", function() {
            beforeEach(async function() {
                await nodeA.sendPayTx();
                await nodeB.sendPayTx();
                expect(await nodeA.getBestBlockNumber()).to.equal(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            });

            describe("nodeA becomes ahead", function() {
                beforeEach(async function() {
                    await nodeA.sendPayTx();
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                it("It should be synced when A-B connected", async function() {
                    await nodeA.connect(nodeB);
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeA.getBestBlockHash()).to.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                }).timeout(10_000);
            });
        });

        describe("A-B diverged with the same transaction", function() {
            beforeEach(async function() {
                const transactionA = await nodeA.sendPayTx({ fee: 10 });
                await wait(1000);
                const transactionB = await nodeB.sendPayTx({ fee: 10 });
                expect(transactionA.unsigned).to.deep.equal(
                    transactionB.unsigned
                );
                expect(await nodeA.getBestBlockNumber()).to.equal(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            });

            describe("nodeA becomes ahead", function() {
                beforeEach(async function() {
                    await nodeA.sendPayTx();
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                it("It should be synced when A-B connected", async function() {
                    await nodeA.connect(nodeB);
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeA.getBestBlockHash()).to.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                }).timeout(10_000);
            });
        });

        describe("A-B diverged with the same transaction", function() {
            describe("Both transaction success", function() {
                beforeEach(async function() {
                    const recipient = await nodeA.createP2PKHAddress();
                    await nodeA.mintAsset({ supply: 10, recipient });
                    await nodeB.mintAsset({ supply: 10, recipient });
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                });

                describe("nodeA becomes ahead", function() {
                    beforeEach(async function() {
                        await nodeA.sendPayTx();
                        expect(await nodeA.getBestBlockNumber()).to.equal(
                            (await nodeB.getBestBlockNumber()) + 1
                        );
                    });

                    it("It should be synced when A-B connected", async function() {
                        await nodeA.connect(nodeB);
                        await nodeB.waitBlockNumberSync(nodeA);
                        expect(await nodeA.getBestBlockHash()).to.deep.equal(
                            await nodeB.getBestBlockHash()
                        );
                    }).timeout(10_000);
                });
            });

            describe("One fails", function() {
                let tx1: any;
                let tx2: any;
                beforeEach(async function() {
                    const recipient1 = await nodeA.createP2PKHAddress();
                    const recipient2 = await nodeA.createP2PKHAddress();
                    const { asset: assetA } = await nodeA.mintAsset({
                        supply: 100,
                        recipient: recipient1
                    });
                    const { asset: assetB } = await nodeB.mintAsset({
                        supply: 100,
                        recipient: recipient1
                    });

                    expect(assetA).to.deep.equal(assetB);
                    const asset = assetA;

                    tx1 = nodeA.sdk.core.createTransferAssetTransaction();
                    tx1.addInputs(asset);
                    tx1.addOutputs(
                        {
                            assetType: asset.assetType,
                            recipient: recipient2,
                            quantity: 10
                        },
                        {
                            assetType: asset.assetType,
                            recipient: recipient1,
                            quantity: 90
                        }
                    );

                    await nodeA.signTransactionInput(tx1, 0);
                    const invoices1 = await nodeA.sendAssetTransaction(tx1);
                    expect(invoices1!.length).to.equal(1);
                    expect(invoices1![0].success).to.be.true;

                    tx2 = nodeA.sdk.core.createTransferAssetTransaction();
                    tx2.addInputs(asset);
                    tx2.addOutputs({
                        assetType: asset.assetType,
                        recipient: recipient2,
                        quantity: 100
                    });

                    await nodeA.signTransactionInput(tx2, 0);
                    const invoicesA = await nodeA.sendAssetTransaction(tx2);
                    expect(invoicesA!.length).to.equal(1);
                    expect(invoicesA![0].success).to.be.false;

                    // FIXME
                    tx2._fee = null;
                    tx2._seq = null;
                    const invoicesB = await nodeB.sendAssetTransaction(tx2);
                    expect(invoicesB!.length).to.equal(1);
                    expect(invoicesB![0].success).to.be.true;

                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                describe("nodeA becomes ahead", function() {
                    it("It should be synced when A-B connected", async function() {
                        await nodeA.connect(nodeB);
                        await nodeB.waitBlockNumberSync(nodeA);

                        expect(await nodeA.getBestBlockHash()).to.deep.equal(
                            await nodeB.getBestBlockHash()
                        );
                        const invoicesA = await nodeA.sdk.rpc.chain.getInvoicesByTracker(
                            tx2.tracker()
                        );
                        expect(invoicesA!.length).to.equal(1);
                        expect(invoicesA![0].success).to.be.false;

                        const invoicesB = await nodeB.sdk.rpc.chain.getInvoicesByTracker(
                            tx2.tracker()
                        );
                        expect(invoicesB!.length).to.equal(1);
                        expect(invoicesB![0].success).to.be.false;
                    }).timeout(30_000);
                });

                describe("nodeB becomes ahead", function() {
                    beforeEach(async function() {
                        await nodeB.sendPayTx();
                        await nodeB.sendPayTx();
                        expect(await nodeB.getBestBlockNumber()).to.equal(
                            (await nodeA.getBestBlockNumber()) + 1
                        );
                    });

                    it("It should be synced when A-B connected", async function() {
                        await nodeA.connect(nodeB);
                        await nodeB.waitBlockNumberSync(nodeA);
                        expect(await nodeA.getBestBlockHash()).to.deep.equal(
                            await nodeB.getBestBlockHash()
                        );

                        const invoicesA = await nodeA.sdk.rpc.chain.getInvoicesByTracker(
                            tx2.tracker()
                        );
                        expect(invoicesA!.length).to.equal(1);
                        expect(invoicesA![0].success).to.be.true;

                        const invoicesB = await nodeB.sdk.rpc.chain.getInvoicesByTracker(
                            tx2.tracker()
                        );
                        expect(invoicesB!.length).to.equal(1);
                        expect(invoicesB![0].success).to.be.true;
                    }).timeout(30_000);
                });
            });
        });
    });

    describe("with no transaction relay", function() {
        const testSize: number = 5;

        beforeEach(async function() {
            nodeA = new CodeChain({ base: BASE });
            nodeB = new CodeChain({ base: BASE });

            await Promise.all([
                nodeA.start(["--no-tx-relay"]),
                nodeB.start(["--no-tx-relay"])
            ]);
            await nodeA.connect(nodeB);

            await Promise.all([
                nodeA.sdk.rpc.devel.stopSealing(),
                nodeB.sdk.rpc.devel.stopSealing()
            ]);
        });

        it("transactions must not be propagated", async function() {
            for (let i = 0; i < testSize; i++) {
                await nodeA.sendPayTx({
                    seq: i,
                    awaitInvoice: false
                });
                expect(
                    (await nodeA.sdk.rpc.chain.getPendingTransactions()).length
                ).to.equal(i + 1);
            }
            await wait(2000);
            expect(
                (await nodeB.sdk.rpc.chain.getPendingTransactions()).length
            ).to.equal(0);
        }).timeout(500 * testSize + 4000);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodeA.testFailed(this.currentTest!.fullTitle());
            nodeB.testFailed(this.currentTest!.fullTitle());
        }
        await Promise.all([nodeA.clean(), nodeB.clean()]);
    });
});
