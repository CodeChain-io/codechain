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
import { SDK } from "codechain-sdk";
import * as stake from "codechain-stakeholder-sdk";
import "mocha";

import { validators as originalValidators } from "../../tendermint.dynval/constants";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { PromiseExpect } from "../helper/promise";
import { withNodes } from "./setup";

chai.use(chaiAsPromised);

const allDynValidators = originalValidators.slice(0, 8);
const [betty, ...otherDynValidators] = allDynValidators;

describe("Dynamic Validator N -> N+1", function() {
    const promiseExpect = new PromiseExpect();
    const termSeconds = 20;
    const margin = 1.3;

    async function beforeInsertionCheck(sdk: SDK) {
        const blockNumber = await sdk.rpc.chain.getBestBlockNumber();
        const termMedata = await stake.getTermMetadata(sdk, blockNumber);
        const currentTermInitialBlockNumber =
            termMedata!.lastTermFinishedBlockNumber + 1;
        const validatorsBefore = (await stake.getPossibleAuthors(
            sdk,
            currentTermInitialBlockNumber
        ))!.map(platformAddr => platformAddr.toString());

        expect(termMedata!.currentTermId).to.be.equals(1);
        expect(validatorsBefore.length).to.be.equals(otherDynValidators.length);
        expect(validatorsBefore).not.to.includes(
            betty.platformAddress.toString()
        );
        expect(validatorsBefore).contains.all.members(
            otherDynValidators.map(validator =>
                validator.platformAddress.toString()
            )
        );
    }

    async function bettyInsertionCheck(sdk: SDK) {
        const blockNumber = await sdk.rpc.chain.getBestBlockNumber();
        const termMedata = await stake.getTermMetadata(sdk, blockNumber);
        const currentTermInitialBlockNumber =
            termMedata!.lastTermFinishedBlockNumber + 1;
        const validatorsAfter = (await stake.getPossibleAuthors(
            sdk,
            currentTermInitialBlockNumber
        ))!.map(platformAddr => platformAddr.toString());

        expect(termMedata!.currentTermId).to.be.equals(2);
        expect(validatorsAfter).contains.all.members(
            allDynValidators.map(validator =>
                validator.platformAddress.toString()
            )
        );
    }

    describe("Nominate a new candidate and delegate", async function() {
        const nodes = withNodes(this, {
            promiseExpect,
            validators: otherDynValidators.map((signer, index) => ({
                signer,
                delegation: 5_000,
                deposit: 10_000_000 - index // tie-breaker
            }))
        });

        it("betty should be included in validators", async function() {
            this.slow(termSeconds * 1000);
            this.timeout(termSeconds * 2 * 1000);

            const checkingNode = nodes[0];
            await beforeInsertionCheck(checkingNode.sdk);
            const faucetSeq = await checkingNode.sdk.rpc.chain.getSeq(
                faucetAddress
            );
            const payTx = checkingNode.sdk.core
                .createPayTransaction({
                    recipient: betty.platformAddress,
                    quantity: 100_000_000
                })
                .sign({
                    secret: faucetSecret,
                    seq: faucetSeq,
                    fee: 10
                });
            const payTxHash = checkingNode.sdk.rpc.chain.sendSignedTransaction(
                payTx
            );
            await checkingNode.waitForTx(payTxHash);
            const nominateTx = stake
                .createSelfNominateTransaction(
                    checkingNode.sdk,
                    10_000_000 - otherDynValidators.length,
                    ""
                )
                .sign({
                    secret: betty.privateKey,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(
                        betty.platformAddress
                    ),
                    fee: 10
                });
            const nominateTxHash = checkingNode.sdk.rpc.chain.sendSignedTransaction(
                nominateTx
            );
            const delegateTx = stake
                .createDelegateCCSTransaction(
                    checkingNode.sdk,
                    betty.platformAddress,
                    5_000
                )
                .sign({
                    secret: faucetSecret,
                    seq: faucetSeq + 1,
                    fee: 10
                });
            const delegateTxHash = checkingNode.sdk.rpc.chain.sendSignedTransaction(
                delegateTx
            );
            await checkingNode.waitForTx([nominateTxHash, delegateTxHash]);

            await checkingNode.waitForTermChange(2, termSeconds * margin);
            await bettyInsertionCheck(checkingNode.sdk);
        });
    });

    describe("Increase one candidate's deposit which is less than the minimum deposit", async function() {
        this.slow(termSeconds * 1000);
        this.timeout(termSeconds * 2 * 1000);

        const nodes = withNodes(this, {
            promiseExpect,
            validators: otherDynValidators
                .map((signer, index) => ({
                    signer,
                    delegation: 5_000,
                    deposit: 10_000_000 - index // tie-breaker
                }))
                .concat([
                    {
                        signer: betty,
                        delegation: 5_000,
                        deposit: 9999
                    }
                ])
        });

        it("betty should be included in validators", async function() {
            const checkingNode = nodes[0];
            await beforeInsertionCheck(checkingNode.sdk);
            const nominateTx = stake
                .createSelfNominateTransaction(checkingNode.sdk, 10_000, "")
                .sign({
                    secret: betty.privateKey,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(
                        betty.platformAddress
                    ),
                    fee: 10
                });
            const nominateTxHash = checkingNode.sdk.rpc.chain.sendSignedTransaction(
                nominateTx
            );
            await checkingNode.waitForTx(nominateTxHash);

            await checkingNode.waitForTermChange(2, termSeconds * margin);
            await bettyInsertionCheck(checkingNode.sdk);
        });
    });

    describe("Delegate more stake to whose stake is less than the minimum delegation", async function() {
        this.slow(termSeconds * 1000);
        this.timeout(termSeconds * 2 * 1000);

        const nodes = withNodes(this, {
            promiseExpect,
            validators: otherDynValidators
                .map((signer, index) => ({
                    signer,
                    delegation: 5_000,
                    deposit: 10_000_000 - index // tie-breaker
                }))
                .concat([
                    {
                        signer: betty,
                        delegation: 999,
                        deposit: 10_000_000
                    }
                ])
        });

        it("betty should be included in validators", async function() {
            const checkingNode = nodes[0];
            await beforeInsertionCheck(checkingNode.sdk);
            const faucetSeq = await checkingNode.sdk.rpc.chain.getSeq(
                faucetAddress
            );
            const delegateTx = stake
                .createDelegateCCSTransaction(
                    checkingNode.sdk,
                    betty.platformAddress,
                    2
                )
                .sign({
                    secret: faucetSecret,
                    seq: faucetSeq,
                    fee: 10
                });
            const delegateTxHash = checkingNode.sdk.rpc.chain.sendSignedTransaction(
                delegateTx
            );
            await checkingNode.waitForTx(delegateTxHash);

            await checkingNode.waitForTermChange(2, termSeconds * margin);
            await bettyInsertionCheck(checkingNode.sdk);
        });
    });

    afterEach(async function() {
        await promiseExpect.checkFulfilled();
    });
});
