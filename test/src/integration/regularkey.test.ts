// Copyright 2018 Kodebox, Inc.
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

import CodeChain from "../helper/spawn";
import { ERROR, errorMatcher } from "../helper/error";

import "mocha";
import { expect } from "chai";

const INVOICE = {
    SUCCESS: {
        success: true,
        error: undefined
    },
    REGULARKEY_ALREADY_IN_USE_AS_PLATFORM_ACCOUNT: {
        success: false,
        error: {
            type: "RegularKeyAlreadyInUseAsPlatformAccount"
        }
    },
    REGULARKEY_ALREADY_IN_USE: {
        success: false,
        error: {
            type: "RegularKeyAlreadyInUse"
        }
    }
};

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
            await node.sendSignedParcel({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).to.satisfy(errorMatcher(ERROR.NOT_ENOUGH_BALANCE));
        }

        await node.setRegularKey(pubKey);
        await node.sendSignedParcel({ secret: privKey });
    });

    it("Make then change regular key", async function() {
        await node.setRegularKey(pubKey);
        await node.sendSignedParcel({ secret: privKey });

        const newPrivKey = node.sdk.util.generatePrivateKey();
        const newPubKey = node.sdk.util.getPublicFromPrivate(newPrivKey);

        await node.setRegularKey(newPubKey);
        await node.sendSignedParcel({ secret: newPrivKey });
        try {
            await node.sendSignedParcel({ secret: privKey });
            expect.fail("It must fail");
        } catch (e) {
            expect(e).to.satisfy(errorMatcher(ERROR.NOT_ENOUGH_BALANCE));
        }
    });

    it("Try to use the key of another account as its regular key", async function() {
        const account = node.sdk.util.getAccountIdFromPrivate(privKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sendSignedParcel({ amount: 5, recipient: address });
        const invoice = (await node.setRegularKey(pubKey))!;
        expect(invoice.error!.type).to.equal(
            INVOICE.REGULARKEY_ALREADY_IN_USE_AS_PLATFORM_ACCOUNT.error.type
        );
        expect(invoice.success).to.equal(
            INVOICE.REGULARKEY_ALREADY_IN_USE_AS_PLATFORM_ACCOUNT.success
        );
    }).timeout(10_000);

    it("Try to use the regulary key already used in another account", async function() {
        const newPrivKey = node.sdk.util.generatePrivateKey();
        const account = node.sdk.util.getAccountIdFromPrivate(newPrivKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sendSignedParcel({ amount: 100, recipient: address });
        const seq = await node.sdk.rpc.chain.getSeq(address);
        let invoice = (await node.setRegularKey(pubKey, {
            seq,
            secret: newPrivKey
        }))!;
        expect(invoice.error).to.be.undefined;
        expect(invoice.success).to.be.true;
        invoice = (await node.setRegularKey(pubKey))!;
        expect(invoice.error!.type).to.equal(
            INVOICE.REGULARKEY_ALREADY_IN_USE.error.type
        );
        expect(invoice.success).to.equal(
            INVOICE.REGULARKEY_ALREADY_IN_USE.success
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
