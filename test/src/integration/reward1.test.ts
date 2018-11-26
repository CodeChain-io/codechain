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
import { faucetAddress } from "../helper/constants";

import "mocha";
import { expect } from "chai";

describe("reward1", function() {
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
            argv: ["--force-sealing"]
        });

        await node.start();
    });

    it("getBlockReward", async function() {
        // FIXME: Add an API to SDK
        const reward = await node.sdk.rpc.sendRpcRequest(
            "engine_getBlockReward",
            [10]
        );
        expect(reward).to.equal(50);
    });

    it("null if the block is not mined", async function() {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const nonMinedBlockNumber = bestBlockNumber + 10;
        // FIXME: Add an API to SDK
        const reward = await node.sdk.rpc.sendRpcRequest(
            "chain_getMiningReward",
            [nonMinedBlockNumber]
        );
        expect(reward).to.equal(null);
    });

    it("mining reward of the empty block is the same with the block reward", async function() {
        await node.sdk.rpc.devel.startSealing();
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        // FIXME: Add an API to SDK
        const miningReward = await node.sdk.rpc.sendRpcRequest(
            "chain_getMiningReward",
            [bestBlockNumber]
        );
        const blockReward = await node.sdk.rpc.sendRpcRequest(
            "engine_getBlockReward",
            [bestBlockNumber]
        );
        expect(miningReward).to.equal(blockReward);
    });

    it("mining reward includes the block fee", async function() {
        await node.sdk.rpc.devel.stopSealing();
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        await node.sendSignedParcel({
            amount: 10,
            fee: 123,
            seq,
            awaitInvoice: false
        });
        await node.sendSignedParcel({
            amount: 10,
            fee: 456,
            seq: seq + 1,
            awaitInvoice: false
        });
        await node.sendSignedParcel({
            amount: 10,
            fee: 321,
            seq: seq + 2,
            awaitInvoice: false
        });
        await node.sdk.rpc.devel.startSealing();
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        // FIXME: Add an API to SDK
        const miningReward = await node.sdk.rpc.sendRpcRequest(
            "chain_getMiningReward",
            [bestBlockNumber]
        );
        const blockReward = await node.sdk.rpc.sendRpcRequest(
            "engine_getBlockReward",
            [bestBlockNumber]
        );
        expect(miningReward).to.equal(blockReward + 123 + 456 + 321);
    });

    afterEach(async function() {
        await node.clean();
    });
});
