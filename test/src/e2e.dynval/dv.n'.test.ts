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

const [alice, betty, notUsed, ...otherDynValidators] = originalDynValidators;
const allDynValidators = [alice, betty, ...otherDynValidators];

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
            {
                const authors = (await stake.getPossibleAuthors(
                    otherDynNodes[0].sdk
                ))!.map(author => author.toString());
                expect(authors).to.includes(alice.platformAddress.toString());
                expect(authors).not.to.includes(
                    betty.platformAddress.toString()
                );
                expect(authors.length).to.be.equals(8);
            }

            {
                const tx = stake
                    .createDelegateCCSTransaction(
                        otherDynNodes[0].sdk,
                        betty.platformAddress,
                        5_000
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await otherDynNodes[0].sdk.rpc.chain.getSeq(
                            faucetAddress
                        ),
                        fee: 10
                    });
                await otherDynNodes[0].sdk.rpc.chain.sendSignedTransaction(tx);
            }

            await otherDynNodes[0].waitForTermChange(2, TERM_SECONDS * margin);

            const blockNumber = await otherDynNodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                otherDynNodes[0].sdk,
                blockNumber
            );

            {
                const authors = (await stake.getPossibleAuthors(
                    otherDynNodes[0].sdk
                ))!.map(author => author.toString());
                expect(authors).not.to.includes(
                    alice.platformAddress.toString()
                );
                expect(authors).to.includes(betty.platformAddress.toString());
                expect(authors.length).to.be.equals(8);
            }

            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
        });
    });
});
