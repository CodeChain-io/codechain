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
import {
    aliceAddress,
    aliceSecret,
    bobAddress,
    bobSecret,
    faucetAddress,
    faucetSecret
} from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("CreateShard", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("Create 1 shard", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;

        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq, fee: 10 })
        );

        const tx = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress] })
            .sign({ secret: faucetSecret, seq: seq + 1, fee: 10 });
        const beforeBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardIdByHash", [
                tx.hash(),
                null
            ])
        ).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx);
        expect(
            await node.sdk.rpc.chain.getTransactionResult(tx.hash(), {
                timeout: 300 * 1000
            })
        ).to.be.true;
        const afterShardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx.hash(), null]
        );
        expect(afterShardId).not.to.be.null;

        const shardOwners = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardOwners",
            [afterShardId, null]
        );
        expect(shardOwners).to.deep.equal([faucetAddress.value]); // The creator becomes the owner.
        const shardUsers = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardUsers",
            [afterShardId, null]
        );
        expect(shardUsers).to.deep.equal([aliceAddress.value]);

        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardOwners", [
                afterShardId,
                beforeBlockNumber
            ])
        ).to.be.null;
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardUsers", [
                afterShardId,
                beforeBlockNumber
            ])
        ).to.be.null;
    });

    it("setShardUsers", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq, fee: 10 })
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: bobAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq: seq + 1, fee: 10 })
        );

        const tx = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress] })
            .sign({ secret: faucetSecret, seq: seq + 2, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(tx);
        const shardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx.hash(), null]
        );

        const users = [aliceAddress, bobAddress];
        const setShardUsers = node.sdk.core
            .createSetShardUsersTransaction({ shardId, users })
            .sign({ secret: faucetSecret, seq: seq + 3, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(setShardUsers);
        const shardUsers = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardUsers",
            [shardId, null]
        );
        expect(shardUsers).to.deep.equal(users.map(user => user.value));
    });

    it("setShardOwners", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq, fee: 10 })
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: bobAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq: seq + 1, fee: 10 })
        );
        const tx = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress, bobAddress] })
            .sign({ secret: faucetSecret, seq: seq + 2, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(tx);
        const shardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [tx.hash(), null]
        );

        const owners = [aliceAddress, faucetAddress, bobAddress];
        const setShardOwners = node.sdk.core
            .createSetShardOwnersTransaction({ shardId, owners })
            .sign({ secret: faucetSecret, seq: seq + 3, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(setShardOwners);
        const shardOwners = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardOwners",
            [shardId, null]
        );
        expect(shardOwners).to.deep.equal(owners.map(owner => owner.value));
    });

    it("Create 2 shards", async function() {
        const seq: number = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq, fee: 10 })
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: bobAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq: seq + 1, fee: 10 })
        );
        const tx1 = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress, bobAddress] })
            .sign({ secret: faucetSecret, seq: seq + 2, fee: 10 });
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardIdByHash", [
                tx1.hash(),
                null
            ])
        ).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx1);
        expect(
            await node.sdk.rpc.chain.getTransactionResult(tx1.hash(), {
                timeout: 300 * 1000
            })
        ).to.be.true;
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardIdByHash", [
                tx1.hash(),
                null
            ])
        ).not.to.be.null;

        const tx2 = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress, bobAddress] })
            .sign({ secret: faucetSecret, seq: seq + 3, fee: 10 });
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardIdByHash", [
                tx2.hash(),
                null
            ])
        ).to.be.null;
        await node.sdk.rpc.chain.sendSignedTransaction(tx2);
        expect(
            await node.sdk.rpc.chain.getTransactionResult(tx2.hash(), {
                timeout: 300 * 1000
            })
        ).to.be.true;
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getShardIdByHash", [
                tx2.hash(),
                null
            ])
        ).not.to.be.null;
    });

    it("non-user cannot mint", async function() {
        const faucetSeq: number = (await node.sdk.rpc.chain.getSeq(
            faucetAddress
        ))!;
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: aliceAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq: faucetSeq, fee: 10 })
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({ recipient: bobAddress, quantity: 1 })
                .sign({ secret: faucetSecret, seq: faucetSeq + 1, fee: 10 })
        );
        const createShard = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress] })
            .sign({ secret: faucetSecret, seq: faucetSeq + 2, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(createShard);
        const shardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [createShard.hash(), null]
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({
                    recipient: bobAddress,
                    quantity: 100
                })
                .sign({ secret: faucetSecret, seq: faucetSeq + 3, fee: 10 })
        );

        const bobSeq: number = (await node.sdk.rpc.chain.getSeq(bobAddress))!;
        const mint = node.sdk.core
            .createMintAssetTransaction({
                scheme: {
                    shardId,
                    metadata: "",
                    supply: "0xa"
                },
                recipient: await node.createP2PKHAddress()
            })
            .sign({ secret: bobSecret, seq: bobSeq, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(mint);

        expect(await node.sdk.rpc.chain.getTransactionResult(mint.hash())).to.be
            .false;
        const hint = await node.sdk.rpc.chain.getErrorHint(mint.hash());
        expect(hint).includes("permission");
    });

    it("user can mint", async function() {
        const faucetSeq: number = (await node.sdk.rpc.chain.getSeq(
            faucetAddress
        ))!;
        const createShard = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress] })
            .sign({ secret: faucetSecret, seq: faucetSeq, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(createShard);
        const shardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [createShard.hash(), null]
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({
                    recipient: aliceAddress,
                    quantity: 100
                })
                .sign({ secret: faucetSecret, seq: faucetSeq + 1, fee: 10 })
        );

        const aliceSeq: number = (await node.sdk.rpc.chain.getSeq(
            aliceAddress
        ))!;
        const mint = node.sdk.core
            .createMintAssetTransaction({
                scheme: {
                    shardId,
                    metadata: "",
                    supply: "0xa"
                },
                recipient: await node.createP2PKHAddress()
            })
            .sign({ secret: aliceSecret, seq: aliceSeq, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(mint);

        expect(await node.sdk.rpc.chain.getTransactionResult(mint.hash())).to.be
            .true;
        const hint = await node.sdk.rpc.chain.getErrorHint(mint.hash());
        expect(hint).to.be.null;
    });

    it("non-user can mint after becoming a user", async function() {
        const faucetSeq: number = (await node.sdk.rpc.chain.getSeq(
            faucetAddress
        ))!;
        const createShard = node.sdk.core
            .createCreateShardTransaction({ users: [aliceAddress] })
            .sign({ secret: faucetSecret, seq: faucetSeq, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(createShard);
        const shardId = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardIdByHash",
            [createShard.hash(), null]
        );
        await node.sdk.rpc.chain.sendSignedTransaction(
            node.sdk.core
                .createPayTransaction({
                    recipient: bobAddress,
                    quantity: 100
                })
                .sign({ secret: faucetSecret, seq: faucetSeq + 1, fee: 10 })
        );

        const bobSeq: number = (await node.sdk.rpc.chain.getSeq(bobAddress))!;
        const recipient = await node.createP2PKHAddress();
        const mint1 = node.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId,
                metadata: "",
                supply: "0xa"
            },
            recipient
        });
        const signedMint1 = mint1.sign({
            secret: bobSecret,
            seq: bobSeq,
            fee: 30
        });
        await node.sdk.rpc.chain.sendSignedTransaction(signedMint1);

        expect(
            await node.sdk.rpc.chain.getTransactionResult(signedMint1.hash())
        ).to.be.false;
        expect(
            await node.sdk.rpc.chain.getTransactionResultsByTracker(
                mint1.tracker()
            )
        ).deep.equal([false]);
        const hint = await node.sdk.rpc.chain.getErrorHint(signedMint1.hash());
        expect(hint).includes("permission");

        const newUsers = [aliceAddress, bobAddress];
        const setShardUsers = node.sdk.core
            .createSetShardUsersTransaction({ shardId, users: newUsers })
            .sign({ secret: faucetSecret, seq: faucetSeq + 2, fee: 10 });
        await node.sdk.rpc.chain.sendSignedTransaction(setShardUsers);
        const shardUsers = await node.sdk.rpc.sendRpcRequest(
            "chain_getShardUsers",
            [shardId, null]
        );
        expect(shardUsers).to.deep.equal(newUsers.map(user => user.value));

        const mint2 = node.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId,
                metadata: "",
                supply: "0xa"
            },
            recipient
        });
        expect(mint1.tracker().value).equal(mint2.tracker().value);
        const signedMint2 = mint2.sign({
            secret: bobSecret,
            seq: bobSeq + 1,
            fee: 20
        });
        await node.sdk.rpc.chain.sendSignedTransaction(signedMint2);

        expect(
            await node.sdk.rpc.chain.getTransactionResult(signedMint2.hash())
        ).to.be.true;
        expect(
            await node.sdk.rpc.chain.getTransactionResultsByTracker(
                mint2.tracker()
            )
        ).deep.equal([false, true]);
        expect(await node.sdk.rpc.chain.getErrorHint(signedMint2.hash())).to.be
            .null;
    });

    after(async function() {
        await node.clean();
    });
});
