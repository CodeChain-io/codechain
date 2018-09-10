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

describe("2 nodes", () => {
    let nodeA: CodeChain;
    let nodeB: CodeChain;

    beforeEach(async () => {
        nodeA = new CodeChain();
        nodeB = new CodeChain();
        await nodeA.start();
        await nodeB.start();
    });

    test("should be able to connect", async () => {
        await nodeA.connect(nodeB);
    });

    afterEach(async () => {
        await nodeA.clean();
        await nodeB.clean();
    });
});

describe("5 nodes", () => {
    const numOfNodes = 5;
    let nodes: CodeChain[];
    let bootstrapNode: CodeChain;

    beforeEach(async () => {
        nodes = [new CodeChain()];
        bootstrapNode = nodes[0];

        for (let i = 1; i < numOfNodes; i++) {
            nodes.push(new CodeChain());
        }

        await Promise.all(
            nodes.map(node =>
                node.start([
                    "--bootstrap-addresses",
                    `127.0.0.1:${bootstrapNode.port}`,
                    "--discovery-refresh",
                    "50"
                ])
            )
        );
    });

    test.skip(
        "number of peers",
        async () => {
            await nodes[0].waitPeers(numOfNodes - 1);
            await nodes[1].waitPeers(numOfNodes - 1);
            await nodes[2].waitPeers(numOfNodes - 1);
            await nodes[3].waitPeers(numOfNodes - 1);
            await nodes[4].waitPeers(numOfNodes - 1);
            expect(await nodes[0].sdk.rpc.network.getPeerCount()).toEqual(
                numOfNodes - 1
            );
            expect(await nodes[1].sdk.rpc.network.getPeerCount()).toEqual(
                numOfNodes - 1
            );
            expect(await nodes[2].sdk.rpc.network.getPeerCount()).toEqual(
                numOfNodes - 1
            );
            expect(await nodes[3].sdk.rpc.network.getPeerCount()).toEqual(
                numOfNodes - 1
            );
            expect(await nodes[4].sdk.rpc.network.getPeerCount()).toEqual(
                numOfNodes - 1
            );
        },
        30 * 1000
    );

    afterEach(async () => {
        await Promise.all(nodes.map(node => node.clean()));
    });
});
