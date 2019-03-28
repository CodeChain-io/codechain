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

import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import { toHex } from "codechain-sdk/lib/utils";
import { PlatformAddress, H256, U64 } from "codechain-primitives/lib";
import "mocha";
import {
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
            validator3Address
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
            validator3Address
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

    it("should have proper initial stake tokens", async function() {
        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(100000)),
            null,
            null,
            null,
            null
        ]);

        expect(stakeholders).to.be.equal(
            toHex(RLP.encode([faucetAddress.accountId.toEncodeObject()]))
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
        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash))) {
            await wait(500);
        }

        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(100000 - 100)),
            toHex(RLP.encode(100)),
            null,
            null,
            null
        ]);
        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        faucetAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject()
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
        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash))) {
            await wait(500);
        }

        const { amounts } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(100000 - 100)),
            null,
            null,
            null,
            null
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
            !(await nodes[0].sdk.rpc.chain.containTransaction(pay1.hash()))
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
        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash1))) {
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

        while (!(await nodes[0].sdk.rpc.chain.containTransaction(pay.hash()))) {
            await wait(500);
        }
        const err0 = await nodes[0].sdk.rpc.chain.getErrorHint(hash);
        const err1 = await nodes[1].sdk.rpc.chain.getErrorHint(hash);
        const err2 = await nodes[2].sdk.rpc.chain.getErrorHint(hash);
        const err3 = await nodes[3].sdk.rpc.chain.getErrorHint(hash);
        expect(err0 || err1 || err2 || err3).not.null;
    });

    it("get fee in proportion to holding stakes", async function() {
        await connectEachOther();
        // faucet: 100000
        const hash = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee: 1000
        });
        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash))) {
            await wait(500);
        }
        // faucet: 50000, val0: 50000,

        const balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(balance).to.be.deep.equal(new U64(1000 / 2));
    });

    it("get fee even if it delegated stakes to other", async function() {
        await connectEachOther();
        // faucet: 100000
        const hash1 = await sendStakeToken({
            senderAddress: faucetAddress,
            senderSecret: faucetSecret,
            receiverAddress: validator0Address,
            quantity: 50000,
            fee: 1000
        });
        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash1))) {
            await wait(500);
        }
        // faucet: 50000, val0: 50000,
        const hash2 = await delegateToken({
            senderAddress: validator0Address,
            senderSecret: validator0Secret,
            receiverAddress: validator1Address,
            quantity: 50000,
            fee: 100
        });

        while (!(await nodes[0].sdk.rpc.chain.containTransaction(hash2))) {
            await wait(500);
        }
        // faucet: 50000, val0: 0 (delegated 50000 to val1), val1: 0
        const balance = await nodes[0].sdk.rpc.chain.getBalance(
            validator0Address
        );
        expect(balance).to.be.deep.equal(new U64(1000 / 2 - 100 + 100 / 2));
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }
        await Promise.all(nodes.map(node => node.clean()));
        promiseExpect.checkFulfilled();
    });
});
