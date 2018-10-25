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

const ERROR = {
    NOT_ENOUGH_BALANCE: {
        code: -32032,
        data: expect.anything(),
        message: expect.anything()
    }
};

const INVOICE = {
    SUCCESS: {
        success: true
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

describe("solo - 1 node", () => {
    let node: CodeChain;
    let privKey: string;
    let pubKey: string;

    beforeEach(async () => {
        node = new CodeChain();
        await node.start();

        privKey = node.sdk.util.generatePrivateKey();
        pubKey = node.sdk.util.getPublicFromPrivate(privKey);
    });

    test("Make regular key", async done => {
        try {
            await node.sendSignedParcel({ secret: privKey });
            done.fail();
        } catch (e) {
            expect(e).toEqual(ERROR.NOT_ENOUGH_BALANCE);
        }

        await node.setRegularKey(pubKey);
        await node.sendSignedParcel({ secret: privKey });
        done();
    });

    test("Make then change regular key", async done => {
        await node.setRegularKey(pubKey);
        await node.sendSignedParcel({ secret: privKey });

        const newPrivKey = node.sdk.util.generatePrivateKey();
        const newPubKey = node.sdk.util.getPublicFromPrivate(newPrivKey);

        await node.setRegularKey(newPubKey);
        await node.sendSignedParcel({ secret: newPrivKey });
        try {
            await node.sendSignedParcel({ secret: privKey });
            done.fail();
        } catch (e) {
            expect(e).toEqual(ERROR.NOT_ENOUGH_BALANCE);
            done();
        }
    });

    test(
        "Try to use the key of another account as its regular key",
        async () => {
            const account = node.sdk.util.getAccountIdFromPrivate(privKey);
            const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                account,
                { networkId: "tc" }
            ).toString();

            await node.sendSignedParcel({ amount: 5, recipient: address });
            const invoice = await node.setRegularKey(pubKey);
            expect(invoice).toEqual(
                INVOICE.REGULARKEY_ALREADY_IN_USE_AS_PLATFORM_ACCOUNT
            );
        },
        10000
    );

    test("Try to use the regulary key already used in another account", async () => {
        const newPrivKey = node.sdk.util.generatePrivateKey();
        const account = node.sdk.util.getAccountIdFromPrivate(newPrivKey);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        ).toString();

        await node.sendSignedParcel({ amount: 100, recipient: address });
        const seq = await node.sdk.rpc.chain.getSeq(address);
        let invoice = await node.setRegularKey(pubKey, {
            seq,
            secret: newPrivKey
        });
        expect(invoice).toEqual(INVOICE.SUCCESS);
        invoice = await node.setRegularKey(pubKey);
        expect(invoice).toEqual(INVOICE.REGULARKEY_ALREADY_IN_USE);
    });

    afterEach(async () => {
        await node.clean();
    });
});
