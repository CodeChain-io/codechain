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
import { Timelock, U256, Asset } from "codechain-sdk/lib/core/classes";
import { faucetAddress } from "../helper/constants";
import { H256 } from "codechain-primitives/lib";

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

describe("Sealing test", () => {
    let node: CodeChain;

    beforeEach(async () => {
        node = new CodeChain();
        await node.start();
    });

    test("stopSealing then startSealing", async () => {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendSignedParcel({ awaitInvoice: false });
        expect(await node.getBestBlockNumber()).toEqual(0);
        await node.sdk.rpc.devel.startSealing();
        expect(await node.getBestBlockNumber()).toEqual(1);
    });

    afterEach(async () => {
        await node.clean();
    });
});

describe("Memory pool size test", () => {
    let nodeA: CodeChain;
    const sizeLimit: number = 4;

    beforeEach(async () => {
        nodeA = new CodeChain({
            argv: ["--mem-pool-size", sizeLimit.toString()]
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    test(
        "To self",
        async () => {
            for (let i = 0; i < sizeLimit * 2; i++) {
                await nodeA.sendSignedParcel({ seq: i, awaitInvoice: false });
            }
            const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
            expect(pendingParcels.length).toEqual(sizeLimit * 2);
        },
        10000
    );

    // FIXME: It fails due to timeout when the block sync extension is stuck.
    // See https://github.com/CodeChain-io/codechain/issues/662
    describeSkippedInTravis("To others", async () => {
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeB = new CodeChain({
                argv: ["--mem-pool-size", sizeLimit.toString()]
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        test(
            "More than limit",
            async () => {
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
                ).toBe(sizeLimit);
            },
            20000
        );

        afterEach(async () => {
            await nodeB.clean();
        });
    });

    afterEach(async () => {
        await nodeA.clean();
    });
});

describe("Memory pool memory limit test", () => {
    let nodeA: CodeChain;
    const memoryLimit: number = 1;
    const mintSize: number = 5000;
    const sizeLimit: number = 5;

    beforeEach(async () => {
        nodeA = new CodeChain({
            argv: ["--mem-pool-mem-limit", memoryLimit.toString()]
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    test(
        "To self",
        async () => {
            for (let i = 0; i < sizeLimit; i++) {
                await nodeA.mintAsset({ amount: 1, seq: i, awaitMint: false });
            }
            const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
            expect(pendingParcels.length).toEqual(sizeLimit);
        },
        50000
    );

    // FIXME: It fails due to timeout when the block sync extension is stuck.
    // See https://github.com/CodeChain-io/codechain/issues/662
    describeSkippedInTravis("To others", async () => {
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeB = new CodeChain({
                argv: ["--mem-pool-mem-limit", memoryLimit.toString()],
                logFlag: true
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        test(
            "More than limit",
            async () => {
                for (let i = 0; i < sizeLimit; i++) {
                    await nodeA.mintAsset({
                        amount: mintSize,
                        seq: i,
                        awaitMint: false
                    });
                }

                for (let i = 0; i < 10; i++) {
                    const pendingParcels = await nodeB.sdk.rpc.chain.getPendingParcels();
                    expect(pendingParcels.length).toEqual(0);
                    await wait(250);
                }
            },
            50000
        );

        afterEach(async () => {
            await nodeB.clean();
        });
    });

    afterEach(async () => {
        await nodeA.clean();
    });
});

describe("Future queue", () => {
    let node: CodeChain;

    beforeEach(async () => {
        node = new CodeChain();
        await node.start();
    });

    test("all pending parcel must be mined", async () => {
        const seq =
            (await node.sdk.rpc.chain.getSeq(faucetAddress)) || U256.ensure(0);
        const seq1 = seq.increase();
        const seq2 = seq1.increase();
        const seq3 = seq2.increase();
        const seq4 = seq3.increase();

        await node.sendSignedParcel({ awaitInvoice: false, seq: seq3 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).toEqual(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq2 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).toEqual(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq1 });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).toEqual(seq);
        await node.sendSignedParcel({ awaitInvoice: false, seq: seq });
        expect(await node.sdk.rpc.chain.getSeq(faucetAddress)).toEqual(seq4);
    });

    afterEach(async () => {
        await node.clean();
    });
});

describe("Timelock", () => {
    let node: CodeChain;

    beforeEach(async () => {
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
        const invoice = await node.sdk.rpc.chain.getTransactionInvoice(txhash);
        if (shouldBeConfirmed) {
            expect(invoice).toEqual({ success: true });
        } else {
            expect(invoice).toBe(null);
        }
    }

    describe("Parcel should go into the current queue", async () => {
        test.each([[1], [2]])(
            "Minted at block 1, send transfer with Timelock::Block(%p)",
            async target => {
                const txhash = await sendTxWithTimelock({
                    type: "block",
                    value: target
                });
                await checkTx(txhash, true);
            }
        );

        test.each([[0], [1]])(
            "Minted at block 1, send transfer with Timelock::BlockAge(%p)",
            async target => {
                const txhash = await sendTxWithTimelock({
                    type: "blockAge",
                    value: target
                });
                await checkTx(txhash, true);
            }
        );

        test("send transfer with Timelock::Time(0)", async () => {
            const txhash = await sendTxWithTimelock({
                type: "time",
                value: 0
            });
            await checkTx(txhash, true);
        });

        test("send transfer with Timelock::TimeAge(0)", async () => {
            const txhash = await sendTxWithTimelock({
                type: "timeAge",
                value: 0
            });
            await checkTx(txhash, true);
        });
    });

    test("A relative timelock for failed transaction's output", async () => {
        const { asset } = await node.mintAsset({ amount: 1 });
        const failedTx = node.sdk.core.createAssetTransferTransaction();
        failedTx.addInputs(asset);
        failedTx.addOutputs({
            amount: 1,
            assetType: asset.assetType,
            recipient: await node.createP2PKHAddress()
        });
        const invoice1 = await node.sendTransaction(failedTx);
        if (invoice1 == null) {
            throw Error("Cannot get the first invoice");
        }
        expect(invoice1.success).toBe(false);

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
        await node.sendTransaction(tx, { awaitInvoice: false });
        await checkTx(tx.hash(), false);
        await node.sdk.rpc.devel.startSealing();
        const invoice2 = await node.sdk.rpc.chain.getTransactionInvoice(
            tx.hash()
        );
        if (invoice2 == null) {
            throw Error("Cannot get the second invoice");
        }
        expect(invoice2.success).toBe(false);
        expect(invoice2.error!.type).toBe("InvalidTransaction");
        expect(invoice2.error!.content.type).toBe("AssetNotFound");
    });

    describe("Parcels should go into the future queue and then move to current", async () => {
        test("Minted at block 1, send transfer with Timelock::Block(3)", async () => {
            const txhash = await sendTxWithTimelock({
                // available from block 3
                type: "block",
                value: 3
            });

            await expect(node.getBestBlockNumber()).resolves.toBe(2);
            await checkTx(txhash, false);

            await node.sdk.rpc.devel.startSealing();

            await expect(node.getBestBlockNumber()).resolves.toBe(3);
            await checkTx(txhash, true);
        });

        test("Minted at block 1, send transfer with Timelock::BlockAge(3)", async () => {
            const txhash = await sendTxWithTimelock({
                // available from block 4, since mintTx is at block 1.
                type: "blockAge",
                value: 3
            });

            for (let i = 2; i <= 3; i++) {
                await expect(node.getBestBlockNumber()).resolves.toBe(i);
                await checkTx(txhash, false);

                await node.sdk.rpc.devel.startSealing();
            }

            await expect(node.getBestBlockNumber()).resolves.toBe(4);
            await checkTx(txhash, true);
        });
    });

    async function sendTransferTx(
        asset: Asset,
        timelock: Timelock,
        options: {
            nonce?: number;
            fee?: number;
        } = {}
    ): Promise<H256> {
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
        const { nonce, fee } = options;
        await node.sendTransaction(tx, { awaitInvoice: false, nonce, fee });
        return tx.hash();
    }

    describe("The current items should move to the future queue", async () => {
        test("Minted at block 1, send transfer without timelock and then replace it with Timelock::Block(3)", async () => {
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
                    nonce: 1,
                    fee: 20
                }
            );
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(2);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(3);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    describe("The future items should move to the current queue", async () => {
        test("Minted at block 1, send transfer with Timelock::Block(10) and then replace it with no timelock", async () => {
            const { asset } = await node.mintAsset({ amount: 1 });
            await node.sdk.rpc.devel.stopSealing();
            const txhash1 = await sendTransferTx(asset, {
                type: "block",
                value: 10
            });
            const txhash2 = await sendTransferTx(asset, undefined, {
                nonce: 1,
                fee: 20
            });
            await checkTx(txhash1, false);
            await checkTx(txhash2, false);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(2);
            await checkTx(txhash1, false);
            await checkTx(txhash2, true);
        });
    });

    describe("Multiple timelocks", async () => {
        let recipient;

        beforeEach(async () => {
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

        test("2 inputs [Block(4), Block(6)] => Block(6)", async () => {
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

            await expect(node.getBestBlockNumber()).resolves.toBe(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(4);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(6);
            await checkTx(tx.hash(), true);
        });

        test("2 inputs [Block(6), Block(4)] => Block(4)", async () => {
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

            await expect(node.getBestBlockNumber()).resolves.toBe(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(4);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(6);
            await checkTx(tx.hash(), true);
        });

        test("2 inputs [Time(0), Block(4)] => Block(4)", async () => {
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

            await expect(node.getBestBlockNumber()).resolves.toBe(3);
            await checkTx(tx.hash(), false);

            await node.sdk.rpc.devel.startSealing();
            await expect(node.getBestBlockNumber()).resolves.toBe(4);
            await checkTx(tx.hash(), true);
        });

        test(
            "2 inputs [Time(now + 3 seconds), Block(4)] => Time(..)",
            async () => {
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

                await expect(node.getBestBlockNumber()).resolves.toBe(3);
                await checkTx(tx.hash(), false);

                await node.sdk.rpc.devel.startSealing();
                await expect(node.getBestBlockNumber()).resolves.toBe(4);
                await checkTx(tx.hash(), false);

                await wait(3000);

                await node.sdk.rpc.devel.startSealing();
                await node.sdk.rpc.devel.startSealing();
                await expect(node.getBestBlockNumber()).resolves.toBe(6);
                await checkTx(tx.hash(), true);
            },
            10000
        );
    });

    afterEach(async () => {
        await node.clean();
    });
});
