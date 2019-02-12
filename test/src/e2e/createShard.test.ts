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

import { expect } from "chai";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("CreateShard", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("Create 1 shard", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        const tx = node.sdk.core
            .createCreateShardTransaction()
            .sign({ secret: faucetSecret, seq, fee: 10 });
        const beforeShardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx.hash(), null]
        );
        expect(beforeShardId).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx);
        const invoice = (await node.sdk.rpc.chain.getInvoice(tx.hash(), {
            timeout: 300 * 1000
        }))!;
        expect(invoice).not.to.be.null;
        expect(invoice).to.be.true;
        const afterShardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx.hash(), null]
        );
        expect(afterShardId).not.to.be.null;
    });

    it("Create 2 shards", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        const tx1 = node.sdk.core
            .createCreateShardTransaction()
            .sign({ secret: faucetSecret, seq, fee: 10 });
        const beforeShardId1 = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx1.hash(), null]
        );
        expect(beforeShardId1).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx1);
        const invoice1 = (await node.sdk.rpc.chain.getInvoice(tx1.hash(), {
            timeout: 300 * 1000
        }))!;
        expect(invoice1).not.to.be.null;
        expect(invoice1).to.be.true;
        const shardId1 = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx1.hash(), null]
        );
        expect(shardId1).not.to.be.null;

        const tx2 = node.sdk.core
            .createCreateShardTransaction()
            .sign({ secret: faucetSecret, seq: seq + 1, fee: 10 });
        const beforeShardId2 = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx2.hash(), null]
        );
        expect(beforeShardId2).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx2);
        const invoice2 = (await node.sdk.rpc.chain.getInvoice(tx2.hash(), {
            timeout: 300 * 1000
        }))!;
        expect(invoice2).not.to.be.null;
        expect(invoice2).to.be.true;
        const shardId2 = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx2.hash(), null]
        );
        expect(shardId2).not.to.be.null;
    });

    after(async function() {
        await node.clean();
    });
});
