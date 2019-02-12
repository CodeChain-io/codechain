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
        try {
            await node.sendPayTx({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).is.similarTo(ERROR.NOT_ENOUGH_BALANCE);
        }

        await node.setRegularKey(pubKey);
        const tx = await node.sendPayTx({ awaitInvoice: false });
        const invoice = (await node.sdk.rpc.chain.getInvoice(tx.hash(), {
            timeout: 300 * 1000
        }))!;
        expect(invoice).to.be.false;
    });

    it("Try to use the key of another account as its regular key", async function() {
        const account = node.sdk.util.getAccountIdFromPrivate(privKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sendPayTx({ quantity: 5, recipient: address });
        const invoice = (await node.setRegularKey(pubKey))!;
        expect(invoice).to.be.false;
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
        let invoice = (await node.setRegularKey(pubKey, {
            seq,
            secret: newPrivKey
        }))!;
        expect(invoice).to.be.true;
        invoice = (await node.setRegularKey(pubKey))!;
        expect(invoice).to.be.false;
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
