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

describe("Network RPC", () => {
    describe("1 node test", () => {
        let node: CodeChain;
        beforeAll(async () => {
            node = new CodeChain();
            await node.start();
        });

        test(`default whitelist [], disabled`, async () => {
            const { list, enabled } = await node.sdk.rpc.network.getWhitelist();
            expect(list).toEqual([]);
            expect(enabled).toBe(false);
        });

        test("default blacklist [], disabled", async () => {
            const { list, enabled } = await node.sdk.rpc.network.getBlacklist();
            expect(list).toEqual([]);
            expect(enabled).toBe(false);
        });

        test("addToWhiteList and removeFromWhitelist", async () => {
            const target = "2.2.2.2";

            await node.sdk.rpc.network.addToWhitelist(
                target,
                "tag string for the target"
            );
            let { list } = await node.sdk.rpc.network.getWhitelist();
            expect(list).toContainEqual([target, "tag string for the target"]);

            await node.sdk.rpc.network.removeFromWhitelist(target);
            ({ list } = await node.sdk.rpc.network.getWhitelist());
            expect(list).not.toContain(target);
        });

        test("addToBlacklist and removeFromBlacklist", async () => {
            const target = "1.1.1.1";

            await node.sdk.rpc.network.addToBlacklist(
                target,
                "tag string for the target"
            );
            let { list } = await node.sdk.rpc.network.getBlacklist();
            expect(list).toContainEqual([target, "tag string for the target"]);

            await node.sdk.rpc.network.removeFromBlacklist(target);
            ({ list } = await node.sdk.rpc.network.getBlacklist());
            expect(list).not.toContain(target);
        });

        test("enableWhitelist and disableWhitelist", async () => {
            await node.sdk.rpc.network.enableWhitelist();
            let { enabled } = await node.sdk.rpc.network.getWhitelist();
            expect(enabled).toBe(true);

            await node.sdk.rpc.network.disableWhitelist();
            ({ enabled } = await node.sdk.rpc.network.getWhitelist());
            expect(enabled).toBe(false);
        });

        test("enableBlacklist and disableBlacklist", async () => {
            await node.sdk.rpc.network.enableBlacklist();
            let { enabled } = await node.sdk.rpc.network.getBlacklist();
            expect(enabled).toBe(true);

            await node.sdk.rpc.network.disableBlacklist();
            ({ enabled } = await node.sdk.rpc.network.getBlacklist());
            expect(enabled).toBe(false);
        });

        afterAll(async () => {
            await node.clean();
        });
    });

    // FIXME: Connection establishment is too slow.
    // See https://github.com/CodeChain-io/codechain/issues/760
    describe.skip("2 nodes test", () => {
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
                    !(await nodeA.sdk.rpc.network.isConnected(
                        address,
                        nodeB.port
                    ))
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
                if (
                    await nodeA.sdk.rpc.network.isConnected(address, nodeB.port)
                ) {
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
});
