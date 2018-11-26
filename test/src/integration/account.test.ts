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
import { ERROR, errorMatcher } from "../helper/error";
import { makeRandomH256, makeRandomPassphrase } from "../helper/random";
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
                    expect(e).to.satisfy(errorMatcher(ERROR.KEY_ERROR));
                }
            });

            it("AlreadyExists", async function() {
                try {
                    await node.sdk.rpc.account.importRaw(randomSecret);
                    await node.sdk.rpc.account.importRaw(randomSecret);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(errorMatcher(ERROR.ALREADY_EXISTS));
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
                    expect(e).to.satisfy(errorMatcher(ERROR.WRONG_PASSWORD));
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
                    expect(e).to.satisfy(errorMatcher(ERROR.NO_SUCH_ACCOUNT));
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
                    expect(e).to.satisfy(errorMatcher(ERROR.WRONG_PASSWORD));
                }
            });

            it("NoSuchAccount", async function() {
                try {
                    await node.sdk.rpc.account.unlock(invalidAddress, "456");
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(errorMatcher(ERROR.NO_SUCH_ACCOUNT));
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
                    expect(e).to.satisfy(errorMatcher(ERROR.WRONG_PASSWORD));
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
                    expect(e).to.satisfy(errorMatcher(ERROR.NO_SUCH_ACCOUNT));
                }
            });
        });

        after(async function() {
            await node.clean();
        });
    });

    describe("account scenario test", function() {
        let node: CodeChain;
        const testSize = 30;
        const unlockTestSize = 15;
        const randomTestSize = 100;

        beforeEach(async function() {
            node = new CodeChain();
            await node.start();
        });

        it(`Scenario #1: getList & create ${testSize} accounts`, async function() {
            let list = await node.sdk.rpc.account.getList();
            expect(list.length).to.equal(0);

            const accountList = [];
            for (let i = 0; i < testSize; i++) {
                const passphrase = makeRandomPassphrase();
                const address = await node.sdk.rpc.account.create(passphrase);
                accountList.push({ address, passphrase });

                list = await node.sdk.rpc.account.getList();
                expect(list.length).to.equal(i + 1);
                for (let j = 0; j <= i; j++) {
                    expect(list).to.include(accountList[i].address);
                }
            }
        }).timeout(500 * testSize + 5000);

        it(`Scenario #2: importRaw ${testSize} accounts`, async function() {
            for (let i = 0; i < testSize; i++) {
                const randomSecret = node.sdk.util.generatePrivateKey();
                const account = node.sdk.util.getAccountIdFromPrivate(
                    randomSecret
                );
                const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                    account,
                    { networkId: "tc" }
                );
                const randomPassphrase = makeRandomPassphrase();

                expect(
                    await node.sdk.rpc.account.importRaw(
                        randomSecret,
                        randomPassphrase
                    )
                ).to.equal(address.toString());
            }
        }).timeout(500 * testSize + 5000);

        it(`Scenario #3: unlock 1 second ${unlockTestSize} times and check working well with sign`, async function() {
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
                const { r, s, v } = node.sdk.util.signEcdsa(message, secret);
                await node.sdk.rpc.account.unlock(address, passphrase, 1);

                for (let j = 0; j <= 2; j++) {
                    try {
                        const signature = await node.sdk.rpc.account.sign(
                            message,
                            address
                        );
                        expect(signature).to.include(r);
                        expect(signature).to.include(s);
                        expect(signature).to.include(v);
                    } catch (e) {
                        expect.fail();
                    }
                    await wait(100);
                }
                await wait(1000 - 100 * 3);

                try {
                    await node.sdk.rpc.account.sign(message, address);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(errorMatcher(ERROR.NOT_UNLOCKED));
                }
            }
        }).timeout(2000 * unlockTestSize + 5000);

        it(`Scenario #X: random agent runs ${randomTestSize} commands`, async function() {
            enum Action {
                Create = 0,
                ImportRaw,
                GetList,
                UnlockForever,
                Sign,
                ChangePassword
            }
            const accountList: {
                address: string;
                passphrase?: string;
                secret?: string;
            }[] = [];
            const actionNumber = randomTestSize;
            const rng = xor128("Random account test");

            function getRandomIndex(size: number) {
                const randomValue = rng.int32();
                return Math.abs(randomValue) % size;
            }

            for (let test = 0; test < actionNumber; test++) {
                const randomAction =
                    accountList.length === 0
                        ? getRandomIndex(3)
                        : getRandomIndex(6);
                switch (randomAction) {
                    case Action.Create:
                        {
                            const passphrase = makeRandomPassphrase();
                            const address = await node.sdk.rpc.account.create(
                                passphrase
                            );
                            accountList.push({ address, passphrase });
                        }
                        break;
                    case Action.ImportRaw:
                        {
                            const secret = node.sdk.util.generatePrivateKey();
                            const account = node.sdk.util.getAccountIdFromPrivate(
                                secret
                            );
                            const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                                account,
                                { networkId: "tc" }
                            ).toString();
                            const passphrase = makeRandomPassphrase();
                            expect(
                                await node.sdk.rpc.account.importRaw(
                                    secret,
                                    passphrase
                                )
                            ).to.equal(address);
                            accountList.push({
                                address,
                                passphrase,
                                secret
                            });
                        }
                        break;
                    case Action.GetList:
                        {
                            const list = await node.sdk.rpc.account.getList();
                            expect(list.length).to.equal(accountList.length);
                            accountList.forEach(value => {
                                expect(list).to.include(value.address);
                            });
                        }
                        break;
                    case Action.UnlockForever:
                        {
                            const randomIdx = getRandomIndex(
                                accountList.length
                            );
                            const { address, passphrase } = accountList[
                                randomIdx
                            ];
                            await node.sdk.rpc.account.unlock(
                                address,
                                passphrase,
                                0
                            );
                            accountList[randomIdx].passphrase = undefined;
                        }
                        break;
                    case Action.Sign:
                        {
                            const accountListWithSecret = accountList.filter(
                                val => val.secret !== undefined
                            );
                            if (accountListWithSecret.length === 0) {
                                test--;
                                continue;
                            }
                            const randomIdx = getRandomIndex(
                                accountListWithSecret.length
                            );
                            const {
                                address,
                                passphrase,
                                secret
                            } = accountListWithSecret[randomIdx];
                            const message = makeRandomH256();

                            const { r, s, v } = node.sdk.util.signEcdsa(
                                message,
                                secret!
                            );
                            const signature = await node.sdk.rpc.account.sign(
                                message,
                                address,
                                passphrase
                            );
                            expect(signature).to.include(r);
                            expect(signature).to.include(s);
                            expect(signature).to.include(v);
                        }
                        break;
                    case Action.ChangePassword:
                        {
                            const randomIdx = getRandomIndex(
                                accountList.length
                            );
                            const { address, passphrase } = accountList[
                                randomIdx
                            ];
                            if (passphrase === undefined) break;

                            const nextPassphrase = makeRandomPassphrase();
                            await node.sdk.rpc.sendRpcRequest(
                                "account_changePassword",
                                [address, passphrase, nextPassphrase]
                            );
                            accountList[randomIdx].passphrase = nextPassphrase;
                        }
                        break;
                }
            }
        }).timeout(500 * randomTestSize + 5000);

        afterEach(async function() {
            await node.clean();
        });
    });
});
