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

describe("sync", () => {
    describe("2 nodes", () => {
        const secret =
            "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";

        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeA = new CodeChain({logFlag: true});
            nodeB = new CodeChain({logFlag: true});

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
                    console.log("nodeA: ", nodeA.logFile);
                    console.log("nodeB: ", nodeB.logFile);
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
                        console.log("nodeA: ", nodeA.logFile);
                        console.log("nodeB: ", nodeB.logFile);
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
                    console.log("nodeA: ", nodeA.logFile);
                    console.log("nodeB: ", nodeB.logFile);
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
                        console.log("nodeA: ", nodeA.logFile);
                        console.log("nodeB: ", nodeB.logFile);
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

        afterEach(async () => {
            await nodeA.clean();
            await nodeB.clean();
        });
    });

    describe.skip.each([[3], [5]])(`%p nodes`, numNodes => {
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
                            // Here is the problem
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

    // NOTE: To create empty blocks, enable --force-sealing option, and then,
    // trigger it by calling devel_startSealing RPC API.
    describe.skip("empty block", () => {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeA = new CodeChain({ argv: ["--force-sealing"] });
            nodeB = new CodeChain({ argv: ["--force-sealing"] });
            await Promise.all([nodeA.start(), nodeB.start()]);
            await nodeA.connect(nodeB);
        });

        test("nodeA creates an empty block", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            expect(await nodeA.getBestBlockNumber()).toBe(1);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).toBe(1);
            expect(await nodeA.getBestBlockHash()).toEqual(
                await nodeB.getBestBlockHash()
            );
        });

        test("nodeA creates 3 empty blocks", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            await nodeA.sdk.rpc.devel.startSealing();
            await nodeA.sdk.rpc.devel.startSealing();

            expect(await nodeA.getBestBlockNumber()).toBe(3);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).toBe(3);
            expect(await nodeA.getBestBlockHash()).toEqual(
                await nodeB.getBestBlockHash()
            );
        });

        afterEach(async () => {
            await Promise.all([nodeA.clean(), nodeB.clean()]);
        });
    });
});
