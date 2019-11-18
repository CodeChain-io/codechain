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
import * as fs from "fs";
import "mocha";
import * as path from "path";

import { aliceAddress } from "../helper/constants";
import CodeChain from "../helper/spawn";

const SNAPSHOT_PATH = `${__dirname}/../../../snapshot/`;

describe("Snapshot", async function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain({
            argv: ["--snapshot-path", SNAPSHOT_PATH]
        });
        await node.start();
    });

    it("can make a snapshot when it is requsted with devel rpc", async function() {
        const pay = await node.sendPayTx({
            quantity: 100,
            recipient: aliceAddress
        });

        const blockHash = (await node.sdk.rpc.chain.getTransaction(pay.hash()))!
            .blockHash!;
        await node.sdk.rpc.sendRpcRequest("devel_snapshot", [
            blockHash.toJSON()
        ]);
        // Wait for 1 secs
        await new Promise(resolve => setTimeout(resolve, 1000));

        const stateRoot = (await node.sdk.rpc.chain.getBlock(blockHash))!
            .stateRoot;
        expect(
            path.join(SNAPSHOT_PATH, blockHash.toString(), stateRoot.toString())
        ).to.satisfies(fs.existsSync);
    });

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
    });

    after(async function() {
        await node.clean();
    });
});
