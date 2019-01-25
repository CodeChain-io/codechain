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
import { ERROR } from "../helper/error";
import { invalidAddress, invalidSecret } from "../helper/constants";

import "mocha";
import { expect } from "chai";
import { xor128 } from "seedrandom";

describe("account", function() {
    describe("account base test", function() {
        let node: CodeChain;
        before(async function() {
            node = new CodeChain();
            await node.start();
        });

        it("getList", async function() {
            expect(await node.sdk.rpc.account.getList()).not.to.be.null;
        });

        it("create", async function() {
            expect(await node.sdk.rpc.account.create()).not.to.be.null;
            expect(await node.sdk.rpc.account.create("my-password")).not.to.be
                .null;
        });

        describe("importRaw", function() {
            let randomSecret: string;
            beforeEach(function() {
                randomSecret = node.sdk.util.generatePrivateKey();
            });

            it("Ok", async function() {
                const account = node.sdk.util.getAccountIdFromPrivate(
                    randomSecret
                );
                const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                    account,
                    { networkId: "tc" }
                );
                expect(
                    await node.sdk.rpc.account.importRaw(randomSecret)
                ).to.equal(address.toString());
            });

            it("KeyError", async function() {
                try {
                    await node.sdk.rpc.account.importRaw(invalidSecret);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.KEY_ERROR);
                }
            });

            it("AlreadyExists", async function() {
                try {
                    await node.sdk.rpc.account.importRaw(randomSecret);
                    await node.sdk.rpc.account.importRaw(randomSecret);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.ALREADY_EXISTS);
                }
            });
        });

        describe("sign", function() {
            const message =
                "0000000000000000000000000000000000000000000000000000000000000000";
            let address: string;
            let secret: string;
            beforeEach(async function() {
                secret = node.sdk.util.generatePrivateKey();
                address = await node.sdk.rpc.account.importRaw(
                    secret,
                    "my-password"
                );
            });

            it("Ok", async function() {
                const { r, s, v } = node.sdk.util.signEcdsa(message, secret);
                const signature = await node.sdk.rpc.account.sign(
                    message,
                    address,
                    "my-password"
                );
                expect(signature).to.include(r);
                expect(signature).to.include(s);
                expect(signature).to.include(v);
            });

            it("WrongPassword", async function() {
                try {
                    await node.sdk.rpc.account.sign(
                        message,
                        address,
                        "wrong-password"
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.WRONG_PASSWORD);
                }
            });

            it("NoSuchAccount", async function() {
                try {
                    await node.sdk.rpc.account.sign(
                        message,
                        invalidAddress,
                        "my-password"
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.NO_SUCH_ACCOUNT);
                }
            });
        });

        describe("unlock", function() {
            let address: string;
            beforeEach(async function() {
                address = await node.sdk.rpc.account.create("123");
            });

            it("Ok", async function() {
                await node.sdk.rpc.account.unlock(address, "123");
                await node.sdk.rpc.account.unlock(address, "123", 0);
                await node.sdk.rpc.account.unlock(address, "123", 300);
            });

            it("WrongPassword", async function() {
                try {
                    await node.sdk.rpc.account.unlock(address, "456");
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.WRONG_PASSWORD);
                }
            });

            it("NoSuchAccount", async function() {
                try {
                    await node.sdk.rpc.account.unlock(invalidAddress, "456");
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.NO_SUCH_ACCOUNT);
                }
            });
        });

        describe("changePassword", function() {
            let address: string;
            beforeEach(async function() {
                address = await node.sdk.rpc.account.create("123");
            });

            it("Ok", async function() {
                await node.sdk.rpc.sendRpcRequest("account_changePassword", [
                    address,
                    "123",
                    "456"
                ]);
            });

            it("WrongPassword", async function() {
                try {
                    await node.sdk.rpc.sendRpcRequest(
                        "account_changePassword",
                        [address, "456", "123"]
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.WRONG_PASSWORD);
                }
            });

            it("NoSuchAccount", async function() {
                try {
                    await node.sdk.rpc.sendRpcRequest(
                        "account_changePassword",
                        [invalidAddress, "123", "345"]
                    );
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.NO_SUCH_ACCOUNT);
                }
            });
        });

        afterEach(function() {
            if (this.currentTest!.state === "failed") {
                node.testFailed(this.currentTest!.fullTitle());
            }
        });

        after(async function() {
            await node.clean();
        });
    });
});
