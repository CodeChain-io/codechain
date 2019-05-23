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
import { U64 } from "codechain-sdk/lib/core/classes";
import "mocha";
import { aliceAddress, aliceSecret, bobAddress } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("reward2", function() {
    let nodeA: CodeChain;
    let nodeB: CodeChain;

    beforeEach(async function() {
        nodeA = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
            argv: ["--author", aliceAddress.toString(), "--force-sealing"]
        });
        nodeB = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
            argv: ["--author", bobAddress.toString(), "--force-sealing"]
        });

        await Promise.all([nodeA.start(), nodeB.start()]);
    });

    it("alice creates an empty block", async function() {
        await nodeA.sdk.rpc.devel.startSealing();
        expect(
            await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
        ).to.deep.equal(new U64(50));

        await nodeB.connect(nodeA);
        await nodeB.waitBlockNumberSync(nodeA);

        expect(
            await nodeB.sdk.rpc.chain.getBalance(aliceAddress)
        ).to.deep.equal(new U64(50));
    }).timeout(30_000);

    it("alice creates one block and bob creates two blocks in parallel. And then, sync", async function() {
        await nodeA.sdk.rpc.devel.startSealing();
        expect(
            await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
        ).to.deep.equal(new U64(50));

        await nodeB.sdk.rpc.devel.startSealing();
        await nodeB.sdk.rpc.devel.startSealing();
        expect(await nodeB.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(100)
        );

        await nodeA.connect(nodeB);
        await nodeA.waitBlockNumberSync(nodeB);

        expect(
            await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
        ).to.deep.equal(new U64(0));
        expect(await nodeA.sdk.rpc.chain.getBalance(bobAddress)).to.deep.equal(
            new U64(100)
        );
    }).timeout(30_000);

    it("A reorganization of block rewards and payments", async function() {
        // nodeA creates a block
        {
            await nodeA.sdk.rpc.devel.startSealing(); // +50 for alice
            expect(
                await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50));
        }

        // sync and disconnect
        {
            await nodeB.connect(nodeA);
            await nodeB.waitBlockNumberSync(nodeA);

            expect(
                await nodeB.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50));

            await nodeB.disconnect(nodeA);
        }

        // nodeA creates 2 blocks
        {
            await nodeA.pay(aliceAddress, 100); // +100 +50 +10*4/10 for alice in nodeA, +10*3/10 for bob
            expect(
                await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 100 + 50 + 4));
            expect(
                await nodeA.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(3));
            await nodeA.sdk.rpc.chain.sendSignedTransaction(
                nodeA.sdk.core
                    .createPayTransaction({
                        recipient: bobAddress,
                        quantity: 5
                    })
                    .sign({
                        secret: aliceSecret,
                        fee: 10,
                        seq: 0
                    })
            ); // +50 -5 + 10*4/10 -10 for alice, +5 +10*3/10 for bob in nodeA

            expect(
                await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 100 + 50 + 4 + 50 - 5 + 4 - 10));
            expect(
                await nodeA.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(3 + 5 + 3));
        }

        // nodeB creates 3 blocks
        {
            await nodeB.pay(aliceAddress, 200); // +200 +10*4/10 for alice, +50 +10*3/10 for bob in nodeB
            expect(
                await nodeB.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 200 + 4));
            expect(
                await nodeB.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(50 + 3));
            await nodeB.pay(bobAddress, 300); // 10*4/10 for alice, +300 +50 +10*3/10 for bob in nodeB
            expect(
                await nodeB.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 200 + 4 + 4));
            expect(
                await nodeB.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(50 + 3 + 300 + 50 + 3));
            await nodeB.sdk.rpc.chain.sendSignedTransaction(
                nodeB.sdk.core
                    .createPayTransaction({
                        recipient: bobAddress,
                        quantity: 15
                    })
                    .sign({
                        secret: aliceSecret,
                        fee: 10,
                        seq: 0
                    })
            ); // -15 -10 +10*4/10 for alice. +50 + 15 + 10*3/10 for bob in nodeB
            expect(
                await nodeB.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 200 + 4 + 4 - 15 - 10 + 4));
            expect(
                await nodeB.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(50 + 3 + 300 + 50 + 3 + 50 + 15 + 3));
        }

        // sync. nodeA now sees nodeB's state
        {
            const nodeBBestBlockHash = await nodeB.getBestBlockHash();
            expect(await nodeB.getBestBlockNumber()).to.equal(4);

            await nodeB.connect(nodeA);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeA.getBestBlockHash()).to.deep.equal(
                nodeBBestBlockHash
            );

            expect(
                await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(new U64(50 + 200 + 4 + 4 - 15 - 10 + 4));
            expect(
                await nodeA.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(50 + 3 + 300 + 50 + 3 + 50 + 15 + 3));
        }

        // nodeA creates a block
        {
            await nodeA.pay(aliceAddress, 1000); // +1000 + 50 + 10*4/10 for alice, 10*3/10 for bob
            expect(
                await nodeA.sdk.rpc.chain.getBalance(aliceAddress)
            ).to.deep.equal(
                new U64(50 + 200 + 4 + 4 - 15 - 10 + 4 + 1000 + 50 + 4)
            );
            expect(
                await nodeA.sdk.rpc.chain.getBalance(bobAddress)
            ).to.deep.equal(new U64(50 + 3 + 300 + 50 + 3 + 50 + 15 + 3 + 3));
        }
    }).timeout(120_000);

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodeA.testFailed(this.currentTest!.fullTitle());
            nodeB.testFailed(this.currentTest!.fullTitle());
        }
        await Promise.all([nodeA.clean(), nodeB.clean()]);
    });
});
