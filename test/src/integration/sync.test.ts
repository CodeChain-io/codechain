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

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

// FIXME: It fails due to timeout when the block sync extension is stuck. See
// https://github.com/CodeChain-io/codechain/issues/662
describeSkippedInTravis("sync", () => {
    describe("2 nodes", () => {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await nodeA.start();
            await nodeB.start();
        });

        describe("A-B connected", () => {
            beforeEach(async () => {
                await nodeA.connect(nodeB);
            });

            test(
                "It should be synced when nodeA created a block",
                async () => {
                    expect(
                        await nodeA.sdk.rpc.network.isConnected(
                            "127.0.0.1",
                            nodeB.port
                        )
                    ).toBe(true);
                    const parcel = await nodeA.sendSignedParcel({
                        awaitInvoice: true
                    });
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeB.getBestBlockHash()).toEqual(
                        parcel.blockHash
                    );
                },
                10000
            );

            describe("A-B diverged", () => {
                beforeEach(async () => {
                    await nodeA.sendSignedParcel();
                    await nodeB.sendSignedParcel();
                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).not.toEqual(
                        await nodeB.getBestBlockHash()
                    );
                });

                test(
                    "It should be synced when nodeA becomes ahead",
                    async () => {
                        await nodeA.sendSignedParcel();
                        await nodeB.waitBlockNumberSync(nodeA);
                        expect(await nodeA.getBestBlockHash()).toEqual(
                            await nodeB.getBestBlockHash()
                        );
                    },
                    10000
                );
            });
        });

        describe("nodeA becomes ahead", () => {
            beforeEach(async () => {
                await nodeA.sendSignedParcel();
            });

            test(
                "It should be synced when A-B connected",
                async () => {
                    await nodeA.connect(nodeB);
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeA.getBestBlockHash()).toEqual(
                        await nodeB.getBestBlockHash()
                    );
                },
                10000
            );
        });

        describe("A-B diverged", () => {
            beforeEach(async () => {
                await nodeA.sendSignedParcel();
                await nodeB.sendSignedParcel();
                expect(await nodeA.getBestBlockNumber()).toEqual(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).not.toEqual(
                    await nodeB.getBestBlockHash()
                );
            });

            describe("nodeA becomes ahead", () => {
                beforeEach(async () => {
                    await nodeA.sendSignedParcel();
                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                test(
                    "It should be synced when A-B connected",
                    async () => {
                        await nodeA.connect(nodeB);
                        await nodeB.waitBlockNumberSync(nodeA);
                        expect(await nodeA.getBestBlockHash()).toEqual(
                            await nodeB.getBestBlockHash()
                        );
                    },
                    10000
                );
            });
        });

        describe("A-B diverged with the same parcel", () => {
            beforeEach(async () => {
                const parcelA = await nodeA.sendSignedParcel({ fee: 10 });
                await wait(1000);
                const parcelB = await nodeB.sendSignedParcel({ fee: 10 });
                expect(parcelA.unsigned).toEqual(parcelB.unsigned);
                expect(await nodeA.getBestBlockNumber()).toEqual(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).not.toEqual(
                    await nodeB.getBestBlockHash()
                );
            }, 3000);

            describe("nodeA becomes ahead", () => {
                beforeEach(async () => {
                    await nodeA.sendSignedParcel();
                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                test(
                    "It should be synced when A-B connected",
                    async () => {
                        await nodeA.connect(nodeB);
                        await nodeB.waitBlockNumberSync(nodeA);
                        expect(await nodeA.getBestBlockHash()).toEqual(
                            await nodeB.getBestBlockHash()
                        );
                    },
                    10000
                );
            });
        });

        describe("A-B diverged with the same transaction", () => {
            describe("Both transaction success", () => {
                beforeEach(async () => {
                    const recipient = await nodeA.createP2PKHAddress();
                    await nodeA.mintAsset({ amount: 10, recipient });
                    await nodeB.mintAsset({ amount: 10, recipient });
                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).not.toEqual(
                        await nodeB.getBestBlockHash()
                    );
                });

                describe("nodeA becomes ahead", () => {
                    beforeEach(async () => {
                        await nodeA.sendSignedParcel();
                        expect(await nodeA.getBestBlockNumber()).toEqual(
                            (await nodeB.getBestBlockNumber()) + 1
                        );
                    });

                    test(
                        "It should be synced when A-B connected",
                        async () => {
                            await nodeA.connect(nodeB);
                            await nodeB.waitBlockNumberSync(nodeA);
                            expect(await nodeA.getBestBlockHash()).toEqual(
                                await nodeB.getBestBlockHash()
                            );
                        },
                        10000
                    );
                });
            });

            describe("One fails", () => {
                let tx1: any;
                let tx2: any;
                beforeEach(async () => {
                    const recipient1 = await nodeA.createP2PKHAddress();
                    const recipient2 = await nodeA.createP2PKHAddress();
                    const { asset: assetA } = await nodeA.mintAsset({
                        amount: 100,
                        recipient: recipient1
                    });
                    const { asset: assetB } = await nodeB.mintAsset({
                        amount: 100,
                        recipient: recipient1
                    });

                    expect(assetA).toEqual(assetB);
                    const asset = assetA;

                    tx1 = nodeA.sdk.core.createAssetTransferTransaction();
                    tx1.addInputs(asset);
                    tx1.addOutputs(
                        {
                            assetType: asset.assetType,
                            recipient: recipient2,
                            amount: 10
                        },
                        {
                            assetType: asset.assetType,
                            recipient: recipient1,
                            amount: 90
                        }
                    );

                    await nodeA.signTransferInput(tx1, 0);
                    expect((await nodeA.sendTransaction(tx1)).success).toBe(
                        true
                    );

                    tx2 = nodeA.sdk.core.createAssetTransferTransaction();
                    tx2.addInputs(asset);
                    tx2.addOutputs({
                        assetType: asset.assetType,
                        recipient: recipient2,
                        amount: 100
                    });

                    await nodeA.signTransferInput(tx2, 0);
                    expect((await nodeA.sendTransaction(tx2)).success).toBe(
                        false
                    );
                    expect((await nodeB.sendTransaction(tx2)).success).toBe(
                        true
                    );

                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        (await nodeB.getBestBlockNumber()) + 1
                    );
                });

                describe("nodeA becomes ahead", () => {
                    test(
                        "It should be synced when A-B connected",
                        async () => {
                            await nodeA.connect(nodeB);
                            await nodeB.waitBlockNumberSync(nodeA);
                            expect(await nodeA.getBestBlockHash()).toEqual(
                                await nodeB.getBestBlockHash()
                            );
                            expect(
                                (await nodeA.sdk.rpc.chain.getTransactionInvoice(
                                    tx2.hash()
                                )).success
                            ).toBe(false);
                            expect(
                                (await nodeB.sdk.rpc.chain.getTransactionInvoice(
                                    tx2.hash()
                                )).success
                            ).toBe(false);
                        },
                        30000
                    );
                });

                describe("nodeB becomes ahead", () => {
                    beforeEach(async () => {
                        await nodeB.sendSignedParcel();
                        await nodeB.sendSignedParcel();
                        expect(await nodeB.getBestBlockNumber()).toEqual(
                            (await nodeA.getBestBlockNumber()) + 1
                        );
                    });

                    test(
                        "It should be synced when A-B connected",
                        async () => {
                            await nodeA.connect(nodeB);
                            await nodeB.waitBlockNumberSync(nodeA);
                            expect(await nodeA.getBestBlockHash()).toEqual(
                                await nodeB.getBestBlockHash()
                            );

                            expect(
                                (await nodeA.sdk.rpc.chain.getTransactionInvoice(
                                    tx2.hash()
                                )).success
                            ).toBe(true);
                            expect(
                                (await nodeB.sdk.rpc.chain.getTransactionInvoice(
                                    tx2.hash()
                                )).success
                            ).toBe(true);
                        },
                        30000
                    );
                });
            });
        });

        afterEach(async () => {
            await nodeA.clean();
            await nodeB.clean();
        });
    });

    describe("2 nodes with no parcel relay", () => {
        let nodeA: CodeChain;
        let nodeB: CodeChain;
        const testSize: number = 5;

        beforeEach(async () => {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await nodeA.start(["--no-parcel-relay"]);
            await nodeB.start(["--no-parcel-relay"]);
            await nodeA.connect(nodeB);

            await nodeA.sdk.rpc.devel.stopSealing();
            await nodeB.sdk.rpc.devel.stopSealing();
        });

        test(
            "parcels must not be propagated",
            async () => {
                for (let i = 0; i < testSize; i++) {
                    await nodeA.sendSignedParcel({
                        nonce: i,
                        awaitInvoice: false
                    });
                    expect(
                        (await nodeA.sdk.rpc.chain.getPendingParcels()).length
                    ).toBe(i + 1);
                }
                await wait(2000);
                expect(
                    (await nodeB.sdk.rpc.chain.getPendingParcels()).length
                ).toBe(0);
            },
            500 * testSize + 4000
        );

        afterEach(async () => {
            await nodeA.clean();
            await nodeB.clean();
        });
    });

    describe.each([[3], [5]])(`%p nodes`, numNodes => {
        let nodes: CodeChain[] = [];

        beforeEach(async () => {
            for (let i = 0; i < numNodes; i++) {
                const node = new CodeChain({ argv: ["--no-discovery"] });
                nodes.push(node);
                await node.start();
            }
        }, 5000 + 1500 * numNodes);

        describe("Connected in a line", () => {
            describe("All connected", () => {
                beforeEach(async () => {
                    for (let i = 0; i < numNodes - 1; i++) {
                        await nodes[i].connect(nodes[i + 1]);
                    }
                }, 5000 + 1500 * numNodes);

                test(
                    "It should be synced when the first node created a block",
                    async () => {
                        const parcel = await nodes[0].sendSignedParcel({
                            awaitInvoice: true
                        });
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                            expect(await nodes[i].getBestBlockHash()).toEqual(
                                parcel.blockHash
                            );
                        }
                    },
                    5000 + 1500 * numNodes
                );

                describe("All diverged by both end nodes", () => {
                    beforeEach(async () => {
                        const nodeA = nodes[0],
                            nodeB = nodes[numNodes - 1];
                        await nodeA.sendSignedParcel();
                        await nodeB.sendSignedParcel();
                        expect(await nodeA.getBestBlockNumber()).toEqual(
                            await nodeB.getBestBlockNumber()
                        );
                        expect(await nodeA.getBestBlockHash()).not.toEqual(
                            await nodeB.getBestBlockHash()
                        );
                    });

                    test(
                        "Every node should be synced to one",
                        async () => {
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[i].waitBlockNumberSync(nodes[0]);
                            }
                        },
                        5000 + 1500 * numNodes
                    );

                    test(
                        "It should be synced when the first node becomes ahead",
                        async () => {
                            await nodes[0].sendSignedParcel();
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[i].waitBlockNumberSync(
                                    nodes[i - 1]
                                );
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).toEqual(await nodes[0].getBestBlockHash());
                            }
                        },
                        5000 + 1500 * numNodes
                    );
                });
            });

            describe("the first node becomes ahead", () => {
                beforeEach(async () => {
                    await nodes[0].sendSignedParcel();
                });

                test(
                    "It should be synced when every node connected",
                    async () => {
                        for (let i = 0; i < numNodes - 1; i++) {
                            await nodes[i].connect(nodes[i + 1]);
                            await nodes[i + 1].waitBlockNumberSync(nodes[i]);
                            expect(await nodes[i].getBestBlockHash()).toEqual(
                                await nodes[i + 1].getBestBlockHash()
                            );
                        }
                    },
                    5000 + 3000 * numNodes
                );
            });
        });

        describe("Connected in a circle", () => {
            const numHalf: number = Math.floor(numNodes / 2);

            beforeEach(async () => {
                for (let i = 0; i < numNodes; i++) {
                    nodes[i].connect(nodes[(i + 1) % numNodes]);
                }
            }, 5000 + 1500 * numNodes);

            test(
                "It should be synced when the first node created a block",
                async () => {
                    const parcel = await nodes[0].sendSignedParcel();
                    for (let i = 1; i <= numHalf; i++) {
                        await nodes[0].waitBlockNumberSync(nodes[i]);
                        expect(await nodes[i].getBestBlockHash()).toEqual(
                            parcel.blockHash
                        );

                        await nodes[0].waitBlockNumberSync(
                            nodes[numNodes - i - 1]
                        );
                        expect(
                            await nodes[numNodes - i - 1].getBestBlockHash()
                        ).toEqual(parcel.blockHash);
                    }
                },
                5000 + 1500 * numNodes
            );

            describe("All diverged by two nodes in the opposite", () => {
                beforeEach(async () => {
                    const nodeA = nodes[0],
                        nodeB = nodes[numHalf];
                    await nodeA.sendSignedParcel();
                    await nodeB.sendSignedParcel();
                    expect(await nodeA.getBestBlockNumber()).toEqual(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).not.toEqual(
                        await nodeB.getBestBlockHash()
                    );
                });

                test(
                    "Every node should be synced",
                    async () => {
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[0]);
                        }
                    },
                    5000 + 1500 * numNodes
                );

                test(
                    "It should be synced when the first node becomes ahead",
                    async () => {
                        await nodes[0].sendSignedParcel();
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                            expect(await nodes[i].getBestBlockHash()).toEqual(
                                await nodes[0].getBestBlockHash()
                            );
                        }
                    },
                    5000 + 1500 * numNodes
                );
            });
        });

        if (numNodes > 3) {
            describe("Connected in a star", () => {
                describe("All connected", () => {
                    beforeEach(async () => {
                        for (let i = 1; i < numNodes; i++) {
                            nodes[0].connect(nodes[i]);
                        }
                    }, 5000 + 1500 * numNodes);

                    test(
                        "It should be synced when the center node created a block",
                        async () => {
                            const parcel = await nodes[0].sendSignedParcel();
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[0].waitBlockNumberSync(nodes[i]);
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).toEqual(parcel.blockHash);
                            }
                        },
                        5000 + 1500 * numNodes
                    );

                    test(
                        "It should be synced when one of the outside node created a block",
                        async () => {
                            const parcel = await nodes[
                                numNodes - 1
                            ].sendSignedParcel();
                            for (let i = 0; i < numNodes - 1; i++) {
                                await nodes[numNodes - 1].waitBlockNumberSync(
                                    nodes[i]
                                );
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).toEqual(parcel.blockHash);
                            }
                        },
                        5000 + 1500 * numNodes
                    );
                });
            });
        }

        afterEach(async () => {
            await Promise.all(nodes.map(n => n.clean()));
            nodes = [];
        }, 5000 + 1500 * numNodes);
    });
});
