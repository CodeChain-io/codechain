// Copyright 2019 Kodebox, Inc.
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
import { SDK } from "codechain-sdk";
import * as stake from "codechain-stakeholder-sdk";
import "mocha";

import { validators } from "../../../tendermint.dynval/constants";
import { faucetAddress, faucetSecret } from "../../helper/constants";
import { PromiseExpect } from "../../helper/promise";
import { findNode, setTermTestTimeout, withNodes } from "../setup";

describe("Dynamic Validator N -> N'", function() {
    const promiseExpect = new PromiseExpect();

    async function expectPossibleAuthors(
        sdk: SDK,
        expected: typeof validators,
        blockNumber?: number
    ) {
        const authors = (await stake.getPossibleAuthors(
            sdk,
            blockNumber
        ))!.map(author => author.toString());
        expect(authors)
            .to.have.lengthOf(expected.length)
            .and.to.include.members(
                expected.map(x => x.platformAddress.toString())
            );
    }

    describe("1. Jail one of the validator + increase the delegation of a candidate who doesn’t have enough delegation", async function() {
        // alice : Elected as a validator, but does not send precommits and does not propose.
        //   Alice should be jailed.
        // betty : Not elected as validator because of small delegation. She acquire more delegation in the first term.
        //   betty should be a validator in the second term.
        const alice = validators[3];
        const betty = validators[4];
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                custodyPeriod: 10,
                releasePeriod: 30
            },
            validators: [
                { signer: validators[0], delegation: 4200, deposit: 100000 },
                { signer: validators[1], delegation: 4100, deposit: 100000 },
                { signer: validators[2], delegation: 4000, deposit: 100000 },
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 2, deposit: 100000 }
            ],
            onBeforeEnable: async nodes => {
                // Kill the alice node first to make alice not to participate in the term 1.
                await findNode(nodes, alice).clean();
            }
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const rpcNode = nodes[0];
            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                alice
            ]);

            const tx = stake
                .createDelegateCCSTransaction(
                    rpcNode.sdk,
                    betty.platformAddress,
                    5_000
                )
                .sign({
                    secret: faucetSecret,
                    seq: await rpcNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await rpcNode.waitForTx(
                rpcNode.sdk.rpc.chain.sendSignedTransaction(tx)
            );

            await termWaiter.waitNodeUntilTerm(rpcNode, {
                target: 2,
                termPeriods: 1
            });

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                betty
            ]);
        });
    });

    describe("2. Jail one of the validator + increase the deposit of a candidate who doesn’t have enough deposit", async function() {
        // alice : Elected as a validator, but does not send precommits and does not propose.
        //   Alice should be jailed.
        // betty : Not elected as validator because of small deposit. She deposits more CCC in the first term.
        //   betty should be a validator in the second term.
        const alice = validators[3];
        const betty = validators[4];
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                custodyPeriod: 10,
                releasePeriod: 30
            },
            validators: [
                { signer: validators[0], delegation: 4200, deposit: 100000 },
                { signer: validators[1], delegation: 4100, deposit: 100000 },
                { signer: validators[2], delegation: 4000, deposit: 100000 },
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 5000, deposit: 100 }
            ],
            onBeforeEnable: async nodes => {
                // Kill the alice node first to make alice not to participate in the term 1.
                await findNode(nodes, alice).clean();
            }
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });
            const rpcNode = nodes[0];

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                alice
            ]);

            const bettyNode = findNode(nodes, betty);
            const tx = stake
                .createSelfNominateTransaction(bettyNode.sdk, 100000, "")
                .sign({
                    secret: betty.privateKey,
                    seq: await bettyNode.sdk.rpc.chain.getSeq(
                        betty.platformAddress
                    ),
                    fee: 10
                });

            bettyNode.waitForTx(
                bettyNode.sdk.rpc.chain.sendSignedTransaction(tx)
            );

            await termWaiter.waitNodeUntilTerm(rpcNode, {
                target: 2,
                termPeriods: 1
            });

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                betty
            ]);
        });
    });

    describe("3. Revoke the validator + increase the delegation of a candidate who doesn’t have enough delegation", async function() {
        // alice : Elected as a validator, but most delegated CCS is revoked.
        //   Alice must be kicked out of the validator group.
        // betty : Not elected as validator because of small delegation. She acquire more delegation in the first term.
        //   betty should be a validator in the second term.
        const alice = validators[3];
        const betty = validators[4];
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: [
                { signer: validators[0], delegation: 4200, deposit: 100000 },
                { signer: validators[1], delegation: 4100, deposit: 100000 },
                { signer: validators[2], delegation: 4000, deposit: 100000 },
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 50, deposit: 100000 }
            ]
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });
            const rpcNode = nodes[0];

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                alice
            ]);

            const seq = await rpcNode.sdk.rpc.chain.getSeq(faucetAddress);
            const tx = stake
                .createDelegateCCSTransaction(
                    rpcNode.sdk,
                    betty.platformAddress,
                    5_000
                )
                .sign({
                    secret: faucetSecret,
                    seq,
                    fee: 10
                });
            const tx2 = stake
                .createRevokeTransaction(
                    rpcNode.sdk,
                    alice.platformAddress,
                    4999
                )
                .sign({
                    secret: faucetSecret,
                    seq: seq + 1,
                    fee: 10
                });
            await rpcNode.waitForTx([
                rpcNode.sdk.rpc.chain.sendSignedTransaction(tx),
                rpcNode.sdk.rpc.chain.sendSignedTransaction(tx2)
            ]);

            await termWaiter.waitNodeUntilTerm(rpcNode, {
                target: 2,
                termPeriods: 1
            });

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                betty
            ]);
        });
    });

    describe("4. Revoke the validator + increase the deposit of a candidate who doesn’t have enough deposit", async function() {
        // alice : Elected as a validator, but most delegated CCS is revoked.
        //   Alice must be kicked out of the validator group.
        // betty : Not elected as validator because of small deposit. She deposits more CCC in the first term.
        //   betty should be a validator in the second term.
        const alice = validators[3];
        const betty = validators[4];
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: [
                { signer: validators[0], delegation: 4200, deposit: 100000 },
                { signer: validators[1], delegation: 4100, deposit: 100000 },
                { signer: validators[2], delegation: 4000, deposit: 100000 },
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 5000, deposit: 10 }
            ]
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });
            const rpcNode = nodes[0];

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                alice
            ]);

            const bettyNode = findNode(nodes, betty);
            const tx = stake
                .createSelfNominateTransaction(bettyNode.sdk, 100000, "")
                .sign({
                    secret: betty.privateKey,
                    seq: await bettyNode.sdk.rpc.chain.getSeq(
                        betty.platformAddress
                    ),
                    fee: 10
                });

            const tx2 = stake
                .createRevokeTransaction(
                    rpcNode.sdk,
                    alice.platformAddress,
                    4999
                )
                .sign({
                    secret: faucetSecret,
                    seq: await rpcNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await Promise.all([
                bettyNode.waitForTx(
                    bettyNode.sdk.rpc.chain.sendSignedTransaction(tx)
                ),
                rpcNode.waitForTx(
                    rpcNode.sdk.rpc.chain.sendSignedTransaction(tx2)
                )
            ]);

            await termWaiter.waitNodeUntilTerm(rpcNode, {
                target: 2,
                termPeriods: 1
            });

            await expectPossibleAuthors(rpcNode.sdk, [
                ...validators.slice(0, 3),
                betty
            ]);
        });
    });

    afterEach(function() {
        promiseExpect.checkFulfilled();
    });
});
