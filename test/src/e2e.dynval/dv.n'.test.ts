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

import * as chai from "chai";
import { expect } from "chai";
import * as chaiAsPromised from "chai-as-promised";
import { blake256, H256 } from "codechain-primitives/lib";
import { SDK } from "codechain-sdk";
import * as stake from "codechain-stakeholder-sdk";
import "mocha";

import { validators as originalDynValidators } from "../../tendermint.dynval/constants";
import {
    aliceSecret,
    bobSecret,
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address
} from "../helper/constants";
import { PromiseExpect, wait } from "../helper/promise";
import CodeChain from "../helper/spawn";
import { withNodes } from "./setup";

chai.use(chaiAsPromised);

const RLP = require("rlp");

const alice = originalDynValidators[0];
const betty = originalDynValidators[1];
const otherDynValidators = originalDynValidators.slice(2, 2 + 6);

describe("Dynamic Validator N -> N'", function() {
    const promiseExpect = new PromiseExpect();
    const TERM_SECONDS = 30;
    const margin = 1.2;

    describe("1. Jail one of the validator + increase the delegation of a candidate who doesn’t have enough delegation", async function() {
        // alice : Elected as a validator, but does not send precommits and does not propose.
        //   Alice should be jailed.
        // betty : Not elected as validator because of small delegation. She acquire more delegation in the first term.
        //   betty should be a validator in the second term.
        const allDynNodes = withNodes(this, {
            promiseExpect,
            overrideParams: {
                termSeconds: TERM_SECONDS
            },
            validators: [
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 2, deposit: 100000 },
                ...otherDynValidators.map((validator, index) => ({
                    signer: validator,
                    delegation: 5000 - index,
                    deposit: 100000
                }))
            ],
            onBeforeEnable: async allDynNodes => {
                const aliceNode = allDynNodes[0];
                // Kill the alice node first to make alice not to participate in the term 1.
                await aliceNode.clean();
            }
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            this.slow(TERM_SECONDS * margin * 1000); // All tests waits at most 1 terms.
            this.timeout(TERM_SECONDS * 2 * 1000);

            const [_aliceNode, _bettyNode, ...otherDynNodes] = allDynNodes;
            const rpcNode = otherDynNodes[0];
            const beforeAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(beforeAuthors).to.includes(alice.platformAddress.toString());
            expect(beforeAuthors).not.to.includes(
                betty.platformAddress.toString()
            );
            expect(beforeAuthors.length).to.be.equals(7);

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

            await rpcNode.waitForTermChange(2, TERM_SECONDS * margin);

            const afterAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(afterAuthors).not.to.includes(
                alice.platformAddress.toString()
            );
            expect(afterAuthors).to.includes(betty.platformAddress.toString());
            expect(afterAuthors.length).to.be.equals(7);
        });
    });

    describe("2. Jail one of the validator + increase the deposit of a candidate who doesn’t have enough deposit", async function() {
        // alice : Elected as a validator, but does not send precommits and does not propose.
        //   Alice should be jailed.
        // betty : Not elected as validator because of small deposit. She deposits more CCC in the first term.
        //   betty should be a validator in the second term.
        const allDynNodes = withNodes(this, {
            promiseExpect,
            overrideParams: {
                termSeconds: TERM_SECONDS
            },
            validators: [
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 5000, deposit: 100 },
                ...otherDynValidators.map((validator, index) => ({
                    signer: validator,
                    delegation: 5000 - index,
                    deposit: 100000
                }))
            ],
            onBeforeEnable: async nodes => {
                const aliceNode = nodes[0];
                // Kill the alice node first to make alice not to participate in the term 1.
                await aliceNode.clean();
            }
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            this.slow(TERM_SECONDS * margin * 1000);
            this.timeout(TERM_SECONDS * 2 * 1000);
            const [_aliceNode, bettyNode, ...otherDynNodes] = allDynNodes;
            const rpcNode = otherDynNodes[0];

            const beforeAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(beforeAuthors).to.includes(alice.platformAddress.toString());
            expect(beforeAuthors).not.to.includes(
                betty.platformAddress.toString()
            );
            expect(beforeAuthors.length).to.be.equals(7);

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

            await rpcNode.waitForTermChange(2, TERM_SECONDS * margin);

            const afterAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(afterAuthors).not.to.includes(
                alice.platformAddress.toString()
            );
            expect(afterAuthors).to.includes(betty.platformAddress.toString());
            expect(afterAuthors.length).to.be.equals(7);
        });
    });

    describe("3. Revoke the validator + increase the delegation of a candidate who doesn’t have enough delegation", async function() {
        // alice : Elected as a validator, but most delegated CCS is revoked.
        //   Alice must be kicked out of the validator group.
        // betty : Not elected as validator because of small delegation. She acquire more delegation in the first term.
        //   betty should be a validator in the second term.
        const allDynNodes = withNodes(this, {
            promiseExpect,
            overrideParams: {
                termSeconds: TERM_SECONDS
            },
            validators: [
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 50, deposit: 100000 },
                ...otherDynValidators.map((validator, index) => ({
                    signer: validator,
                    delegation: 5000 - index,
                    deposit: 100000
                }))
            ]
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            this.slow(TERM_SECONDS * margin * 1000);
            this.timeout(TERM_SECONDS * 2 * 1000);
            const [, , ...otherDynNodes] = allDynNodes;
            const rpcNode = otherDynNodes[0];

            const beforeAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(beforeAuthors).to.includes(alice.platformAddress.toString());
            expect(beforeAuthors).not.to.includes(
                betty.platformAddress.toString()
            );
            expect(beforeAuthors.length).to.be.equals(7);

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

            await rpcNode.waitForTermChange(2, TERM_SECONDS * margin);

            const afterAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(afterAuthors).not.to.includes(
                alice.platformAddress.toString()
            );
            expect(afterAuthors).to.includes(betty.platformAddress.toString());
            expect(afterAuthors.length).to.be.equals(7);
        });
    });

    describe("4. Revoke the validator + increase the deposit of a candidate who doesn’t have enough deposit", async function() {
        // alice : Elected as a validator, but most delegated CCS is revoked.
        //   Alice must be kicked out of the validator group.
        // betty : Not elected as validator because of small deposit. She deposits more CCC in the first term.
        //   betty should be a validator in the second term.
        const allDynNodes = withNodes(this, {
            promiseExpect,
            overrideParams: {
                termSeconds: TERM_SECONDS
            },
            validators: [
                { signer: alice, delegation: 5000, deposit: 100000 },
                { signer: betty, delegation: 5000, deposit: 10 },
                ...otherDynValidators.map((validator, index) => ({
                    signer: validator,
                    delegation: 5000 - index,
                    deposit: 100000
                }))
            ]
        });

        it("Alice should get out of the committee and Betty should be included in the committee", async function() {
            this.slow(TERM_SECONDS * margin * 1000);
            this.timeout(TERM_SECONDS * 2 * 1000);
            const [, bettyNode, ...otherDynNodes] = allDynNodes;
            const rpcNode = otherDynNodes[0];

            const beforeAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(beforeAuthors).to.includes(alice.platformAddress.toString());
            expect(beforeAuthors).not.to.includes(
                betty.platformAddress.toString()
            );
            expect(beforeAuthors.length).to.be.equals(7);

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

            await rpcNode.waitForTermChange(2, TERM_SECONDS * margin);

            const afterAuthors = (await stake.getPossibleAuthors(
                rpcNode.sdk
            ))!.map(author => author.toString());
            expect(afterAuthors).not.to.includes(
                alice.platformAddress.toString()
            );
            expect(afterAuthors).to.includes(betty.platformAddress.toString());
            expect(afterAuthors.length).to.be.equals(7);
        });
    });
});
