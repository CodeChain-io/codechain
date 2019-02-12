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
import { SignedTransaction } from "codechain-sdk/lib/core/classes";

const BASE = 200;

describe("Memory pool size test", function() {
    let nodeA: CodeChain;
    const sizeLimit: number = 4;

    beforeEach(async function() {
        nodeA = new CodeChain({
            argv: ["--mem-pool-size", sizeLimit.toString()],
            base: BASE
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    it("To self", async function() {
        const sending = [];
        for (let i = 0; i < sizeLimit * 2; i++) {
            sending.push(nodeA.sendPayTx({ seq: i, awaitInvoice: false }));
        }
        await Promise.all(sending);
        const pendingTransactions = await nodeA.sdk.rpc.chain.getPendingTransactions();
        expect(pendingTransactions.length).to.equal(sizeLimit * 2);
    }).timeout(10_000);

    describe("To others", async function() {
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeB = new CodeChain({
                argv: ["--mem-pool-size", sizeLimit.toString()],
                base: BASE
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        it("More than limit", async function() {
            for (let i = 0; i < sizeLimit * 2; i++) {
                await nodeA.sendPayTx({
                    seq: i,
                    awaitInvoice: false
                });
            }

            let counter = 0;
            while (
                (await nodeB.sdk.rpc.chain.getPendingTransactions()).length <
                sizeLimit
            ) {
                await wait(500);
                counter += 1;
            }
            await wait(500 * (counter + 1));

            const pendingTransactions = await nodeB.sdk.rpc.chain.getPendingTransactions();
            expect(pendingTransactions.length).to.equal(sizeLimit);
        }).timeout(20_000);

        it("Rejected by limit and reaccepted", async function() {
            const sent = [];
            for (let i = 0; i < sizeLimit * 2; i++) {
                sent.push(
                    await nodeA.sendPayTx({
                        seq: i,
                        awaitInvoice: false
                    })
                );
            }

            while (
                (await nodeB.sdk.rpc.chain.getPendingTransactions()).length <
                sizeLimit
            ) {
                await wait(500);
            }

            const pendingTransactions = await nodeB.sdk.rpc.chain.getPendingTransactions();
            const pendingTransactionHashes = pendingTransactions.map(
                (tx: SignedTransaction) => tx.hash().value
            );
            const rejectedTransactions = sent.filter(
                tx => !pendingTransactionHashes.includes(tx.hash().value)
            );

            await nodeB.sdk.rpc.devel.startSealing();

            while (
                (await nodeB.sdk.rpc.chain.getPendingTransactions()).length > 0
            ) {
                await wait(500);
            }

            await nodeB.sdk.rpc.devel.stopSealing();

            await Promise.all(
                rejectedTransactions.map((tx: SignedTransaction) =>
                    nodeB.sdk.rpc.chain
                        .sendSignedTransaction(tx)
                        .then(txhash =>
                            expect(txhash.value).to.eq(tx.hash().value)
                        )
                )
            );

            const pendingTransactionsAfterResend = await nodeB.sdk.rpc.chain.getPendingTransactions();
            const pendingTransactionHashesAfterResend = pendingTransactionsAfterResend.map(
                (tx: SignedTransaction) => tx.hash().value
            );

            rejectedTransactions.forEach(
                tx =>
                    expect(
                        pendingTransactionHashesAfterResend.includes(
                            tx.hash().value
                        )
                    ).to.true
            );
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
            argv: ["--mem-pool-mem-limit", memoryLimit.toString()],
            base: BASE
        });
        await nodeA.start();
        await nodeA.sdk.rpc.devel.stopSealing();
    });

    it("To self", async function() {
        for (let i = 0; i < sizeLimit; i++) {
            await nodeA.mintAsset({ supply: 1, seq: i, awaitMint: false });
        }
        const pendingTransactions = await nodeA.sdk.rpc.chain.getPendingTransactions();
        expect(pendingTransactions.length).to.equal(sizeLimit);
    }).timeout(50_000);

    describe("To others", async function() {
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeB = new CodeChain({
                chain: `${__dirname}/../scheme/mempool.json`,
                argv: ["--mem-pool-mem-limit", memoryLimit.toString()],
                base: BASE
            });
            await nodeB.start();
            await nodeB.sdk.rpc.devel.stopSealing();

            await nodeA.connect(nodeB);
        });

        it("More than limit", async function() {
            const [aBlockNumber, bBlockNumber] = await Promise.all([
                nodeA.sdk.rpc.chain.getBestBlockNumber(),
                nodeB.sdk.rpc.chain.getBestBlockNumber()
            ]);
            expect(aBlockNumber).to.equal(bBlockNumber);
            const metadata = "Very large transaction" + " ".repeat(1024 * 1024);
            const minting = [];
            for (let i = 0; i < sizeLimit; i++) {
                minting.push(
                    nodeA.mintAsset({
                        supply: mintSize,
                        seq: i,
                        metadata,
                        awaitMint: false
                    })
                );
            }
            await Promise.all(minting);
            await wait(3_000);

            const pendingTransactions = await nodeB.sdk.rpc.chain.getPendingTransactions();
            expect(pendingTransactions.length).to.equal(0);
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
