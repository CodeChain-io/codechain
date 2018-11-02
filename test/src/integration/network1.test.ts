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

describe("network1 node test", () => {
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
