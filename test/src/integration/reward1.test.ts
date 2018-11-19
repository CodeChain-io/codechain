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

describe("reward1", () => {
    let node: CodeChain;

    beforeEach(async () => {
        node = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`
        });

        await node.start();
    });

    test("getBlockReward", async () => {
        // FIXME: Add an API to SDK
        const reward = await node.sdk.rpc.sendRpcRequest(
            "chain_getBlockReward",
            [10]
        );
        expect(reward).toEqual(50);
    });

    afterEach(async () => {
        await node.clean();
    });
});
