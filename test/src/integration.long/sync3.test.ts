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

import CodeChain from "../helper/spawn";

import "mocha";
import { expect } from "chai";

describe("sync 3 nodes", function() {
    const BASE = 650;
    const NUM_NODES = 3;
    let nodes: CodeChain[] = [];

    beforeEach(async function() {
        this.timeout(5000 + 5000 * NUM_NODES);

        for (let i = 0; i < NUM_NODES; i++) {
            const node = new CodeChain({
                argv: ["--no-discovery"],
                base: BASE
            });
            nodes.push(node);
            await node.start();
        }
    });

    describe("Connected in a line", function() {
        describe("All connected", function() {
            beforeEach(async function() {
                this.timeout(5000 + 5000 * NUM_NODES);

                const connects = [];
                for (let i = 0; i < NUM_NODES - 1; i++) {
                    connects.push(nodes[i].connect(nodes[i + 1]));
                }
                await Promise.all(connects);
            });

            it("It should be synced when the first node created a block", async function() {
                const transaction = await nodes[0].sendPayTx({
                    awaitInvoice: true
                });
                for (let i = 1; i < NUM_NODES; i++) {
                    await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                    expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                        transaction.blockHash
                    );
                }
            }).timeout(5000 + 10000 * NUM_NODES);

            describe("All diverged by both end nodes", function() {
                beforeEach(async function() {
                    const nodeA = nodes[0],
                        nodeB = nodes[NUM_NODES - 1];
                    await nodeA.sendPayTx();
                    await nodeB.sendPayTx();
                    expect(await nodeA.getBestBlockNumber()).to.equal(
                        await nodeB.getBestBlockNumber()
                    );
                    expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                        await nodeB.getBestBlockHash()
                    );
                });

                it("Every node should be synced to one", async function() {
                    const waits = [];
                    for (let i = 1; i < NUM_NODES; i++) {
                        waits.push(nodes[i].waitBlockNumberSync(nodes[0]));
                    }
                    await Promise.all(waits);
                }).timeout(5000 + 5000 * NUM_NODES);

                it("It should be synced when the first node becomes ahead", async function() {
                    await nodes[0].sendPayTx();
                    for (let i = 1; i < NUM_NODES; i++) {
                        await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                        expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                            await nodes[0].getBestBlockHash()
                        );
                    }
                }).timeout(5000 + 10000 * NUM_NODES);
            });
        });

        describe("the first node becomes ahead", function() {
            beforeEach(async function() {
                await nodes[0].sendPayTx();
            });

            it("It should be synced when every node connected", async function() {
                for (let i = 0; i < NUM_NODES - 1; i++) {
                    await nodes[i].connect(nodes[i + 1]);
                    await nodes[i + 1].waitBlockNumberSync(nodes[i]);
                    expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                        await nodes[i + 1].getBestBlockHash()
                    );
                }
            }).timeout(5000 + 15000 * NUM_NODES);
        });
    });

    describe("Connected in a circle", function() {
        const numHalf: number = Math.floor(NUM_NODES / 2);

        beforeEach(async function() {
            this.timeout(5000 + 5000 * NUM_NODES);

            const connects = [];
            for (let i = 0; i < NUM_NODES; i++) {
                connects.push(nodes[i].connect(nodes[(i + 1) % NUM_NODES]));
            }
            await Promise.all(connects);
        });

        it("It should be synced when the first node created a block", async function() {
            const transaction = await nodes[0].sendPayTx();
            for (let i = 1; i <= numHalf; i++) {
                await nodes[0].waitBlockNumberSync(nodes[i]);
                expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                    transaction.blockHash
                );

                await nodes[0].waitBlockNumberSync(nodes[NUM_NODES - i - 1]);
                expect(
                    await nodes[NUM_NODES - i - 1].getBestBlockHash()
                ).to.deep.equal(transaction.blockHash);
            }
        }).timeout(5000 + 5000 * NUM_NODES);

        describe("All diverged by two nodes in the opposite", function() {
            beforeEach(async function() {
                const nodeA = nodes[0],
                    nodeB = nodes[numHalf];
                await nodeA.sendPayTx();
                await nodeB.sendPayTx();
                expect(await nodeA.getBestBlockNumber()).to.equal(
                    await nodeB.getBestBlockNumber()
                );
                expect(await nodeA.getBestBlockHash()).to.not.deep.equal(
                    await nodeB.getBestBlockHash()
                );
            });

            it("Every node should be synced", async function() {
                const waits = [];
                for (let i = 1; i < NUM_NODES; i++) {
                    waits.push(nodes[i].waitBlockNumberSync(nodes[0]));
                }
                await Promise.all(waits);
            }).timeout(5000 + 5000 * NUM_NODES);

            it("It should be synced when the first node becomes ahead", async function() {
                await nodes[0].sendPayTx();
                for (let i = 1; i < NUM_NODES; i++) {
                    await nodes[i].waitBlockNumberSync(nodes[i - 1]);
                    expect(await nodes[i].getBestBlockHash()).to.deep.equal(
                        await nodes[0].getBestBlockHash()
                    );
                }
            }).timeout(5000 + 10000 * NUM_NODES);
        });
    });

    afterEach(async function() {
        this.timeout(5000 + 3000 * NUM_NODES);

        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }

        await Promise.all(nodes.map(node => node.clean()));
        nodes = [];
    });
});
