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

import { wait } from "../helper/promise";
import CodeChain from "../helper/spawn";
import {
    Timelock,
    Asset,
    AssetTransferAddress
} from "codechain-sdk/lib/core/classes";
import { faucetAddress } from "../helper/constants";
import { H256 } from "codechain-primitives/lib";

import "mocha";
import { expect } from "chai";

describe("Sealing test", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("stopSealing then startSealing", async function() {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendSignedParcel({ awaitInvoice: false });
        expect(await node.getBestBlockNumber()).to.equal(0);
        await node.sdk.rpc.devel.startSealing();
        expect(await node.getBestBlockNumber()).to.equal(1);
    });

    afterEach(async function() {
        await node.clean();
    });
});

describe("Memory pool size test", function() {
    let nodeA: CodeChain;
    const sizeLimit: number = 4;

    beforeEach(async function() {
        nodeA = new CodeChain({
            argv: ["--mem-pool-size", sizeLimit.toString()]
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    it("To self", async function() {
        for (let i = 0; i < sizeLimit * 2; i++) {
            await nodeA.sendSignedParcel({ seq: i, awaitInvoice: false });
        }
        const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
        expect(pendingParcels.length).to.equal(sizeLimit * 2);
    }).timeout(10_000);

    describe("To others", async function() {
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeB = new CodeChain({
                argv: ["--mem-pool-size", sizeLimit.toString()]
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        it("More than limit", async function() {
            for (let i = 0; i < sizeLimit * 2; i++) {
                await nodeA.sendSignedParcel({
                    seq: i,
                    awaitInvoice: false
                });
            }

            let counter = 0;
            while (
                (await nodeB.sdk.rpc.chain.getPendingParcels()).length <
                sizeLimit
            ) {
                await wait(500);
                counter += 1;
            }
            await wait(500 * (counter + 1));

            const pendingParcels = await nodeB.sdk.rpc.chain.getPendingParcels();
            expect(
                (await nodeB.sdk.rpc.chain.getPendingParcels()).length
            ).to.equal(sizeLimit);
        }).timeout(20_000);

        afterEach(async function() {
            await nodeB.clean();
        });
    });

    afterEach(async function() {
        await nodeA.clean();
    });
});

describe("Memory pool memory limit test", function() {
    let nodeA: CodeChain;
    const memoryLimit: number = 1;
    const mintSize: number = 5000;
    const sizeLimit: number = 5;

    beforeEach(async function() {
        nodeA = new CodeChain({
            chain: `${__dirname}/../scheme/mempool.json`,
            argv: ["--mem-pool-mem-limit", memoryLimit.toString()]
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    it("To self", async function() {
        for (let i = 0; i < sizeLimit; i++) {
            await nodeA.mintAsset({ amount: 1, seq: i, awaitMint: false });
        }
        const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
        expect(pendingParcels.length).to.equal(sizeLimit);
    }).timeout(50_000);

    describe("To others", async function() {
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeB = new CodeChain({
                chain: `${__dirname}/../scheme/mempool.json`,
                argv: ["--mem-pool-mem-limit", memoryLimit.toString()]
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        it("More than limit", async function() {
            const aBlockNumber = await nodeA.sdk.rpc.chain.getBestBlockNumber();
            const bBlockNumber = await nodeB.sdk.rpc.chain.getBestBlockNumber();
            expect(aBlockNumber).to.equal(bBlockNumber);
            const metadata = "Very large parcel" + " ".repeat(1 * 1024 * 1024);
            for (let i = 0; i < sizeLimit; i++) {
                await nodeA.mintAsset({
                    amount: mintSize,
                    seq: i,
                    metadata,
                    awaitMint: false
                });
            }
            await wait(3_000);

            const pendingParcels = await nodeB.sdk.rpc.chain.getPendingParcels();
            expect(pendingParcels.length).to.equal(0);
            expect(await nodeA.sdk.rpc.chain.getBestBlockNumber()).to.equal(
                aBlockNumber
            );
            expect(await nodeB.sdk.rpc.chain.getBestBlockNumber()).to.equal(
                bBlockNumber
            );
        }).timeout(50_000);

        afterEach(async function() {
            await nodeB.clean();
        });
    });

    afterEach(async function() {
        await nodeA.clean();
    });
});

describe("Future queue", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("all pending parcel must be mined", async function() {
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress)) || 0;

        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 3 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 2 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq + 1 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).to.equal(
            seq + 4
        );
    });

    afterEach(async function() {
        await node.clean();
    });
});

describe("Timelock", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            argv: ["--force-sealing"]
        });
        await node.start();
    });

    async function sendTxWithTimelock(timelock: Timelock): Promise<H256> {
        const { asset } = await node.mintAsset({ amount: 1 });
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(
            asset.createTransferInput({
                timelock
            })
        );
        tx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransferInput(tx, 0);
        await node.sendTransaction(tx, { awaitInvoice: false });
        return tx.hash();
    }

    async function checkTx(txhash: H256, shouldBeConfirmed: boolean) {
        const invoices = await node.sdk.rpc.chain.getTransactionInvoices(
            txhash
        );
        if (shouldBeConfirmed) {
            expect(invoices).to.deep.equal([
                { success: true, error: undefined }
            ]);
        } else {
            expect(invoices.length).to.equal(0);
        }
    }

    describe("Parcel should go into the current queue", async function() {
        [1, 2].forEach(function(target) {
            it(`Minted at block 1, send transfer with Timelock::Block(${target})`, async function() {
                const txhash = await sendTxWithTimelock({
                    type: "block",
                    value: target
                });
                await checkTx(txhash, true);
            });
        });

        [0, 1].forEach(function(target) {
            it(`Minted at block 1, send transfer with Timelock::BlockAge(${target})`, async function() {
                const txhash = await sendTxWithTimelock({
                    type: "blockAge",
                    value: target
                });
                await checkTx(txhash, true);
            });
        });

        it("send transfer with Timelock::Time(0)", async function() {
            const txhash = await sendTxWithTimelock({
                type: "time",
                value: 0
            });
            await checkTx(txhash, true);
        });

        it("send transfer with Timelock::TimeAge(0)", async function() {
            const txhash = await sendTxWithTimelock({
                type: "timeAge",
                value: 0
            });
            await checkTx(txhash, true);
        });
    });

    it("A relative timelock for failed transaction's output", async function() {
        const { asset } = await node.mintAsset({ amount: 1 });
        const failedTx = node.sdk.core.createAssetTransferTransaction();
        failedTx.addInputs(asset);
        failedTx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        const invoices1 = await node.sendTransaction(failedTx);
        expect(invoices1!.length).to.equal(1);
        expect(invoices1![0].success).to.be.false;

        const output0 = failedTx.getTransferredAsset(0);
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(
            output0.createTransferInput({
                timelock: {
                    type: "blockAge",
                    value: 2
                }
            })
        );
        tx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransferInput(tx, 0);
        try {
            await node.sendTransaction(tx, { awaitInvoice: false });
            expect.fail();
        } catch (e) {
            expect(e.data).to.have.string("Timelocked");
            expect(e.data).to.have.string("BlockAge(2)");
            expect(e.data).to.have.string("18446744073709551615");
        }
        await checkTx(tx.hash(), false);
        await node.sdk.rpc.devel.startSealing();
    });

    describe("Parcels should go into the future queue and then move to current", async function() {
        it("Minted at block 1, send transfer with Timelock::Block(3)", async function() {
            const txhash = await sendTxWithTimelock({
                // available from block 3
                type: "block",
                value: 3
            });

            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(txhash, false);

            await node.sdk.rpc.devel.startSealing();

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(txhash, true);
        });

        it("Minted at block 1, send transfer with Timelock::BlockAge(3)", async function() {
            const txhash = await sendTxWithTimelock({
                // available from block 4, since mintTx is at block 1.
                type: "blockAge",
                value: 3
            });

            for (let i = 2; i <= 3; i++) {
                expect(await node.getBestBlockNumber()).to.equal(i);
                await checkTx(txhash, false);

                await node.sdk.rpc.devel.startSealing();
            }

            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(txhash, true);
        });
    });

    async function sendTransferTx(
        asset: Asset,
        timelock?: Timelock,
        options: {
            fee?: number;
        } = {}
    ): Promise<H256> {
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(
            timelock
                ? asset.createTransferInput({
                      timelock
                  })
                : asset.createTransferInput()
        );
        tx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        await node.signTransferInput(tx, 0);
        const { fee } = options;
        await node.sendTransaction(tx, { awaitInvoice: false, fee });
        return tx.hash();
    }

    describe("The current items should move to the future queue", async function() {
        it("Minted at block 1, send transfer without timelock and then replace it with Timelock::Block(3)", async function() {
            const { asset } = await node.mintAsset({ amount: 1 });
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
            expect(await node.getBestBlockNumber()).to.equal(2);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    describe("The future items should move to the current queue", async function() {
        it("Minted at block 1, send transfer with Timelock::Block(10) and then replace it with no timelock", async function() {
            const { asset } = await node.mintAsset({ amount: 1 });
            await node.sdk.rpc.devel.stopSealing();
            const txhash1 = await sendTransferTx(asset, {
                type: "block",
                value: 10
            });
            const txhash2 = await sendTransferTx(asset, undefined, {
                fee: 20
            });
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(2);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    describe("Multiple timelocks", async function() {
        let recipient: AssetTransferAddress;

        beforeEach(async function() {
            recipient = await node.createP2PKHAddress();
        });

        async function createUTXOs(count: number): Promise<Asset[]> {
            const { asset } = await node.mintAsset({ amount: count });
            const transferTx = node.sdk.core.createAssetTransferTransaction();
            transferTx.addInputs(asset);
            transferTx.addOutputs(
                Array.from(Array(count)).map(_ => ({
                    assetType: asset.assetType,
                    amount: 1,
                    recipient
                }))
            );
            await node.signTransferInput(transferTx, 0);
            await node.sendTransaction(transferTx);
            return transferTx.getTransferredAssets();
        }

        it("2 inputs [Block(4), Block(6)] => Block(6)", async function() {
            const assets = await createUTXOs(2);
            const { assetType } = assets[0];
            const tx = node.sdk.core.createAssetTransferTransaction();
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
            tx.addOutputs({ amount: 2, recipient, assetType });
            await node.signTransferInput(tx, 0);
            await node.signTransferInput(tx, 1);
            await node.sendTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.hash(), true);
        }).timeout(10_000);

        it("2 inputs [Block(6), Block(4)] => Block(4)", async function() {
            const assets = await createUTXOs(2);
            const { assetType } = assets[0];
            const tx = node.sdk.core.createAssetTransferTransaction();
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
            tx.addOutputs({ amount: 2, recipient, assetType });
            await node.signTransferInput(tx, 0);
            await node.signTransferInput(tx, 1);
            await node.sendTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.hash(), true);
        }).timeout(10_000);

        it("2 inputs [Time(0), Block(4)] => Block(4)", async function() {
            const assets = await createUTXOs(2);
            const { assetType } = assets[0];
            const tx = node.sdk.core.createAssetTransferTransaction();
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
            tx.addOutputs({ amount: 2, recipient, assetType });
            await node.signTransferInput(tx, 0);
            await node.signTransferInput(tx, 1);
            await node.sendTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.hash(), true);
        }).timeout(10_000);

        it("2 inputs [Time(now + 3 seconds), Block(4)] => Time(..)", async function() {
            const assets = await createUTXOs(2);
            const { assetType } = assets[0];
            const tx = node.sdk.core.createAssetTransferTransaction();
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
            tx.addOutputs({ amount: 2, recipient, assetType });
            await node.signTransferInput(tx, 0);
            await node.signTransferInput(tx, 1);
            await node.sendTransaction(tx, { awaitInvoice: false });

            expect(await node.getBestBlockNumber()).to.equal(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(4);
            await checkTx(tx.hash(), false);

            await wait(3000);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            expect(await node.getBestBlockNumber()).to.equal(6);
            await checkTx(tx.hash(), true);
        }).timeout(10_000);
    });

    afterEach(async function() {
        await node.clean();
    });
});
