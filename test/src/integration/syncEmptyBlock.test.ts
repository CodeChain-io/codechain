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

// FIXME: It fails due to timeout when the block sync extension is stuck. See
// https://github.com/CodeChain-io/codechain/issues/662
describe.skip("syncEmptyBlock", () => {
    // NOTE: To create empty blocks, enable --force-sealing option, and then,
    // trigger it by calling devel_startSealing RPC API.
    describe("empty block", () => {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async () => {
            nodeA = new CodeChain({ argv: ["--force-sealing"] });
            nodeB = new CodeChain({ argv: ["--force-sealing"] });
            await Promise.all([nodeA.start(), nodeB.start()]);
            await nodeA.connect(nodeB);
        });

        test("nodeA creates an empty block", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            expect(await nodeA.getBestBlockNumber()).toBe(1);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).toBe(1);
            expect(await nodeA.getBestBlockHash()).toEqual(
                await nodeB.getBestBlockHash()
            );
        });

        test("nodeA creates 3 empty blocks", async () => {
            await nodeA.sdk.rpc.devel.startSealing();
            await nodeA.sdk.rpc.devel.startSealing();
            await nodeA.sdk.rpc.devel.startSealing();

            expect(await nodeA.getBestBlockNumber()).toBe(3);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).toBe(3);
            expect(await nodeA.getBestBlockHash()).toEqual(
                await nodeB.getBestBlockHash()
            );
        });

        afterEach(async () => {
            await Promise.all([nodeA.clean(), nodeB.clean()]);
        });
    });
});
