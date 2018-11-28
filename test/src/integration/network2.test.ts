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

import "mocha";
import { expect } from "chai";

describe("network2 nodes", function() {
    let nodeA: CodeChain;
    let nodeB: CodeChain;
    const address = "127.0.0.1";
    before(async function() {
        nodeA = new CodeChain();
        nodeB = new CodeChain();
        await Promise.all([nodeA.start(), nodeB.start()]);
    });

    describe("Not connected", function() {
        beforeEach(async function() {
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

        it("connect", async function() {
            expect(
                await nodeA.sdk.rpc.network.connect(
                    address,
                    nodeB.port
                )
            ).to.be.a("null");
        });

        it("getPeerCount", async function() {
            expect(await nodeA.sdk.rpc.network.getPeerCount()).to.equal(0);
        });

        it("getPeers", async function() {
            expect(await nodeA.sdk.rpc.network.getPeers()).to.be.empty;
        });
    });

    describe("1 connected", function() {
        beforeEach(async function() {
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

        it("isConnected", async function() {
            expect(await nodeA.sdk.rpc.network.isConnected(address, nodeB.port))
                .to.be.true;
        });

        it("disconnect", async function() {
            expect(await nodeA.sdk.rpc.network.disconnect(address, nodeB.port))
                .to.be.null;
        });

        it("getPeerCount", async function() {
            expect(await nodeA.sdk.rpc.network.getPeerCount()).to.equal(1);
            expect(await nodeB.sdk.rpc.network.getPeerCount()).to.equal(1);
        });

        it("getPeers", async function() {
            expect(await nodeA.sdk.rpc.network.getPeers()).to.deep.equal([
                `${address}:${nodeB.port}`
            ]);
            expect(await nodeB.sdk.rpc.network.getPeers()).to.deep.equal([
                `${address}:${nodeA.port}`
            ]);
        });
    });

    after(async function() {
        await Promise.all([nodeA.clean(), nodeB.clean()]);
    });
});
