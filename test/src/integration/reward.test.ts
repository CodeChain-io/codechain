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
import { U256 } from "codechain-sdk/lib/core/U256";

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

describe("Block Reward", () => {
    describe("Reward = 50, 1 miner", () => {
        let node: CodeChain;
        const author = "tccqz8mtc5gr9jx92jwxf95gc3yhpv92du2mq3x4zhq";
        const authorSecret =
            "4aa026c5fecb70923a1ee2bb10bbfadb63d228f39c39fe1da2b1dee63364aff1";

        beforeEach(async () => {
            node = new CodeChain({
                chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
                argv: ["--author", author, "--force-sealing"]
            });
            await node.start();
        });

        test("Mining an empty block", async () => {
            await node.sdk.rpc.devel.startSealing();
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(50));
        });

        test("Mining a block with 1 parcel", async () => {
            await node.sendSignedParcel({ fee: 10 });
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(50 + 10));
        });

        test("Mining a block with 3 parcels", async () => {
            await node.sdk.rpc.devel.stopSealing();
            await node.sendSignedParcel({
                fee: 10,
                nonce: 0,
                awaitInvoice: false
            });
            await node.sendSignedParcel({
                fee: 10,
                nonce: 1,
                awaitInvoice: false
            });
            await node.sendSignedParcel({
                fee: 15,
                nonce: 2,
                awaitInvoice: false
            });
            await node.sdk.rpc.devel.startSealing();
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(50 + 35));
        });

        test("Mining a block with a parcel that pays the author", async () => {
            await node.payment(author, 100);
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(50 + 10 + 100));
        });

        test("Mining a block with a parcel which author pays someone in", async () => {
            await node.sendSignedParcel({ fee: 10 }); // +60
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(60));

            const parcel = await node.sdk.core
                .createPaymentParcel({
                    recipient: "tccqzn9jjm3j6qg69smd7cn0eup4w7z2yu9my9a2k78",
                    amount: 50
                })
                .sign({ secret: authorSecret, nonce: 0, fee: 10 }); // -60
            await node.sdk.rpc.chain.sendSignedParcel(parcel); // +60
            await expect(
                node.sdk.rpc.chain.getBalance(author)
            ).resolves.toEqual(new U256(60));
        });

        afterEach(async () => {
            await node.clean();
        });
    });

    // FIXME: It fails due to timeout when the block sync extension is stuck.
    // See https://github.com/CodeChain-io/codechain/issues/662
    describeSkippedInTravis("Reward = 50, 2 miners", () => {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        const authorA = "tccqz8mtc5gr9jx92jwxf95gc3yhpv92du2mq3x4zhq";
        const authorASecret =
            "4aa026c5fecb70923a1ee2bb10bbfadb63d228f39c39fe1da2b1dee63364aff1";

        const authorB = "tccqzw22ugf6lkxs2enrm2tfqfc24ltk7lk2c7tw9j4";
        const authorBSecret =
            "91580d24073185b91904514c23663b1180090cbeefc24b3d2e2ab1ba229e2620";

        beforeEach(async () => {
            nodeA = new CodeChain({
                chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
                argv: ["--author", authorA, "--force-sealing"],
                logFlag: true
            });
            nodeB = new CodeChain({
                chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
                argv: ["--author", authorB, "--force-sealing"],
                logFlag: true
            });

            await Promise.all([nodeA.start(), nodeB.start()]);
        });

        test("authorA creates an empty block", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            await expect(
                nodeA.sdk.rpc.chain.getBalance(authorA)
            ).resolves.toEqual(new U256(50));

            await nodeB.connect(nodeA);
            await nodeB.waitBlockNumberSync(nodeA);

            await expect(
                nodeB.sdk.rpc.chain.getBalance(authorA)
            ).resolves.toEqual(new U256(50));
        });

        test("authorA creates one block and authorB creates two blocks in parallel. And then, sync", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            await expect(
                nodeA.sdk.rpc.chain.getBalance(authorA)
            ).resolves.toEqual(new U256(50));

            await nodeB.sdk.rpc.devel.startSealing();
            await nodeB.sdk.rpc.devel.startSealing();
            await expect(
                nodeB.sdk.rpc.chain.getBalance(authorB)
            ).resolves.toEqual(new U256(100));

            await nodeA.connect(nodeB);
            await nodeA.waitBlockNumberSync(nodeB);

            await expect(
                nodeA.sdk.rpc.chain.getBalance(authorA)
            ).resolves.toEqual(new U256(0));
            await expect(
                nodeA.sdk.rpc.chain.getBalance(authorB)
            ).resolves.toEqual(new U256(100));
        });

        test(
            "A reorganization of block rewards and payments",
            async () => {
                // nodeA creates a block
                {
                    await nodeA.sdk.rpc.devel.startSealing(); // +50 for authorA
                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(50));
                }

                // sync and disconnect
                {
                    await nodeB.connect(nodeA);
                    await nodeB.waitBlockNumberSync(nodeA);

                    await expect(
                        nodeB.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(50));

                    await nodeB.disconnect(nodeA);
                }

                // nodeA creates 2 blocks
                {
                    await nodeA.payment(authorA, 100); // +160 for authorA in nodeA
                    await nodeA.sdk.rpc.chain.sendSignedParcel(
                        nodeA.sdk.core
                            .createPaymentParcel({
                                recipient: authorB,
                                amount: 5
                            })
                            .sign({
                                secret: authorASecret,
                                fee: 10,
                                nonce: 0
                            })
                    ); // +45 for authorA, +5 for authorB in nodeA
                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(255));
                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorB)
                    ).resolves.toEqual(new U256(5));
                }

                // nodeB creates 3 blocks
                {
                    await nodeB.payment(authorA, 200); // +200 for authorA, +60 for authorB in nodeB
                    await nodeB.payment(authorB, 300); // +360 for authorB in nodeB
                    await nodeB.sdk.rpc.chain.sendSignedParcel(
                        nodeB.sdk.core
                            .createPaymentParcel({
                                recipient: authorB,
                                amount: 15
                            })
                            .sign({
                                secret: authorASecret,
                                fee: 10,
                                nonce: 0
                            })
                    ); // -25 for authorA. +75 for authorB in nodeB
                    await expect(
                        nodeB.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(225));
                    await expect(
                        nodeB.sdk.rpc.chain.getBalance(authorB)
                    ).resolves.toEqual(new U256(495));
                }

                // sync. nodeA now sees nodeB's state
                {
                    const nodeBBestBlockHash = await nodeB.getBestBlockHash();
                    await expect(nodeB.getBestBlockNumber()).resolves.toBe(4);

                    await nodeB.connect(nodeA);
                    await nodeA.waitBlockNumberSync(nodeB);
                    await expect(nodeA.getBestBlockHash()).resolves.toEqual(
                        nodeBBestBlockHash
                    );

                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(225));
                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorB)
                    ).resolves.toEqual(new U256(495));
                }

                // nodeA creates a block
                {
                    await nodeA.payment(authorA, 1000); // +1060 for authorA
                    await expect(
                        nodeA.sdk.rpc.chain.getBalance(authorA)
                    ).resolves.toEqual(new U256(225 + 1060));
                }
            },
            7000
        );

        afterEach(async () => {
            await Promise.all([nodeA.clean(), nodeB.clean()]);
        });
    });
});
