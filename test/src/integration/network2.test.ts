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

// FIXME: Connection establishment is too slow.
// See https://github.com/CodeChain-io/codechain/issues/760
describeSkippedInTravis("network2 nodes", () => {
    let nodeA: CodeChain;
    let nodeB: CodeChain;
    const address = "127.0.0.1";
    beforeAll(async () => {
        nodeA = new CodeChain();
        nodeB = new CodeChain();
        await Promise.all([nodeA.start(), nodeB.start()]);
    });

    describe("Not connected", () => {
        beforeEach(async () => {
            // ensure disconnected
            if (
                !(await nodeA.sdk.rpc.network.isConnected(address, nodeB.port))
            ) {
                return;
            }
            await nodeA.sdk.rpc.network.disconnect(address, nodeB.port);
            while (
                (await nodeA.sdk.rpc.network.isConnected(
                    address,
                    nodeB.port
                )) === true
            ) {
                await wait(100);
            }
        });

        test("connect", async () => {
            expect(
                await nodeA.sdk.rpc.network.connect(
                    address,
                    nodeB.port
                )
            ).toBe(null);
        });

        test("getPeerCount", async () => {
            expect(await nodeA.sdk.rpc.network.getPeerCount()).toBe(0);
        });

        test("getPeers", async () => {
            expect(await nodeA.sdk.rpc.network.getPeers()).toEqual([]);
        });
    });

    describe("1 connected", () => {
        beforeEach(async () => {
            // ensure connected
            if (await nodeA.sdk.rpc.network.isConnected(address, nodeB.port)) {
                return;
            }
            await nodeA.sdk.rpc.network.connect(
                address,
                nodeB.port
            );
            while (
                (await nodeA.sdk.rpc.network.isConnected(
                    address,
                    nodeB.port
                )) === false
            ) {
                await wait(100);
            }
        });

        test("isConnected", async () => {
            expect(
                await nodeA.sdk.rpc.network.isConnected(address, nodeB.port)
            ).toBe(true);
        });

        test("disconnect", async () => {
            expect(
                await nodeA.sdk.rpc.network.disconnect(address, nodeB.port)
            ).toBe(null);
        });

        test("getPeerCount", async () => {
            expect(await nodeA.sdk.rpc.network.getPeerCount()).toBe(1);
            expect(await nodeB.sdk.rpc.network.getPeerCount()).toBe(1);
        });

        test("getPeers", async () => {
            expect(await nodeA.sdk.rpc.network.getPeers()).toEqual([
                `${address}:${nodeB.port}`
            ]);
            expect(await nodeB.sdk.rpc.network.getPeers()).toEqual([
                `${address}:${nodeA.port}`
            ]);
        });
    });

    afterAll(async () => {
        await Promise.all([nodeA.clean(), nodeB.clean()]);
    });
});
