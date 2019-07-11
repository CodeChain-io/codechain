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
import * as stake from "codechain-stakeholder-sdk";
import { SDK } from "codechain-sdk";
import "mocha";

import { validators as originalValidators } from "../../tendermint.dynval/constants";
import { PromiseExpect } from "../helper/promise";
import { withNodes } from "./setup";

chai.use(chaiAsPromised);

const allDynValidators = originalValidators.slice(0, 8);
const [alice, ...otherDynValidators] = allDynValidators;

describe("Dynamic Validator N -> N-1", function() {
    const promiseExpect = new PromiseExpect();
    const termSeconds = 20;
    const margin = 1.3;

    async function aliceContainedCheck(sdk: SDK) {
        const blockNumber = await sdk.rpc.chain.getBestBlockNumber();
        const termMedata = await stake.getTermMetadata(sdk, blockNumber);
        const currentTermInitialBlockNumber =
            termMedata!.lastTermFinishedBlockNumber + 1;
        const validatorsBefore = (await stake.getPossibleAuthors(
            sdk,
            currentTermInitialBlockNumber
        ))!.map(platformAddr => platformAddr.toString());

        expect(termMedata!.currentTermId).to.be.equals(1);
        expect(validatorsBefore.length).to.be.equals(allDynValidators.length);
        expect(validatorsBefore).to.includes(alice.platformAddress.toString());
        expect(validatorsBefore).contains.all.members(
            allDynValidators.map(validator => validator.platformAddress.value)
        );
    }

    async function aliceDropOutCheck(sdk: SDK) {
        const blockNumber = await sdk.rpc.chain.getBestBlockNumber();
        const termMedata = await stake.getTermMetadata(sdk, blockNumber);
        const currentTermInitialBlockNumber =
            termMedata!.lastTermFinishedBlockNumber + 1;
        const validatorsAfter = (await stake.getPossibleAuthors(
            sdk,
            currentTermInitialBlockNumber
        ))!.map(platformAddr => platformAddr.toString());

        expect(termMedata!.currentTermId).to.be.equals(2);
        expect(validatorsAfter.length).to.be.equals(otherDynValidators.length);
        expect(validatorsAfter).not.to.includes(
            alice.platformAddress.toString()
        );
        expect(validatorsAfter).contains.all.members(
            otherDynValidators.map(validator => validator.platformAddress.value)
        );
    }

    describe("A node is imprisoned to jail", async function() {
        const nodes = withNodes(this, {
            promiseExpect,
            validators: allDynValidators.map((signer, index) => ({
                signer,
                delegation: 5_000,
                deposit: 10_000_000 - index // tie-breaker
            })),
            onBeforeEnable: async allDynNodes => {
                const aliceNode = allDynNodes[0];
                // Kill the alice node first to make alice not to participate in the term 1.
                await aliceNode.clean();
            }
        });

        it("alice should be dropped out from validator list", async function() {
            this.slow(termSeconds * 1000);
            this.timeout(termSeconds * 2 * 1000);

            const checkingNode = nodes[1];
            await aliceContainedCheck(checkingNode.sdk);

            await checkingNode.waitForTermChange(2, termSeconds * margin);

            await aliceDropOutCheck(checkingNode.sdk);
        });
    });

    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
