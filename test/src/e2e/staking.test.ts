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
    validator1Address
} from "../helper/constants";
import { PromiseExpect } from "../helper/promise";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("Staking", function() {
    const promiseExpect = new PromiseExpect();
    const chain = `${__dirname}/../scheme/solo-block-reward-50.json`;
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            chain,
            argv: ["--author", validator0Address.toString(), "--force-sealing"]
        });
        await node.start();
    });

    async function getAllStakingInfo() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            aliceAddress,
            bobAddress
        ];
        const amounts = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    node.sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        ["Account", addr.accountId.toEncodeObject()]
                    )
                )
            )
        );
        const stakeholders = await promiseExpect.shouldFulfill(
            "get customActionData",
            node.sdk.rpc.engine.getCustomActionData(stakeActionHandlerId, [
                "StakeholderAddresses"
            ])
        );
        return { amounts, stakeholders };
    }

    async function getAllDelegation() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            aliceAddress,
            bobAddress
        ];
        const delegations = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    node.sdk.rpc.engine.getCustomActionData(
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
                ? await node.sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
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
                ? await node.sdk.rpc.chain.getSeq(params.senderAddress)
                : params.seq;

        return promiseExpect.shouldFulfill(
            "sendSignTransaction",
            node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
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

    it("should have proper initial stake tokens", async function() {
        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000)),
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
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
            toHex(RLP.encode(100)),
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
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 70000
        });

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            toHex(RLP.encode(70000)),
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
        await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(70000 - 100)),
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
            null
        ]);
    });

    it("doesn't leave zero balanced account after delegate", async function() {
        await delegateToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 70000
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
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
            null
        ]);
    });

    it("get fee in proportion to holding stakes", async function() {
        // faucet: 70000, alice: 20000, bob: 10000
        const fee = 1000;
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee
        });
        // faucet: 20000, alice: 20000, bob: 10000, val0: 50000,

        const blockNumber = await node.getBestBlockNumber();
        const blockReward = parseInt(
            require(chain).engine.solo.params.blockReward as string,
            16
        );
        const minCustomCost = require(chain).params.minCustomCost;

        const oldAliceBalance = await node.sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await node.sdk.rpc.chain.getBalance(aliceAddress);
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .minus(fee)
                .toString(10)
        );

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance.toString(10)).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 5) / 10))
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
                .toString(10)
        );
    });

    it("get fee even if it delegated stakes to other", async function() {
        // faucet: 70000, alice: 20000, bob: 10000
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee: 1000
        });

        const fee = 100;
        await node.sendPayTx({
            recipient: validator0Address,
            quantity: fee
        });

        // faucet: 20000, alice: 20000, bob: 10000, val0: 50000
        await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 50000,
            fee
        });
        // faucet: 20000, alice: 20000, bob: 10000, val0: 0 (delegated 50000 to val1), val1: 0

        const blockNumber = await node.getBestBlockNumber();
        const blockReward = parseInt(
            require(chain).engine.solo.params.blockReward as string,
            16
        );
        const minCustomCost = require(chain).params.minCustomCost as number;

        const oldAliceBalance = await node.sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await node.sdk.rpc.chain.getBalance(aliceAddress);
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance.toString(10)).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 5) / 10))
                .minus(fee)
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
                .toString(10)
        );

        const oldValidator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address,
            blockNumber - 1
        );
        const validator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address
        );

        expect(validator1Balance.toString(10)).to.be.deep.equal(
            oldValidator1Balance.toString(10)
        );
    });

    it("get fee even if it delegated stakes to other stakeholder", async function() {
        // faucet: 70000, alice: 20000, bob: 10000
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 30000,
            fee: 1000
        });

        // faucet: 40000, alice: 20000, bob: 10000, val0: 30000
        await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator1Address,
            quantity: 30000,
            fee: 1000
        });

        const fee = 567;
        await node.sendPayTx({
            recipient: validator0Address,
            quantity: fee,
            fee
        });

        // faucet: 10000, alice: 20000, bob: 10000, val0: 30000, val1: 30000
        await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 30000,
            fee
        });
        // faucet: 20000, alice: 20000, bob: 10000, val0: 0 (delegated 30000 to val1), val1: 30000

        const blockNumber = await node.getBestBlockNumber();
        const blockReward = parseInt(
            require(chain).engine.solo.params.blockReward as string,
            16
        );
        const minCustomCost = require(chain).params.minCustomCost as number;

        const oldAliceBalance = await node.sdk.rpc.chain.getBalance(
            aliceAddress,
            blockNumber - 1
        );
        const aliceBalance = await node.sdk.rpc.chain.getBalance(aliceAddress);
        expect(aliceBalance.toString(10)).to.be.deep.equal(
            oldAliceBalance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .toString(10)
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance.toString(10)).to.be.deep.equal(
            oldBobBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldFaucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance.toString(10)).to.be.deep.equal(
            oldFaucetBalance
                .plus(Math.floor((minCustomCost * 1) / 10))
                .toString(10)
        );

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance.toString(10)).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 3) / 10))
                .minus(fee)
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
                .toString(10)
        );

        const oldValidator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address,
            blockNumber - 1
        );
        const validator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address
        );
        expect(validator1Balance.toString(10)).to.be.deep.equal(
            oldValidator1Balance
                .plus(Math.floor((minCustomCost * 3) / 10))
                .toString(10)
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
        promiseExpect.checkFulfilled();
    });
});
