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
import {
    makeRandomH256,
    makeRandomPassphrase,
    getRandomIndex
} from "../helper/random";

const ERROR = {
    KEY_ERROR: {
        code: -32041,
        data: expect.anything(),
        message: expect.anything()
    },
    ALREADY_EXISTS: {
        code: -32042,
        data: expect.anything(),
        message: expect.anything()
    },
    WRONG_PASSWORD: {
        code: -32043,
        data: expect.anything(),
        message: expect.anything()
    },
    NO_SUCH_ACCOUNT: {
        code: -32044,
        data: expect.anything(),
        message: expect.anything()
    },
    NOT_UNLOCKED: {
        code: -32045,
        data: expect.anything(),
        message: expect.anything()
    },
    INVALID_PARAMS: {
        code: -32602,
        message: expect.anything()
    }
};

describe("account", () => {
    const noSuchAccount = "tccqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqj5aqu5";

    describe("account base test", () => {
        let node: CodeChain;
        beforeAll(async () => {
            node = new CodeChain();
            await node.start();
        });

        test("getList", async () => {
            await expect(node.sdk.rpc.account.getList()).resolves.toEqual(
                expect.anything()
            );
        });

        test("create", async () => {
            expect(await node.sdk.rpc.account.create()).toEqual(
                expect.anything()
            );
            expect(await node.sdk.rpc.account.create("my-password")).toEqual(
                expect.anything()
            );
        });

        describe("importRaw", () => {
            let randomSecret: string;
            beforeEach(() => {
                randomSecret = node.sdk.util.generatePrivateKey();
            });

            test("Ok", async () => {
                const account = node.sdk.util.getAccountIdFromPrivate(
                    randomSecret
                );
                const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                    account,
                    { networkId: "tc" }
                );
                expect(
                    await node.sdk.rpc.account.importRaw(randomSecret)
                ).toEqual(address.toString());
            });

            test("KeyError", done => {
                const invalidSecret =
                    "0000000000000000000000000000000000000000000000000000000000000000";
                node.sdk.rpc.account
                    .importRaw(invalidSecret)
                    .then(done.fail)
                    .catch(e => {
                        expect(e).toEqual(ERROR.KEY_ERROR);
                        done();
                    });
            });

            test("AlreadyExists", async done => {
                node.sdk.rpc.account.importRaw(randomSecret).then(() => {
                    node.sdk.rpc.account
                        .importRaw(randomSecret)
                        .then(() => done.fail())
                        .catch(e => {
                            expect(e).toEqual(ERROR.ALREADY_EXISTS);
                            done();
                        });
                });
            });
        });

        describe("sign", () => {
            const message =
                "0000000000000000000000000000000000000000000000000000000000000000";
            let address;
            let secret;
            beforeEach(async () => {
                secret = node.sdk.util.generatePrivateKey();
                address = await node.sdk.rpc.account.importRaw(
                    secret,
                    "my-password"
                );
            });

            test("Ok", async () => {
                const { r, s, v } = node.sdk.util.signEcdsa(message, secret);
                const signature = await node.sdk.rpc.account.sign(
                    message,
                    address,
                    "my-password"
                );
                expect(signature).toContain(r);
                expect(signature).toContain(s);
                expect(signature).toContain(v);
            });

            test("WrongPassword", async done => {
                node.sdk.rpc.account
                    .sign(message, address, "wrong-password")
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.WRONG_PASSWORD);
                        done();
                    });
            });

            test("NoSuchAccount", async done => {
                node.sdk.rpc.account
                    .sign(message, noSuchAccount, "my-password")
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.NO_SUCH_ACCOUNT);
                        done();
                    });
            });
        });

        describe("unlock", () => {
            let address;
            beforeEach(async () => {
                address = await node.sdk.rpc.account.create("123");
            });

            test("Ok", async () => {
                await node.sdk.rpc.account.unlock(address, "123");
                await node.sdk.rpc.account.unlock(address, "123", 0);
                await node.sdk.rpc.account.unlock(address, "123", 300);
            });

            test("WrongPassword", async done => {
                node.sdk.rpc.account
                    .unlock(address, "456")
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.WRONG_PASSWORD);
                        done();
                    });
            });

            test("NoSuchAccount", async done => {
                node.sdk.rpc.account
                    .unlock(noSuchAccount)
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.NO_SUCH_ACCOUNT);
                        done();
                    });
            });
        });

        describe("changePassword", () => {
            let address;
            beforeEach(async () => {
                address = await node.sdk.rpc.account.create("123");
            });

            test("Ok", async () => {
                await node.sdk.rpc.sendRpcRequest("account_changePassword", [
                    address,
                    "123",
                    "456"
                ]);
            });

            test("WrongPassword", async done => {
                await node.sdk.rpc
                    .sendRpcRequest("account_changePassword", [
                        address,
                        "456",
                        "123"
                    ])
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.WRONG_PASSWORD);
                        done();
                    });
            });

            test("NoSuchAccount", async done => {
                node.sdk.rpc
                    .sendRpcRequest("account_changePassword", [
                        noSuchAccount,
                        "123",
                        "456"
                    ])
                    .then(() => done.fail())
                    .catch(e => {
                        expect(e).toEqual(ERROR.NO_SUCH_ACCOUNT);
                        done();
                    });
            });
        });

        afterAll(async () => {
            await node.clean();
        });
    });

    describe("account scenario test", () => {
        let node: CodeChain;
        const testSize = 30;
        const unlockTestSize = 15;
        const randomTestSize = 100;

        beforeEach(async () => {
            node = new CodeChain();
            await node.start();
        });

        test(
            `Scenario #1: getList & create ${testSize} accounts`,
            async done => {
                let list = await node.sdk.rpc.account.getList();
                expect(list.length).toEqual(0);

                const accountList = [];
                for (let i = 0; i < testSize; i++) {
                    const passphrase = makeRandomPassphrase();
                    const address = await node.sdk.rpc.account.create(
                        passphrase
                    );
                    accountList.push({ address, passphrase });

                    list = await node.sdk.rpc.account.getList();
                    expect(list.length).toEqual(i + 1);
                    for (let j = 0; j <= i; j++) {
                        expect(list).toContain(accountList[i].address);
                    }
                }

                done();
            },
            500 * testSize + 5000
        );

        test(
            `Scenario #2: importRaw ${testSize} accounts`,
            async () => {
                for (let i = 0; i < testSize; i++) {
                    const randomSecret = node.sdk.util.generatePrivateKey();
                    const account = node.sdk.util.getAccountIdFromPrivate(
                        randomSecret
                    );
                    const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                        account
                    );
                    const randomPassphrase = makeRandomPassphrase();

                    expect(
                        await node.sdk.rpc.account.importRaw(
                            randomSecret,
                            randomPassphrase
                        )
                    ).toEqual(address.toString());
                }
            },
            500 * testSize + 5000
        );

        test(
            `Scenario #3: unlock 1 second ${unlockTestSize} times and check working well with sign`,
            async done => {
                const secret = node.sdk.util.generatePrivateKey();
                const account = node.sdk.util.getAccountIdFromPrivate(secret);
                const address = node.sdk.core.classes.PlatformAddress.fromAccountId(
                    account
                );
                const passphrase = makeRandomPassphrase();
                await node.sdk.rpc.account.importRaw(secret, passphrase);

                for (let i = 0; i < unlockTestSize; i++) {
                    const message = makeRandomH256();
                    const { r, s, v } = node.sdk.util.signEcdsa(
                        message,
                        secret
                    );
                    await node.sdk.rpc.account.unlock(address, passphrase, 1);

                    for (let j = 0; j <= 2; j++) {
                        try {
                            const signature = await node.sdk.rpc.account.sign(
                                message,
                                address,
                                null
                            );
                            expect(signature).toContain(r);
                            expect(signature).toContain(s);
                            expect(signature).toContain(v);
                        } catch (e) {
                            done.fail();
                        }
                        await wait(100);
                    }
                    await wait(1000 - 100 * 3);

                    try {
                        await node.sdk.rpc.account.sign(message, address, null);
                        done.fail();
                    } catch (e) {
                        expect(e).toEqual(ERROR.NOT_UNLOCKED);
                    }
                }
                done();
            },
            2000 * unlockTestSize + 5000
        );

        test(
            `Scenario #X: random agent runs ${randomTestSize} commands`,
            async done => {
                enum Action {
                    Create = 0,
                    ImportRaw,
                    GetList,
                    UnlockForever,
                    Sign,
                    ChangePassword
                }
                const accountList = [];
                const actionNumber = randomTestSize;

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
                                ).toEqual(address);
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
                                expect(list.length).toEqual(accountList.length);
                                accountList.forEach(value => {
                                    expect(list).toContain(value.address);
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
                                accountList[randomIdx].passphrase = null;
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
                                    secret
                                );
                                const signature = await node.sdk.rpc.account.sign(
                                    message,
                                    address,
                                    passphrase
                                );
                                expect(signature).toContain(r);
                                expect(signature).toContain(s);
                                expect(signature).toContain(v);
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
                                if (passphrase === null) break;

                                const nextPassphrase = makeRandomPassphrase();
                                await node.sdk.rpc.sendRpcRequest(
                                    "account_changePassword",
                                    [address, passphrase, nextPassphrase]
                                );
                                accountList[
                                    randomIdx
                                ].passphrase = nextPassphrase;
                            }
                            break;
                    }
                }
                done();
            },
            500 * randomTestSize + 5000
        );

        afterEach(async () => {
            await node.clean();
        });
    });
});
