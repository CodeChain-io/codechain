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
import * as stake from "codechain-stakeholder-sdk";
import "mocha";

import { validators } from "../../../tendermint.dynval/constants";
import { PromiseExpect } from "../../helper/promise";
import CodeChain from "../../helper/spawn";
import { fullyConnect, setTermTestTimeout, withNodes } from "../setup";

describe("Shutdown test", function() {
    const promiseExpect = new PromiseExpect();

    function filterNodes(nodes: CodeChain[], from: number, to: number) {
        const selected = nodes
            .map((node, i) => ({ node, signer: validators[i] }))
            .slice(from, to);
        return {
            length: selected.length,
            nodes: selected.map(({ node }) => node),
            addrs: selected.map(({ signer }) =>
                signer.platformAddress.toString()
            )
        };
    }

    describe("Partial shutdown", async function() {
        const getObserver = (n = nodes) => ({
            node: n[0],
            signer: validators[0]
        });
        const getAlphas = (n = nodes) => filterNodes(n, 1, 4);
        const getBetas = (n = nodes) => filterNodes(n, 4, 7);
        const getAlphaBetas = (n = nodes) => filterNodes(n, 1, 7);
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                minNumOfValidators: 4,
                maxNumOfValidators: 8,
                delegationThreshold: 1,
                custodyPeriod: 10,
                releasePeriod: 30
            },
            validators: [
                // Observer: no self-nomination, no-deposit
                { signer: validators[0] },
                // Alphas : They have so strong voting power, so they dominate the validation.
                { signer: validators[1], delegation: 1000, deposit: 100000 },
                { signer: validators[2], delegation: 1000, deposit: 100000 },
                { signer: validators[3], delegation: 1000, deposit: 100000 },
                // Betas
                { signer: validators[4], delegation: 1, deposit: 100000 },
                { signer: validators[5], delegation: 1, deposit: 100000 },
                { signer: validators[6], delegation: 1, deposit: 100000 }
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(
                possibleAuthors.map(x => x.toString()),
                "Alphas + Betas should be validators"
            )
                .to.have.lengthOf(getAlphaBetas().length)
                .and.to.include.members(getAlphaBetas().addrs);
        });

        async function waitUntilTermAlmostFinish(
            stopBefore: number,
            termSeconds: number
        ): Promise<void> {
            const node = getObserver().node;
            const sdk = node.sdk;

            const lastTermFinishedBlockNumber = (await stake.getTermMetadata(
                sdk
            ))!.lastTermFinishedBlockNumber;
            const lastTermFinishedTS = (await sdk.rpc.chain.getBlock(
                lastTermFinishedBlockNumber
            ))!.timestamp;
            const nextTermExpectedTS =
                Math.floor(lastTermFinishedTS / termSeconds) * termSeconds +
                termSeconds;
            const targetTS = nextTermExpectedTS - stopBefore;

            let lastBlockNumber: number;
            while (true) {
                lastBlockNumber = await node.getBestBlockNumber();
                const block = (await node.sdk.rpc.chain.getBlock(
                    lastBlockNumber
                ))!;
                if (block.timestamp >= targetTS) {
                    return;
                }
                if (block.timestamp >= nextTermExpectedTS) {
                    throw new Error("Flaky test! The term has been closed");
                }
                await node.waitBlockNumber(lastBlockNumber + 1);
            }
        }

        it("Alphas should be next validators after a complete shutdown", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 2
            });

            // Only Alphas will validate.
            await waitUntilTermAlmostFinish(5, termWaiter.termSeconds);
            // Shutdown all validators ASAP before term is closed.
            await Promise.all(getAlphaBetas().nodes.map(node => node.clean()));

            const lastBlockNumberOfTerm = await getObserver().node.getBestBlockNumber();
            {
                expect(lastBlockNumberOfTerm).to.be.equal(
                    await getObserver().node.getBestBlockNumber(),
                    "Should have stopped nodes in time, otherwise it is flaky test"
                );
                const termMetadata = await stake.getTermMetadata(
                    getObserver().node.sdk
                );
                expect(termMetadata).is.not.null;
                expect(termMetadata!.currentTermId).is.equals(
                    1,
                    "Term should haven't be closed yet"
                );
                const possibleAuthors = (await stake.getPossibleAuthors(
                    getObserver().node.sdk
                ))!;
                expect(
                    possibleAuthors.map(x => x.toString()),
                    "Alphas + Betas should still be validators"
                )
                    .to.have.lengthOf(getAlphaBetas().length)
                    .and.to.include.members(getAlphaBetas().addrs);
            }

            await termWaiter.waitForTermPeriods(1, 2);
            // Revival
            await Promise.all(getAlphaBetas().nodes.map(node => node.start()));
            await fullyConnect(nodes, promiseExpect);

            // Wait for it should close the term
            await getObserver().node.waitBlockNumber(lastBlockNumberOfTerm + 1);
            const termMetadata = await stake.getTermMetadata(
                getObserver().node.sdk,
                lastBlockNumberOfTerm + 1
            );
            expect(termMetadata).is.not.null;
            expect(termMetadata!.currentTermId).is.equals(2);
            {
                const jailed = await stake.getJailed(getObserver().node.sdk);
                expect(
                    jailed.map(x => x.address.toString())
                ).to.include.members(
                    getBetas().addrs,
                    "All Betas should be jailed (might be some alphas)"
                );
                const possibleAuthors = (await stake.getPossibleAuthors(
                    getObserver().node.sdk
                ))!;
                expect(getAlphas().addrs).to.include.members(
                    possibleAuthors.map(x => x.toString()),
                    "All validators are alphas"
                );
                expect(getBetas().addrs).not.to.include(
                    possibleAuthors.map(x => x.toString()),
                    "But not betas"
                );
            }
        });
    });

    describe("Total shutdown", async function() {
        const getObserver = (n = nodes) => ({
            node: n[0],
            signer: validators[0]
        });
        const getValidators = (n = nodes) => filterNodes(n, 1, 1 + 3);
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                minNumOfValidators: 3,
                maxNumOfValidators: 3,
                delegationThreshold: 1,
                custodyPeriod: 10,
                releasePeriod: 30
            },
            validators: [
                // Observer: no self-nomination, no deposit
                { signer: validators[0] },
                // Validators
                ...validators.slice(1, 1 + 3).map((signer, i) => ({
                    signer,
                    delegation: 1000,
                    deposit: 100000 - i // tie-breaker
                }))
            ]
        });

        it("only a term closer should be a validator after a complete shutdown", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 2
            });

            // Shutdown all validators ASAP before any block is created.
            await Promise.all(getValidators().nodes.map(node => node.clean()));
            const blockNumberAtStop = await getObserver().node.getBestBlockNumber();
            {
                const termMetadata = await stake.getTermMetadata(
                    getObserver().node.sdk
                );
                expect(termMetadata).is.not.null;
                expect(termMetadata!.currentTermId).is.equals(1);
                expect(termMetadata!.lastTermFinishedBlockNumber).to.be.equal(
                    blockNumberAtStop,
                    "Should have stopped nodes in time, otherwise it is flaky test"
                );
                const possibleAuthors = (await stake.getPossibleAuthors(
                    getObserver().node.sdk
                ))!;
                expect(
                    possibleAuthors.map(x => x.toString()),
                    "They should still be validators"
                )
                    .to.have.lengthOf(getValidators().length)
                    .and.to.include.members(getValidators().addrs);
            }

            await termWaiter.waitForTermPeriods(2, 2);
            // Revival
            await Promise.all(getValidators().nodes.map(node => node.start()));
            await fullyConnect(nodes, promiseExpect);
            await getObserver().node.waitBlockNumber(blockNumberAtStop + 1);
            {
                const termMetadata = await stake.getTermMetadata(
                    getObserver().node.sdk,
                    blockNumberAtStop + 1
                );
                expect(termMetadata).is.not.null;
                expect(termMetadata!.currentTermId).is.equals(
                    2,
                    "Term should be changed"
                );
                const block = (await getObserver().node.sdk.rpc.chain.getBlock(
                    blockNumberAtStop + 1
                ))!;
                expect(getValidators().addrs).to.include(
                    block.author.toString(),
                    "Block author should be one of the validator"
                );

                const jailed = await stake.getJailed(getObserver().node.sdk);
                expect(
                    jailed.map(x => x.address.toString()),
                    "All validators except the block author should be jailed"
                )
                    .to.have.lengthOf(getValidators().length - 1)
                    .and.to.include.members(
                        getValidators().addrs.filter(
                            addr => addr !== block.author.toString()
                        )
                    );
                const possibleAuthors = (await stake.getPossibleAuthors(
                    getObserver().node.sdk
                ))!;
                expect(
                    possibleAuthors.map(x => x.toString())
                ).to.be.deep.equals(
                    [block.author.toString()],
                    "All validators are alphas"
                );
            }
        });
    });

    afterEach(function() {
        promiseExpect.checkFulfilled();
    });
});
