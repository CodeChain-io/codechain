// Copyright 2018-2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// pubKeylished by the Free Software Foundation, either version 3 of the
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
import { faucetAddress } from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("solo - 1 node", function() {
    let node: CodeChain;
    let privKey: string;
    let pubKey: string;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();

        privKey = node.sdk.util.generatePrivateKey();
        pubKey = node.sdk.util.getPublicFromPrivate(privKey);
    });

    it("Make regular key", async function() {
        try {
            await node.sendPayTx({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).is.similarTo(ERROR.NOT_ENOUGH_BALANCE);
        }

        await node.setRegularKey(pubKey);
        await node.sendPayTx({ secret: privKey });
    });

    it("Make then change regular key with the master key", async function() {
        await node.setRegularKey(pubKey);
        await node.sendPayTx({ secret: privKey });

        const newPrivKey = node.sdk.util.generatePrivateKey();
        const newPubKey = node.sdk.util.getPublicFromPrivate(newPrivKey);

        await node.setRegularKey(newPubKey);
        await node.sendPayTx({ secret: newPrivKey });
        try {
            await node.sendPayTx({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).is.similarTo(ERROR.NOT_ENOUGH_BALANCE);
        }
    });

    it("Make then change regular key with the previous regular key", async function() {
        await node.setRegularKey(pubKey);
        await node.sendPayTx({ secret: privKey });

        const newPrivKey = node.sdk.util.generatePrivateKey();
        const newPubKey = node.sdk.util.getPublicFromPrivate(newPrivKey);

        await node.setRegularKey(newPubKey, { secret: privKey });
        await node.sendPayTx({ secret: newPrivKey });
        try {
            await node.sendPayTx({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).is.similarTo(ERROR.NOT_ENOUGH_BALANCE);
        }
    });

    it("Try to use the master key instead of the regular key", async function() {
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        await node.sdk.rpc.devel.stopSealing();
        const hash = await node.setRegularKey(pubKey, { seq });
        const tx = await node.sendPayTx({ seq: seq + 1 });
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumber + 1);

        expect(await node.sdk.rpc.chain.getErrorHint(hash)).be.null;
        expect(await node.sdk.rpc.chain.getErrorHint(tx.hash())).not.be.null;
    });

    it("Try to use the key of another account as its regular key", async function() {
        const account = node.sdk.util.getAccountIdFromPrivate(privKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sdk.rpc.devel.stopSealing();
        const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const tx1 = await node.sendPayTx({
            quantity: 5,
            recipient: address,
            seq
        });
        const hash2 = await node.setRegularKey(pubKey, { seq: seq + 1 });
        await node.sdk.rpc.devel.startSealing();
        await node.waitBlockNumber(blockNumber + 1);

        const block = (await node.sdk.rpc.chain.getBlock(blockNumber + 1))!;
        expect(block).not.be.null;
        expect(block.transactions.length).equal(1);
        expect(block.transactions[0].hash().value).equal(tx1.hash().value);
        expect(await node.sdk.rpc.chain.getErrorHint(hash2)).not.be.null;
    }).timeout(10_000);

    it("Try to use the regulary key already used in another account", async function() {
        const newPrivKey = node.sdk.util.generatePrivateKey();
        const account = node.sdk.util.getAccountIdFromPrivate(newPrivKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sendPayTx({ quantity: 100, recipient: address });
        const seq = await node.sdk.rpc.chain.getSeq(address);

        const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();

        await node.sdk.rpc.devel.stopSealing();
        const hash1 = await node.setRegularKey(pubKey, {
            seq,
            secret: newPrivKey
        });
        const hash2 = await node.setRegularKey(pubKey, {
            seq: seq + 1
        });
        await node.sdk.rpc.devel.startSealing();

        await node.waitBlockNumber(blockNumber);

        const block = (await node.sdk.rpc.chain.getBlock(blockNumber + 1))!;
        expect(block).not.be.null;
        expect(block.transactions.length).equal(1);
        expect(block.transactions[0].hash().value).equal(hash1.value);

        expect(await node.sdk.rpc.chain.getErrorHint(hash2)).not.null;
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
