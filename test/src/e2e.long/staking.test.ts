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
import { H256, PlatformAddress } from "codechain-primitives/lib";
import { toHex } from "codechain-sdk/lib/utils";
import "mocha";
import {
    aliceAddress,
    bobAddress,
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address,
    validator0Secret,
    validator1Address,
    validator2Address,
    validator3Address
} from "../helper/constants";
import { PromiseExpect, wait } from "../helper/promise";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("Staking", function() {
    this.timeout(60_000);
    const promiseExpect = new PromiseExpect();
    let nodes: CodeChain[];

    beforeEach(async function() {
        this.timeout(60_000);

        const validatorAddresses = [
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        nodes = validatorAddresses.map(address => {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint-int.json`,
                argv: [
                    "--engine-signer",
                    address.toString(),
                    "--password-path",
                    "test/tendermint/password.json",
                    "--force-sealing",
                    "--no-discovery"
                ],
                additionalKeysPath: "tendermint/keys"
            });
        });
        await Promise.all(nodes.map(node => node.start()));
    });

    async function connectEachOther() {
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[0].connect(nodes[2]),
                nodes[0].connect(nodes[3]),
                nodes[1].connect(nodes[2]),
                nodes[1].connect(nodes[3]),
                nodes[2].connect(nodes[3])
            ])
        );
        await promiseExpect.shouldFulfill(
            "wait peers",
            Promise.all([
                nodes[0].waitPeers(4 - 1),
                nodes[1].waitPeers(4 - 1),
                nodes[2].waitPeers(4 - 1),
                nodes[3].waitPeers(4 - 1)
            ])
        );
    }

    async function getAllStakingInfo() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address,
            aliceAddress,
            bobAddress
        ];
        const amounts = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    nodes[0].sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        ["Account", addr.accountId.toEncodeObject()]
                    )
                )
            )
        );
        const stakeholders = await promiseExpect.shouldFulfill(
            "get customActionData",
            nodes[0].sdk.rpc.engine.getCustomActionData(stakeActionHandlerId, [
                "StakeholderAddresses"
            ])
        );
        return { amounts, stakeholders };
    }

    async function getAllDelegation() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address,
            aliceAddress,
            bobAddress
        ];
        const delegations = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    nodes[0].sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        ["Delegation", addr.accountId.toEncodeObject()]
                    )
                )
            )
        );
        return delegations;
    }

    async function sendStakeToken(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        receiverAddress: PlatformAddress;
        quantity: number;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10 } = params;
        const seq =
            params.seq == null
                ? await nodes[0].sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                1,
                                params.receiverAddress.accountId.toEncodeObject(),
                                params.quantity
                            ])
                        )
                    })
                    .sign({
                        secret: params.senderSecret,
                        seq,
                        fee
                    })
            )
        );
    }

    async function delegateToken(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        receiverAddress: PlatformAddress;
        quantity: number;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10 } = params;
        const seq =
            params.seq == null
                ? await nodes[0].sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                2,
                                params.receiverAddress.accountId.toEncodeObject(),
                                params.quantity
                            ])
                        )
                    })
                    .sign({
                        secret: params.senderSecret,
                        seq,
                        fee
                    })
            )
        );
    }

    async function revokeToken(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        delegateeAddress: PlatformAddress;
        quantity: number;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10 } = params;
        const seq =
            params.seq == null
                ? await nodes[0].sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                3,
                                params.delegateeAddress.accountId.toEncodeObject(),
                                params.quantity
                            ])
                        )
                    })
                    .sign({
                        secret: params.senderSecret,
                        seq,
                        fee
                    })
            )
        );
    }

    it("should have proper initial stake tokens", async function() {
        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000)),
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode([
                    aliceAddress.accountId.toEncodeObject(),
                    faucetAddress.accountId.toEncodeObject(),
                    bobAddress.accountId.toEncodeObject()
                ])
            )
        );
    });

    it("should send stake tokens", async function() {
        await connectEachOther();

        const hash = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
            toHex(RLP.encode(100)),
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        faucetAddress.accountId.toEncodeObject(),
                        aliceAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject(),
                        bobAddress.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    it("doesn't leave zero balance account after transfer", async function() {
        await connectEachOther();

        const hash = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 70000
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            toHex(RLP.encode(70000)),
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        aliceAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject(),
                        bobAddress.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    it("can delegate tokens", async function() {
        await connectEachOther();

        const hash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            toHex(
                RLP.encode([
                    [validator0Address.accountId.toEncodeObject(), 100]
                ])
            ),
            null,
            null,
            null,
            null,
            null,
            null
        ]);
    });

    it("doesn't leave zero balanced account after delegate", async function() {
        await connectEachOther();

        const hash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 70000
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            toHex(
                RLP.encode([
                    [validator0Address.accountId.toEncodeObject(), 70000]
                ])
            ),
            null,
            null,
            null,
            null,
            null,
            null
        ]);
    });

    it("cannot delegate to non-validator", async function() {
        await connectEachOther();
        // give some ccc to pay fee
        const pay1 = await nodes[0].sendPayTx({
            recipient: validator0Address,
            quantity: 100000
        });

        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(pay1.hash()))
        ) {
            await wait(500);
        }
        // give some ccs to delegate.

        const hash1 = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 200
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash1))) {
            await wait(500);
        }

        const blockNumber = await nodes[0].getBestBlockNumber();
        const seq = await nodes[0].sdk.rpc.chain.getSeq(validator0Address);
        const pay = await nodes[0].sendPayTx({
            recipient: faucetAddress,
            secret: validator0Secret,
            quantity: 1,
            seq
        });

        // delegate
        const hash = await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: faucetAddress,
            quantity: 100,
            seq: seq + 1
        });
        await nodes[0].waitBlockNumber(blockNumber + 1);

        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(pay.hash()))
        ) {
            await wait(500);
        }
        const err0 = await nodes[0].sdk.rpc.chain.getErrorHint(hash);
        const err1 = await nodes[1].sdk.rpc.chain.getErrorHint(hash);
        const err2 = await nodes[2].sdk.rpc.chain.getErrorHint(hash);
        const err3 = await nodes[3].sdk.rpc.chain.getErrorHint(hash);
        expect(err0 || err1 || err2 || err3).not.null;
    });

    it("can revoke tokens", async function() {
        await connectEachOther();

        const delegateHash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(delegateHash))
        ) {
            await wait(500);
        }

        const hash = await revokeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            delegateeAddress: validator0Address,
            quantity: 50
        });

        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100 + 50)),
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            toHex(
                RLP.encode([[validator0Address.accountId.toEncodeObject(), 50]])
            ),
            null,
            null,
            null,
            null,
            null,
            null
        ]);
    });

    it("cannot revoke more than delegated", async function() {
        await connectEachOther();

        const delegateHash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(delegateHash))
        ) {
            await wait(500);
        }

        const blockNumber = await nodes[0].getBestBlockNumber();
        const seq = await nodes[0].sdk.rpc.chain.getSeq(faucetAddress);
        const pay = await nodes[0].sendPayTx({
            recipient: faucetAddress,
            secret: faucetSecret,
            quantity: 1,
            seq
        });

        // revoke
        const hash = await revokeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            delegateeAddress: validator0Address,
            quantity: 200,
            seq: seq + 1
        });
        await nodes[0].waitBlockNumber(blockNumber + 1);

        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(pay.hash()))
        ) {
            await wait(500);
        }
        const err0 = await nodes[0].sdk.rpc.chain.getErrorHint(hash);
        const err1 = await nodes[1].sdk.rpc.chain.getErrorHint(hash);
        const err2 = await nodes[2].sdk.rpc.chain.getErrorHint(hash);
        const err3 = await nodes[3].sdk.rpc.chain.getErrorHint(hash);
        expect(err0 || err1 || err2 || err3).not.null;

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            toHex(
                RLP.encode([
                    [validator0Address.accountId.toEncodeObject(), 100]
                ])
            ),
            null,
            null,
            null,
            null,
            null,
            null
        ]);
    });

    it("revoking all should clear delegation", async function() {
        await connectEachOther();

        const delegateHash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (
            !(await nodes[0].sdk.rpc.chain.containsTransaction(delegateHash))
        ) {
            await wait(500);
        }

        const hash = await revokeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            delegateeAddress: validator0Address,
            quantity: 100
        });

        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000)),
            null,
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            null,
            null,
            null,
            null,
            null,
            null
        ]);
    });

    it("get fee in proportion to holding stakes", async function() {
        await connectEachOther();

        // faucet: 70000, alice: 20000, bob: 10000
        const fee = 1000;
        const hash = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }
        // faucet: 20000, alice: 20000, bob: 10000, val0: 50000,

        const blockNumber = await nodes[0].getBestBlockNumber();
        const minCustomCost = require("../scheme/tendermint-int.json").params
            .minCustomCost;

        const oldAliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress
        );
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await nodes[0].sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await nodes[0].sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .minus(fee)
                .toString(10)
        );

        const author = (await nodes[0].sdk.rpc.chain.getBlock(blockNumber))!
            .author;
        const oldValidator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address
        );
        if (author.value === validator0Address.value) {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 5) / 10))
                    .plus(fee)
                    .minus(minCustomCost)
                    .toString(10)
            );
        } else {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 5) / 10))
                    .toString(10)
            );
            const oldAuthorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                author,
                blockNumber - 1
            );
            const authorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                author
            );
            expect(authorBalance.toString(10)).to.be.deep.equal(
                oldAuthorBalance
                    .plus(fee)
                    .minus(minCustomCost)
                    .toString(10)
            );
        }
    });

    it("get fee even if it delegated stakes to other", async function() {
        await connectEachOther();
        // faucet: 70000, alice: 20000, bob: 10000
        const hash1 = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee: 1000
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash1))) {
            await wait(500);
        }

        const fee = 100;
        const payHash = (await nodes[0].sendPayTx({
            recipient: validator0Address,
            quantity: fee
        })).hash();
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(payHash))) {
            await wait(500);
        }

        // faucet: 20000, alice: 20000, bob: 10000, val0: 50000
        const hash2 = await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 50000,
            fee
        });

        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash2))) {
            await wait(500);
        }
        // faucet: 20000, alice: 20000, bob: 10000, val0: 0 (delegated 50000 to val1), val1: 0

        const blockNumber = await nodes[0].getBestBlockNumber();
        const minCustomCost = require("../scheme/tendermint-int.json").params
            .minCustomCost;

        const oldAliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress
        );
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await nodes[0].sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await nodes[0].sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const author = (await nodes[0].sdk.rpc.chain.getBlock(blockNumber))!
            .author;
        const oldValidator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address
        );
        if (author.value === validator0Address.value) {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 5) / 10))
                    .minus(fee)
                    .plus(fee)
                    .minus(minCustomCost)
                    .toString(10)
            );
        } else {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 5) / 10))
                    .minus(fee)
                    .toString(10)
            );

            const oldValidator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
                validator1Address,
                blockNumber - 1
            );
            const validator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
                validator1Address
            );
            if (author.value === validator1Address.value) {
                expect(validator1Balance.toString(10)).to.be.deep.equal(
                    oldValidator1Balance
                        .plus(fee)
                        .minus(minCustomCost)
                        .toString(10)
                );
            } else {
                expect(validator1Balance.toString(10)).to.be.deep.equal(
                    oldValidator1Balance.toString(10)
                );

                const oldAuthorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                    author,
                    blockNumber - 1
                );
                const authorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                    author
                );
                expect(authorBalance.toString(10)).to.be.deep.equal(
                    oldAuthorBalance
                        .plus(fee)
                        .minus(minCustomCost)
                        .toString(10)
                );
            }
        }
    });

    it("get fee even if it delegated stakes to other stakeholder", async function() {
        await connectEachOther();
        // faucet: 70000, alice: 20000, bob: 10000
        const hash1 = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 30000,
            fee: 1000
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash1))) {
            await wait(500);
        }

        // faucet: 40000, alice: 20000, bob: 10000, val0: 30000
        const hash2 = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator1Address,
            quantity: 30000,
            fee: 1000
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash2))) {
            await wait(500);
        }

        const fee = 567;
        const payHash = (await nodes[0].sendPayTx({
            recipient: validator0Address,
            quantity: fee,
            fee
        })).hash();
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(payHash))) {
            await wait(500);
        }

        // faucet: 10000, alice: 20000, bob: 10000, val0: 30000, val1: 30000
        const hash3 = await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 30000,
            fee
        });

        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash3))) {
            await wait(500);
        }
        // faucet: 20000, alice: 20000, bob: 10000, val0: 0 (delegated 30000 to val1), val1: 30000

        const blockNumber = await nodes[0].getBestBlockNumber();
        const minCustomCost = require("../scheme/tendermint-int.json").params
            .minCustomCost;

        const oldAliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await nodes[0].sdk.rpc.chain.getBalance(
            aliceAddress
        );
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await nodes[0].sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await nodes[0].sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await nodes[0].sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const author = (await nodes[0].sdk.rpc.chain.getBlock(blockNumber))!
            .author;
        const oldValidator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address
        );
        const oldValidator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator1Address,
            blockNumber - 1
        );
        const validator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator1Address
        );
        if (author.value === validator0Address.value) {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 3) / 10))
                    .minus(fee)
                    .plus(fee)
                    .minus(minCustomCost)
                    .toString(10)
            );
        } else {
            expect(validator0Balance.toString(10)).to.be.deep.equal(
                oldValidator0Balance
                    .plus(Math.floor((minCustomCost * 3) / 10))
                    .minus(fee)
                    .toString(10)
            );

            const oldValidator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
                validator1Address,
                blockNumber - 1
            );
            const validator1Balance = await nodes[0].sdk.rpc.chain.getBalance(
                validator1Address
            );
            if (author.value === validator1Address.value) {
                expect(validator1Balance.toString(10)).to.be.deep.equal(
                    oldValidator1Balance
                        .plus(Math.floor((minCustomCost * 3) / 10))
                        .plus(fee)
                        .minus(minCustomCost)
                        .toString(10)
                );
            } else {
                expect(validator1Balance.toString(10)).to.be.deep.equal(
                    oldValidator1Balance
                        .plus(Math.floor((minCustomCost * 3) / 10))
                        .toString(10)
                );

                const oldAuthorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                    author,
                    blockNumber - 1
                );
                const authorBalance = await nodes[0].sdk.rpc.chain.getBalance(
                    author
                );
                expect(authorBalance.toString(10)).to.be.deep.equal(
                    oldAuthorBalance
                        .plus(fee)
                        .minus(minCustomCost)
                        .toString(10)
                );
            }
        }
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }
        await Promise.all(nodes.map(node => node.clean()));
        promiseExpect.checkFulfilled();
    });
});

describe("Staking-disable-delegation", function() {
    this.timeout(60_000);
    const promiseExpect = new PromiseExpect();
    let nodes: CodeChain[];

    beforeEach(async function() {
        this.timeout(60_000);

        const validatorAddresses = [
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        nodes = validatorAddresses.map(address => {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint-int.json`,
                argv: [
                    "--engine-signer",
                    address.toString(),
                    "--password-path",
                    "test/tendermint/password.json",
                    "--force-sealing",
                    "--no-discovery"
                ],
                additionalKeysPath: "tendermint/keys",
                env: {
                    ENABLE_DELEGATIONS: "false"
                }
            });
        });
        await Promise.all(nodes.map(node => node.start()));
    });

    async function connectEachOther() {
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[0].connect(nodes[2]),
                nodes[0].connect(nodes[3]),
                nodes[1].connect(nodes[2]),
                nodes[1].connect(nodes[3]),
                nodes[2].connect(nodes[3])
            ])
        );
        await promiseExpect.shouldFulfill(
            "wait peers",
            Promise.all([
                nodes[0].waitPeers(4 - 1),
                nodes[1].waitPeers(4 - 1),
                nodes[2].waitPeers(4 - 1),
                nodes[3].waitPeers(4 - 1)
            ])
        );
    }

    async function getAllStakingInfo() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address,
            aliceAddress,
            bobAddress
        ];
        const amounts = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    nodes[0].sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        ["Account", addr.accountId.toEncodeObject()]
                    )
                )
            )
        );
        const stakeholders = await promiseExpect.shouldFulfill(
            "get customActionData",
            nodes[0].sdk.rpc.engine.getCustomActionData(stakeActionHandlerId, [
                "StakeholderAddresses"
            ])
        );
        return { amounts, stakeholders };
    }

    async function sendStakeToken(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        receiverAddress: PlatformAddress;
        quantity: number;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10 } = params;
        const seq =
            params.seq == null
                ? await nodes[0].sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                1,
                                params.receiverAddress.accountId.toEncodeObject(),
                                params.quantity
                            ])
                        )
                    })
                    .sign({
                        secret: params.senderSecret,
                        seq,
                        fee
                    })
            )
        );
    }

    async function delegateToken(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        receiverAddress: PlatformAddress;
        quantity: number;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10 } = params;
        const seq =
            params.seq == null
                ? await nodes[0].sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                2,
                                params.receiverAddress.accountId.toEncodeObject(),
                                params.quantity
                            ])
                        )
                    })
                    .sign({
                        secret: params.senderSecret,
                        seq,
                        fee
                    })
            )
        );
    }

    it("should send stake tokens", async function() {
        await connectEachOther();

        const hash = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        while (!(await nodes[0].sdk.rpc.chain.containsTransaction(hash))) {
            await wait(500);
        }

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
            toHex(RLP.encode(100)),
            null,
            null,
            null,
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        faucetAddress.accountId.toEncodeObject(),
                        aliceAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject(),
                        bobAddress.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    it("cannot delegate tokens", async function() {
        await connectEachOther();
        const hash = await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });
        const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
        await nodes[0].waitBlockNumber(blockNumber + 1);

        const err0 = await nodes[0].sdk.rpc.chain.getErrorHint(hash);
        const err1 = await nodes[1].sdk.rpc.chain.getErrorHint(hash);
        const err2 = await nodes[2].sdk.rpc.chain.getErrorHint(hash);
        const err3 = await nodes[3].sdk.rpc.chain.getErrorHint(hash);
        expect(err0 || err1 || err2 || err3).not.null;
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }
        await Promise.all(nodes.map(node => node.clean()));
        promiseExpect.checkFulfilled();
    });
});
