// Copyright 2019 Kodebox, Inc.
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

import "mocha";
import { PromiseExpect } from "../helper/promise";
import CodeChain from "../helper/spawn";

describe("bootstrap", function() {
    const NUM_NODES = 100;
    describe(`connect ${NUM_NODES} nodes`, function() {
        const promiseExpect = new PromiseExpect();
        let bootstrap: CodeChain;

        beforeEach(async function() {
            this.timeout(10_000);

            bootstrap = new CodeChain({
                argv: ["--no-discovery", "--max-peers", "100"]
            });
            await bootstrap.start();
        });

        it("Connect all nodes", async function() {
            const nodes = [];
            for (let i = 0; i < NUM_NODES; i++) {
                const node = new CodeChain({
                    argv: [
                        "--no-discovery",
                        "--bootstrap-addresses",
                        `127.0.0.1:${bootstrap.port}`
                    ]
                });
                nodes.push(node);
            }

            await promiseExpect.shouldFulfill(
                `start ${NUM_NODES} nodes`,
                Promise.all(nodes.map(node => node.start()))
            );

            await promiseExpect.shouldFulfill(
                `wait ${NUM_NODES} peers`,
                bootstrap.waitPeers(NUM_NODES)
            );

            await promiseExpect.shouldFulfill(
                `clean ${NUM_NODES} nodes`,
                Promise.all(nodes.map(node => node.clean()))
            );

            await promiseExpect.shouldFulfill(
                "wait all peer disconnected",
                bootstrap.waitPeers(0)
            );
        }).timeout(50_000 + 5_000 * NUM_NODES);

        afterEach(async function() {
            this.timeout(5_000 + 3_000 * NUM_NODES);

            if (this.currentTest!.state === "failed") {
                bootstrap.keepLogs();
            }
            await bootstrap.clean();
        });
    });
});
