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

import "mocha";
import { expect } from "chai";

describe("syncEmptyBlock", function() {
    // NOTE: To create empty blocks, enable --force-sealing option, and then,
    // trigger it by calling devel_startSealing RPC API.
    describe("empty block", function() {
        let nodeA: CodeChain;
        let nodeB: CodeChain;

        beforeEach(async function() {
            nodeA = new CodeChain({ argv: ["--force-sealing"] });
            nodeB = new CodeChain({ argv: ["--force-sealing"] });
            await Promise.all([nodeA.start(), nodeB.start()]);
            await nodeA.connect(nodeB);
        });

        it("nodeA creates an empty block", async function() {
            await nodeA.sdk.rpc.devel.startSealing();
            expect(await nodeA.getBestBlockNumber()).to.equal(1);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).to.equal(1);
            expect(await nodeA.getBestBlockHash()).to.deep.equal(
                await nodeB.getBestBlockHash()
            );
        });

        it("nodeA creates 3 empty blocks", async function() {
            await Promise.all([
                nodeA.sdk.rpc.devel.startSealing(),
                nodeA.sdk.rpc.devel.startSealing(),
                nodeA.sdk.rpc.devel.startSealing()
            ]);

            expect(await nodeA.getBestBlockNumber()).to.equal(3);
            await nodeA.waitBlockNumberSync(nodeB);
            expect(await nodeB.getBestBlockNumber()).to.equal(3);
            expect(await nodeA.getBestBlockHash()).to.deep.equal(
                await nodeB.getBestBlockHash()
            );
        });

        afterEach(async function() {
            if (this.currentTest!.state === "failed") {
                nodeA.testFailed(this.currentTest!.fullTitle());
                nodeB.testFailed(this.currentTest!.fullTitle());
            }
            await Promise.all([nodeA.clean(), nodeB.clean()]);
        });
    });
});
