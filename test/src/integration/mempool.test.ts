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

    test("To self", async () => {
        for (let i = 0; i < sizeLimit * 2; i++) {
            await nodeA.sendSignedParcel({ nonce: i, awaitInvoice: false });
        }
        const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
        expect(pendingParcels.length).toEqual(sizeLimit * 2);
    }, 10000);

    // FIXME: It fails due to timeout when the block sync extension is stuck.
    // See https://github.com/CodeChain-io/codechain/issues/662
    describe.skip("To others", async () => {
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
                        nonce: i,
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
                await nodeA.mintAssets({ count: mintSize, nonce: i });
            }
            const pendingParcels = await nodeA.sdk.rpc.chain.getPendingParcels();
            expect(pendingParcels.length).toEqual(sizeLimit);
        },
        50000
    );

    // FIXME: It fails due to timeout when the block sync extension is stuck.
    // See https://github.com/CodeChain-io/codechain/issues/662
    describe.skip("To others", async () => {
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
                    await nodeA.mintAssets({ count: mintSize, nonce: i });
                }

                for (let i = 0; i < 10; i++) {
                    const pendingParcels = await nodeB.sdk.rpc.chain.getPendingParcels();
                    expect(
                        (await nodeB.sdk.rpc.chain.getPendingParcels()).length
                    ).toEqual(0);
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
