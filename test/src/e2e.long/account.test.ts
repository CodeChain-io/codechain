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
import { xor128 } from "seedrandom";
import { makeRandomH256, makeRandomPassphrase } from "../helper/random";
import CodeChain from "../helper/spawn";

describe("account", function() {
    const BASE = 0;
    describe("account scenario test", function() {
        let node: CodeChain;
        const testSize = 30;
        const randomTestSize = 100;

        beforeEach(async function() {
            node = new CodeChain({ base: BASE });
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
                            if (passphrase === undefined) {
                                break;
                            }

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
            if (this.currentTest!.state === "failed") {
                node.testFailed(this.currentTest!.fullTitle());
            }
            await node.clean();
        });
    });
});
