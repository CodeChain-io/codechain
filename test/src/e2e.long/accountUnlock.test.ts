// Copyright 2018-2019 Kodebox, Inc.
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
import { ERROR } from "../helper/error";
import { wait } from "../helper/promise";
import { makeRandomH256, makeRandomPassphrase } from "../helper/random";
import CodeChain from "../helper/spawn";

describe("account unlock", function() {
    let node: CodeChain;
    const unlockTestSize = 15;

    beforeEach(async function() {
        node = new CodeChain();
        await node.start();
    });

    it(`unlock 1 second ${unlockTestSize} times and check working well with sign`, async function() {
        const secret = node.sdk.util.generatePrivateKey();
        const account = node.sdk.util.getAccountIdFromPrivate(secret);
        const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
            account,
            { networkId: "tc" }
        );
        const passphrase = makeRandomPassphrase();
        await node.sdk.rpc.account.importRaw(secret, passphrase);

        for (let i = 0; i < unlockTestSize; i++) {
            const message = makeRandomH256();
            const calculatedSignature = node.sdk.util.signEcdsa(
                message,
                secret
            );
            await node.sdk.rpc.account.unlock(address, passphrase, 1);

            for (let j = 0; j <= 2; j++) {
                try {
                    const signature = await node.sdk.rpc.account.sign(
                        message,
                        address
                    );
                    expect(signature).to.equal(`0x${calculatedSignature}`);
                } catch (e) {
                    expect.fail(e);
                }
                await wait(100);
            }
            await wait(1000 - 100 * 3);

            try {
                await node.sdk.rpc.account.sign(message, address);
                expect.fail();
            } catch (e) {
                expect(e).similarTo(ERROR.NOT_UNLOCKED);
            }
        }
    }).timeout(2000 * unlockTestSize + 5000);

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
