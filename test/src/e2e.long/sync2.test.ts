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
import "mocha";
import { wait } from "../helper/promise";
import CodeChain from "../helper/spawn";

describe("sync 2 nodes", function() {
    let nodeA: CodeChain;
    let nodeB: CodeChain;

    describe("2 nodes", function() {
        beforeEach(async function() {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await Promise.all([nodeA.start(), nodeB.start()]);
        });

        describe("A-B connected", function() {
            beforeEach(async function() {
                this.timeout(60_000);
                await nodeA.connect(nodeB);
            });

            it("It should be synced when nodeA created a block", async function() {
                while (
                    !(await nodeA.sdk.rpc.network.isConnected(
                        "127.0.0.1",
                        nodeB.port
                    ))
                ) {
                    await wait(500);
                }

                const blockNumber = await nodeA.sdk.rpc.chain.getBestBlockNumber();
                const transaction = await nodeA.sendPayTx();
                await nodeA.waitBlockNumber(blockNumber + 1);
                await nodeB.waitBlockNumberSync(nodeA);
                expect(
                    await nodeA.sdk.rpc.chain.getTransaction(transaction.hash())
                ).not.null;
                expect(
                    await nodeB.sdk.rpc.chain.getTransaction(transaction.hash())
                ).not.null;
            }).timeout(30_000);

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
                }).timeout(30_000);
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
                }).timeout(30_000);
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
                        this.timeout(60_000);
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
                    }).timeout(30_000);
                });
            });
        });
    });

    describe("with no transaction relay", function() {
        const testSize: number = 5;

        beforeEach(async function() {
            nodeA = new CodeChain();
            nodeB = new CodeChain();

            await Promise.all([
                nodeA.start({ argv: ["--no-tx-relay"] }),
                nodeB.start({ argv: ["--no-tx-relay"] })
            ]);
            await nodeA.connect(nodeB);

            await Promise.all([
                nodeA.sdk.rpc.devel.stopSealing(),
                nodeB.sdk.rpc.devel.stopSealing()
            ]);
        });

        it("transactions must not be propagated", async function() {
            for (let seq = 0; seq < testSize; seq++) {
                await nodeA.sendPayTx({
                    seq
                });
                expect(
                    (await nodeA.sdk.rpc.chain.getPendingTransactions())
                        .transactions.length
                ).to.equal(seq + 1);
            }
            await wait(2000);
            expect(
                (await nodeB.sdk.rpc.chain.getPendingTransactions())
                    .transactions.length
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
