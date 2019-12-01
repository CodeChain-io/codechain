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
    aliceSecret,
    bobAddress,
    carolAddress,
    daveAddress,
    faucetAddress,
    stakeActionHandlerId,
    validator0Address,
    validator0Secret,
    validator1Address,
    validator1Secret
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
        await node.sendPayTx({ recipient: aliceAddress, quantity: 100_000 });
        await node.sendPayTx({
            recipient: validator1Address,
            quantity: 100_000
        });
    });

    async function getAllStakingInfo() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            validator1Address,
            aliceAddress,
            bobAddress,
            carolAddress,
            daveAddress
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
            validator1Address,
            aliceAddress,
            bobAddress,
            carolAddress,
            daveAddress
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

    async function selfNominate(params: {
        senderAddress: PlatformAddress;
        senderSecret: string;
        deposit: number;
        metadata: Buffer | null;
        fee?: number;
        seq?: number;
    }): Promise<H256> {
        const { fee = 10, deposit, metadata } = params;
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
                        bytes: Buffer.from(RLP.encode([4, deposit, metadata]))
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
            null,
            null,
            null,
            toHex(RLP.encode(40000)),
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode([
                    aliceAddress.accountId.toEncodeObject(),
                    carolAddress.accountId.toEncodeObject(),
                    daveAddress.accountId.toEncodeObject(),
                    bobAddress.accountId.toEncodeObject()
                ])
            )
        );
    });

    it("should send stake tokens", async function() {
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            toHex(RLP.encode(100)),
            null,
            toHex(RLP.encode(40000 - 100)),
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        aliceAddress.accountId.toEncodeObject(),
                        carolAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject(),
                        daveAddress.accountId.toEncodeObject(),
                        bobAddress.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    it("doesn't leave zero balance account after transfer", async function() {
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 40000
        });

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            toHex(RLP.encode(40000)),
            null,
            null,
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        carolAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject(),
                        daveAddress.accountId.toEncodeObject(),
                        bobAddress.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    it("can delegate tokens", async function() {
        await selfNominate({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            deposit: 0,
            metadata: null
        });

        await delegateToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            toHex(RLP.encode(40000 - 100)),
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            null,
            null,
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
        await selfNominate({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            deposit: 0,
            metadata: null
        });

        await delegateToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 40000
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            null,
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            null,
            null,
            toHex(
                RLP.encode([
                    [validator0Address.accountId.toEncodeObject(), 40000]
                ])
            ),
            null,
            null,
            null
        ]);
    });

    it("can revoke tokens", async function() {
        await selfNominate({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            deposit: 0,
            metadata: null
        });

        await delegateToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        await revokeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            delegateeAddress: validator0Address,
            quantity: 50
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            toHex(RLP.encode(40000 - 100 + 50)),
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            null,
            null,
            toHex(
                RLP.encode([[validator0Address.accountId.toEncodeObject(), 50]])
            ),
            null,
            null,
            null
        ]);
    });

    it("cannot revoke more than delegated", async function() {
        await selfNominate({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            deposit: 0,
            metadata: null
        });

        await delegateToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        await node.sdk.rpc.devel.stopSealing();
        await node.sendPayTx({
            recipient: faucetAddress,
            secret: validator0Secret,
            quantity: 1,
            seq: await node.sdk.rpc.chain.getSeq(validator0Address)
        });
        const hash = await revokeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            delegateeAddress: validator0Address,
            quantity: 200
        });
        await node.sdk.rpc.devel.startSealing();

        expect(await node.sdk.rpc.chain.getErrorHint(hash)).not.to.be.null;

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            toHex(RLP.encode(40000 - 100)),
            toHex(RLP.encode(30000)),
            toHex(RLP.encode(20000)),
            toHex(RLP.encode(10000))
        ]);

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            null,
            null,
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

    it("revoking all should clear delegation", async function() {
        await selfNominate({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            deposit: 0,
            metadata: null
        });

        await delegateToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 100
        });

        await revokeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            delegateeAddress: validator0Address,
            quantity: 100
        });

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            null,
            null,
            null,
            toHex(RLP.encode(40000)),
            toHex(RLP.encode(30000)),
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
        // alice: 40000, bob: 30000, carol: 20000, dave: 10000
        const fee = 1000;
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 20000,
            fee
        });
        // alice: 20000, bob: 30000, carol: 20000, dave: 10000, val0: 20000,

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
        expect(aliceBalance).to.be.deep.equal(
            oldAliceBalance
                .minus(fee)
                .plus(Math.floor((minCustomCost * 2) / 10))
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance).to.be.deep.equal(
            oldBobBalance.plus(Math.floor((minCustomCost * 3) / 10))
        );

        const oldCarolBalance = await node.sdk.rpc.chain.getBalance(
            carolAddress,
            blockNumber - 1
        );
        const carolBalance = await node.sdk.rpc.chain.getBalance(carolAddress);
        expect(carolBalance).to.be.deep.equal(
            oldCarolBalance.plus(Math.floor((minCustomCost * 2) / 10))
        );

        const oldDaveBalance = await node.sdk.rpc.chain.getBalance(
            daveAddress,
            blockNumber - 1
        );
        const daveBalance = await node.sdk.rpc.chain.getBalance(daveAddress);
        expect(daveBalance).to.be.deep.equal(
            oldDaveBalance.plus(Math.floor((minCustomCost * 1) / 10))
        );

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
        );
    });

    it("get fee even if it delegated stakes to other", async function() {
        await selfNominate({
            senderAddress: validator1Address,
            senderSecret: validator1Secret,
            deposit: 0,
            metadata: null
        });

        // alice: 40000, bob: 30000, carol 20000, dave: 10000
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 20000,
            fee: 1000
        });

        const fee = 100;
        await node.sendPayTx({
            recipient: validator0Address,
            quantity: fee
        });

        // alice: 20000, bob: 30000, carol 20000, dave: 10000, val0: 20000
        await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 20000,
            fee
        });
        // alice: 20000, bob: 30000, carol 20000, dave: 10000, val0: 0 (delegated 20000 to val1), val1: 0

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            toHex(
                RLP.encode([
                    [validator1Address.accountId.toEncodeObject(), 20000]
                ])
            ),
            null,
            null,
            null,
            null,
            null
        ]);

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
        expect(aliceBalance).to.be.deep.equal(
            oldAliceBalance.plus(Math.floor((minCustomCost * 2) / 10))
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance).to.be.deep.equal(
            oldBobBalance.plus(Math.floor((minCustomCost * 3) / 10))
        );

        const oldCarolBalance = await node.sdk.rpc.chain.getBalance(
            carolAddress,
            blockNumber - 1
        );
        const carolBalance = await node.sdk.rpc.chain.getBalance(carolAddress);
        expect(carolBalance).to.be.deep.equal(
            oldCarolBalance.plus(Math.floor((minCustomCost * 2) / 10))
        );

        const oldDaveBalance = await node.sdk.rpc.chain.getBalance(
            daveAddress,
            blockNumber - 1
        );
        const daveBalance = await node.sdk.rpc.chain.getBalance(daveAddress);
        expect(daveBalance).to.be.deep.equal(
            oldDaveBalance.plus(Math.floor((minCustomCost * 1) / 10))
        );

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .minus(fee)
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
        );

        const oldValidator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address,
            blockNumber - 1
        );
        const validator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address
        );
        expect(validator1Balance).to.be.deep.equal(oldValidator1Balance);
    });

    it("get fee even if it delegated stakes to other stakeholder", async function() {
        await selfNominate({
            senderAddress: validator1Address,
            senderSecret: validator1Secret,
            deposit: 0,
            metadata: null
        });

        // alice: 40000, bob: 30000, carol 20000, dave: 10000
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator0Address,
            quantity: 20000,
            fee: 1000
        });

        // alice: 20000, bob: 30000, carol 20000, dave: 10000, val0 20000
        await sendStakeToken({
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: validator1Address,
            quantity: 10000,
            fee: 1000
        });
        // alice: 10000, bob: 30000, carol 20000, dave: 10000, val0 20000, val1: 10000

        const fee = 567;
        await node.sendPayTx({
            recipient: validator0Address,
            quantity: fee,
            fee
        });

        await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 20000,
            fee
        });
        // alice: 10000, bob: 30000, carol 20000, dave: 10000, val0 20000 (delegated 20000 to val1), val1: 10000

        const delegations = await getAllDelegation();
        expect(delegations).to.be.deep.equal([
            null,
            toHex(
                RLP.encode([
                    [validator1Address.accountId.toEncodeObject(), 20000]
                ])
            ),
            null,
            null,
            null,
            null,
            null
        ]);

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
        expect(aliceBalance).to.be.deep.equal(
            oldAliceBalance.plus(Math.floor((minCustomCost * 1) / 10))
        );

        const oldBobBalance = await node.sdk.rpc.chain.getBalance(
            bobAddress,
            blockNumber - 1
        );
        const bobBalance = await node.sdk.rpc.chain.getBalance(bobAddress);
        expect(bobBalance).to.be.deep.equal(
            oldBobBalance.plus(Math.floor((minCustomCost * 3) / 10))
        );

        const oldFaucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress,
            blockNumber - 1
        );
        const faucetBalance = await node.sdk.rpc.chain.getBalance(
            faucetAddress
        );
        expect(faucetBalance).to.be.deep.equal(oldFaucetBalance);

        const oldValidator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address,
            blockNumber - 1
        );
        const validator0Balance = await node.sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(validator0Balance).to.be.deep.equal(
            oldValidator0Balance
                .plus(Math.floor((minCustomCost * 2) / 10))
                .minus(fee)
                .plus(fee)
                .minus(minCustomCost)
                .plus(blockReward)
        );

        const oldValidator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address,
            blockNumber - 1
        );
        const validator1Balance = await node.sdk.rpc.chain.getBalance(
            validator1Address
        );
        expect(validator1Balance).to.be.deep.equal(
            oldValidator1Balance.plus(Math.floor((minCustomCost * 1) / 10))
        );
    });

    it("Shouldn't accept regular key to self nominate", async function() {
        const privKey = node.sdk.util.generatePrivateKey();
        const pubKey = node.sdk.util.getPublicFromPrivate(privKey);

        await node.setRegularKey(pubKey, {
            seq: await node.sdk.rpc.chain.getSeq(validator0Address),
            secret: validator0Secret
        });

        await node.sendSignedTransactionExpectedToFail(() =>
            selfNominate({
                senderAddress: validator0Address,
                senderSecret: privKey,
                deposit: 0,
                metadata: null
            })
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
        promiseExpect.checkFulfilled();
    });
});
