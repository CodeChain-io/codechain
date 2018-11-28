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

describe("sync", function() {
    describe("2 nodes", function() {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await nodeA.start();
            await nodeB.start();
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
                const parcel = await nodeA.sendSignedParcel({
                    awaitInvoice: true
                });
                await nodeB.waitBlockNumberSync(nodeA);
                expect(await nodeB.getBestBlockHash()).to.deep.equal(
                    parcel.blockHash
                );
            }).timeout(10_000);

            describe("A-B diverged", function() {
                beforeEach(async function() {
                    await nodeA.sendSignedParcel();
                    await nodeB.sendSignedParcel();
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                });

                it("It should be synced when nodeA becomes ahead", async function() {
                    await nodeA.sendSignedParcel();
                    await nodeB.waitBlockNumberSync(nodeA);
                    expect(await nodeA.getBestBlockHash()).to.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                }).timeout(10_000);
            });
        });

        describe("nodeA becomes ahead", function() {
            beforeEach(async function() {
                await nodeA.sendSignedParcel();
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
                await nodeA.sendSignedParcel();
                await nodeB.sendSignedParcel();
                expect(await nodeA.getBestBlockNumber()).to.equal(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            });

            describe("nodeA becomes ahead", function() {
                beforeEach(async function() {
                    await nodeA.sendSignedParcel();
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

        describe("A-B diverged with the same parcel", function() {
            beforeEach(async function() {
                const parcelA = await nodeA.sendSignedParcel({ fee: 10 });
                await wait(1000);
                const parcelB = await nodeB.sendSignedParcel({ fee: 10 });
                expect(parcelA.unsigned).to.deep.equal(parcelB.unsigned);
                expect(await nodeA.getBestBlockNumber()).to.equal(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            });

            describe("nodeA becomes ahead", function() {
                beforeEach(async function() {
                    await nodeA.sendSignedParcel();
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
                    await nodeA.mintAsset({ amount: 10, recipient });
                    await nodeB.mintAsset({ amount: 10, recipient });
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                });

                describe("nodeA becomes ahead", function() {
                    beforeEach(async function() {
                        await nodeA.sendSignedParcel();
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
                        amount: 100,
                        recipient: recipient1
                    });
                    const { asset: assetB } = await nodeB.mintAsset({
                        amount: 100,
                        recipient: recipient1
                    });

                    expect(assetA).to.deep.equal(assetB);
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
                    const invoices1 = await nodeA.sendTransaction(tx1);
                    expect(invoices1!.length).to.equal(1);
                    expect(invoices1![0].success).to.be.true;

                    tx2 = nodeA.sdk.core.createAssetTransferTransaction();
                    tx2.addInputs(asset);
                    tx2.addOutputs({
                        assetType: asset.assetType,
                        recipient: recipient2,
                        amount: 100
                    });

                    await nodeA.signTransferInput(tx2, 0);
                    const invoicesA = await nodeA.sendTransaction(tx2);
                    expect(invoicesA!.length).to.equal(1);
                    expect(invoicesA![0].success).to.be.false;
                    const invoicesB = await nodeB.sendTransaction(tx2);
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
                        const invoicesA = await nodeA.sdk.rpc.chain.getTransactionInvoices(
                            tx2.hash()
                        );
                        expect(invoicesA!.length).to.equal(1);
                        expect(invoicesA![0].success).to.be.false;

                        const invoicesB = await nodeB.sdk.rpc.chain.getTransactionInvoices(
                            tx2.hash()
                        );
                        expect(invoicesB!.length).to.equal(1);
                        expect(invoicesB![0].success).to.be.false;
                    }).timeout(30_000);
                });

                describe("nodeB becomes ahead", function() {
                    beforeEach(async function() {
                        await nodeB.sendSignedParcel();
                        await nodeB.sendSignedParcel();
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

                        const invoicesA = await nodeA.sdk.rpc.chain.getTransactionInvoices(
                            tx2.hash()
                        );
                        expect(invoicesA!.length).to.equal(1);
                        expect(invoicesA![0].success).to.be.true;

                        const invoicesB = await nodeB.sdk.rpc.chain.getTransactionInvoices(
                            tx2.hash()
                        );
                        expect(invoicesB!.length).to.equal(1);
                        expect(invoicesB![0].success).to.be.true;
                    }).timeout(30_000);
                });
            });
        });

        afterEach(async function() {
            await nodeA.clean();
            await nodeB.clean();
        });
    });

    describe("2 nodes with no parcel relay", function() {
        let nodeA: CodeChain;
        let nodeB: CodeChain;
        const testSize: number = 5;

        beforeEach(async function() {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await nodeA.start(["--no-parcel-relay"]);
            await nodeB.start(["--no-parcel-relay"]);
            await nodeA.connect(nodeB);

            await nodeA.sdk.rpc.devel.stopSealing();
            await nodeB.sdk.rpc.devel.stopSealing();
        });

        it("parcels must not be propagated", async function() {
            for (let i = 0; i < testSize; i++) {
                await nodeA.sendSignedParcel({
                    seq: i,
                    awaitInvoice: false
                });
                expect(
                    (await nodeA.sdk.rpc.chain.getPendingParcels()).length
                ).to.equal(i + 1);
            }
            await wait(2000);
            expect(
                (await nodeB.sdk.rpc.chain.getPendingParcels()).length
            ).to.equal(0);
        }).timeout(500 * testSize + 4000);

        afterEach(async function() {
            await nodeA.clean();
            await nodeB.clean();
        });
    });

    [3, 5].forEach(function(numNodes) {
        describe(`${numNodes} nodes`, function() {
            let nodes: CodeChain[] = [];

            beforeEach(async function() {
                this.timeout(5000 + 1500 * numNodes);

                for (let i = 0; i < numNodes; i++) {
                    const node = new CodeChain({ argv: ["--no-discovery"] });
                    nodes.push(node);
                    await node.start();
                }
            });

            describe("Connected in a line", function() {
                describe("All connected", function() {
                    beforeEach(async function() {
                        this.timeout(5000 + 1500 * numNodes);

                        const connects = [];
                        for (let i = 0; i < numNodes - 1; i++) {
                            connects.push(nodes[i].connect(nodes[i + 1]));
                        }
                        await Promise.all(connects);
                    });

                    it("It should be synced when the first node created a block", async function() {
                        const parcel = await nodes[0].sendSignedParcel({
                            awaitInvoice: true
                        });
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                            expect(
                                await nodes[i].getBestBlockHash()
                            ).to.deep.equal(parcel.blockHash);
                        }
                    }).timeout(5000 + 1500 * numNodes);

                    describe("All diverged by both end nodes", function() {
                        beforeEach(async function() {
                            const nodeA = nodes[0],
                                nodeB = nodes[numNodes - 1];
                            await nodeA.sendSignedParcel();
                            await nodeB.sendSignedParcel();
                            expect(await nodeA.getBestBlockNumber()).to.equal(
                                await nodeB.getBestBlockNumber()
                            );
                            expect(
                                await nodeA.getBestBlockHash()
                            ).to.not.deep.equal(await nodeB.getBestBlockHash());
                        });

                        it("Every node should be synced to one", async function() {
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[i].waitBlockNumberSync(nodes[0]);
                            }
                        }).timeout(5000 + 1500 * numNodes);

                        it("It should be synced when the first node becomes ahead", async function() {
                            await nodes[0].sendSignedParcel();
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[i].waitBlockNumberSync(
                                    nodes[i - 1]
                                );
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).to.deep.equal(
                                    await nodes[0].getBestBlockHash()
                                );
                            }
                        }).timeout(5000 + 1500 * numNodes);
                    });
                });

                describe("the first node becomes ahead", function() {
                    beforeEach(async function() {
                        await nodes[0].sendSignedParcel();
                    });

                    it("It should be synced when every node connected", async function() {
                        for (let i = 0; i < numNodes - 1; i++) {
                            await nodes[i].connect(nodes[i + 1]);
                            await nodes[i + 1].waitBlockNumberSync(nodes[i]);
                            expect(
                                await nodes[i].getBestBlockHash()
                            ).to.deep.equal(
                                await nodes[i + 1].getBestBlockHash()
                            );
                        }
                    }).timeout(5000 + 3000 * numNodes);
                });
            });

            describe("Connected in a circle", function() {
                const numHalf: number = Math.floor(numNodes / 2);

                beforeEach(async function() {
                    this.timeout(5000 + 1500 * numNodes);

                    const connects = [];
                    for (let i = 0; i < numNodes; i++) {
                        connects.push(
                            nodes[i].connect(nodes[(i + 1) % numNodes])
                        );
                    }
                    await Promise.all(connects);
                });

                it("It should be synced when the first node created a block", async function() {
                    const parcel = await nodes[0].sendSignedParcel();
                    for (let i = 1; i <= numHalf; i++) {
                        await nodes[0].waitBlockNumberSync(nodes[i]);
                        expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                            parcel.blockHash
                        );

                        await nodes[0].waitBlockNumberSync(
                            nodes[numNodes - i - 1]
                        );
                        expect(
                            await nodes[numNodes - i - 1].getBestBlockHash()
                        ).to.deep.equal(parcel.blockHash);
                    }
                }).timeout(5000 + 1500 * numNodes);

                describe("All diverged by two nodes in the opposite", function() {
                    beforeEach(async function() {
                        const nodeA = nodes[0],
                            nodeB = nodes[numHalf];
                        await nodeA.sendSignedParcel();
                        await nodeB.sendSignedParcel();
                        expect(await nodeA.getBestBlockNumber()).to.equal(
                            await nodeB.getBestBlockNumber()
                        );
                        expect(
                            await nodeA.getBestBlockHash()
                        ).to.not.deep.equal(await nodeB.getBestBlockHash());
                    });

                    it("Every node should be synced", async function() {
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[0]);
                        }
                    }).timeout(5000 + 1500 * numNodes);

                    it("It should be synced when the first node becomes ahead", async function() {
                        await nodes[0].sendSignedParcel();
                        for (let i = 1; i < numNodes; i++) {
                            await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                            expect(
                                await nodes[i].getBestBlockHash()
                            ).to.deep.equal(await nodes[0].getBestBlockHash());
                        }
                    }).timeout(5000 + 1500 * numNodes);
                });
            });

            if (numNodes > 3) {
                describe("Connected in a star", function() {
                    describe("All connected", function() {
                        beforeEach(async function() {
                            this.timeout(5000 + 1500 * numNodes);

                            let connects = [];
                            for (let i = 1; i < numNodes; i++) {
                                connects.push(nodes[0].connect(nodes[i]));
                            }
                            await Promise.all(connects);
                        });

                        it("It should be synced when the center node created a block", async function() {
                            const parcel = await nodes[0].sendSignedParcel();
                            for (let i = 1; i < numNodes; i++) {
                                await nodes[0].waitBlockNumberSync(nodes[i]);
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).to.deep.equal(parcel.blockHash);
                            }
                        }).timeout(5000 + 1500 * numNodes);

                        it("It should be synced when one of the outside node created a block", async function() {
                            const parcel = await nodes[
                                numNodes - 1
                            ].sendSignedParcel();
                            for (let i = 0; i < numNodes - 1; i++) {
                                await nodes[numNodes - 1].waitBlockNumberSync(
                                    nodes[i]
                                );
                                expect(
                                    await nodes[i].getBestBlockHash()
                                ).to.deep.equal(parcel.blockHash);
                            }
                        }).timeout(5000 + 1500 * numNodes);
                    });
                });
            }

            afterEach(async function() {
                this.timeout(5000 + 1500 * numNodes);

                await Promise.all(nodes.map(n => n.clean()));
                nodes = [];
            });
        });
    });
});
