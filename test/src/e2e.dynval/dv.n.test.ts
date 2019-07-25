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
import "mocha";

import { validators } from "../../tendermint.dynval/constants";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { PromiseExpect } from "../helper/promise";
import { setTermTestTimeout, withNodes } from "./setup";

chai.use(chaiAsPromised);

describe("Dynamic Validator N -> N", function() {
    const promiseExpect = new PromiseExpect();

    describe("1. No delegation, nominate, revoke, jail", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: validators.map((signer, index) => ({
                signer,
                delegation: 5000,
                deposit: 10_000_000 - index // tie-breaker
            }))
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });

        it("should keep possible authors after a term change", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                nodes[0].sdk,
                blockNumber
            );
            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk,
                blockNumber
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });
    });

    describe("2. Delegate to the candidates but the total delegation is still less than the minimum delegation (Capped by maxNumOfValidators)", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 8,
                delegationThreshold: 1000
            },
            validators: [
                // Validators
                { signer: validators[0], delegation: 5000, deposit: 100000 },
                { signer: validators[1], delegation: 4900, deposit: 100000 },
                { signer: validators[2], delegation: 4800, deposit: 100000 },
                { signer: validators[3], delegation: 4700, deposit: 100000 },
                { signer: validators[4], delegation: 4600, deposit: 100000 },
                { signer: validators[5], delegation: 4500, deposit: 100000 },
                { signer: validators[6], delegation: 4400, deposit: 100000 },
                { signer: validators[7], delegation: 4300, deposit: 100000 },
                // Candidates
                { signer: validators[8], delegation: 4200, deposit: 100000 },
                { signer: validators[9], delegation: 4100, deposit: 100000 }
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });

        it("should keep possible authors after a term change", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const insufficientDelegationTx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[8].platformAddress,
                        50
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(insufficientDelegationTx);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                nodes[0].sdk,
                blockNumber
            );
            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk,
                blockNumber
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });
    });

    describe("2. Delegate to the candidates but the total delegation is still less than the minimum delegation (Capped by delegationThreshold)", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 8,
                delegationThreshold: 1000
            },
            validators: [
                // Validators
                { signer: validators[0], delegation: 5000, deposit: 100000 },
                { signer: validators[1], delegation: 4900, deposit: 100000 },
                { signer: validators[2], delegation: 4800, deposit: 100000 },
                { signer: validators[3], delegation: 4700, deposit: 100000 },
                { signer: validators[4], delegation: 4600, deposit: 100000 },
                { signer: validators[5], delegation: 4500, deposit: 100000 },
                // Remain as candidates (Below delegationThreshold)
                { signer: validators[6], delegation: 900, deposit: 100000 },
                { signer: validators[7], delegation: 800, deposit: 100000 },
                { signer: validators[8], delegation: 700, deposit: 100000 },
                { signer: validators[9], delegation: 600, deposit: 100000 }
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(6);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 6).map(x => x.platformAddress.value)
            );
        });

        it("should keep possible authors after a term change", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const insufficientDelegationTx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[6].platformAddress,
                        50
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(insufficientDelegationTx);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                nodes[0].sdk,
                blockNumber
            );
            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk,
                blockNumber
            ))!;
            expect(possibleAuthors.length).is.equals(6);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 6).map(x => x.platformAddress.value)
            );
        });
    });

    describe("3. Revoke the delegations of validator but it still delegated more than the minimum delegation (Capped by maxNumOfValidators)", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 8,
                delegationThreshold: 1000
            },
            validators: [
                // Validators
                { signer: validators[0], delegation: 5000, deposit: 100000 },
                { signer: validators[1], delegation: 4900, deposit: 100000 },
                { signer: validators[2], delegation: 4800, deposit: 100000 },
                { signer: validators[3], delegation: 4700, deposit: 100000 },
                { signer: validators[4], delegation: 4600, deposit: 100000 },
                { signer: validators[5], delegation: 4500, deposit: 100000 },
                { signer: validators[6], delegation: 4400, deposit: 100000 },
                { signer: validators[7], delegation: 4300, deposit: 100000 },
                // Candidates
                { signer: validators[8], delegation: 4200, deposit: 100000 },
                { signer: validators[9], delegation: 4100, deposit: 100000 }
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });

        it("should keep possible authors after a term change", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const insufficientDelegationTx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createRevokeTransaction(
                        nodes[0].sdk,
                        validators[7].platformAddress,
                        50
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(insufficientDelegationTx);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                nodes[0].sdk,
                blockNumber
            );
            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk,
                blockNumber
            ))!;
            expect(possibleAuthors.length).is.equals(8);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 8).map(x => x.platformAddress.value)
            );
        });
    });

    describe("3. Revoke the delegations of validator but it still delegated more than the minimum delegation (Capped by delegationThreshold)", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 8,
                delegationThreshold: 1000
            },
            validators: [
                // Validators
                { signer: validators[0], delegation: 5000, deposit: 100000 },
                { signer: validators[1], delegation: 4900, deposit: 100000 },
                { signer: validators[2], delegation: 4800, deposit: 100000 },
                { signer: validators[3], delegation: 4700, deposit: 100000 },
                { signer: validators[4], delegation: 4600, deposit: 100000 },
                { signer: validators[5], delegation: 4500, deposit: 100000 },
                // Remain as candidates (Below delegationThreshold)
                { signer: validators[6], delegation: 900, deposit: 100000 },
                { signer: validators[7], delegation: 800, deposit: 100000 },
                { signer: validators[8], delegation: 700, deposit: 100000 },
                { signer: validators[9], delegation: 600, deposit: 100000 }
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(6);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 6).map(x => x.platformAddress.value)
            );
        });

        it("should keep possible authors after a term change", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const insufficientDelegationTx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[5].platformAddress,
                        50
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(insufficientDelegationTx);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                nodes[0].sdk,
                blockNumber
            );
            expect(termMetadata).not.to.be.null;
            expect(termMetadata!.currentTermId).to.be.equals(2);
            expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                blockNumber
            );
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk,
                blockNumber
            ))!;
            expect(possibleAuthors.length).is.equals(6);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 6).map(x => x.platformAddress.value)
            );
        });
    });

    describe("4. Nominate new candidate", async function() {
        const aliceIndex = 4;
        const { nodes } = withNodes(this, {
            promiseExpect,
            overrideParams: {
                maxNumOfValidators: 8,
                delegationThreshold: 1000,
                minDeposit: 10000
            },
            validators: [
                // Validators
                { signer: validators[0], delegation: 5000, deposit: 100000 },
                { signer: validators[1], delegation: 4900, deposit: 100000 },
                { signer: validators[2], delegation: 4800, deposit: 100000 },
                { signer: validators[3], delegation: 4700, deposit: 100000 },
                // Eligibles
                ...validators.slice(aliceIndex).map(signer => ({ signer }))
            ]
        });

        beforeEach(async function() {
            const possibleAuthors = (await stake.getPossibleAuthors(
                nodes[0].sdk
            ))!;
            expect(possibleAuthors.length).is.equals(4);
            expect(possibleAuthors.map(x => x.value)).contains.all.members(
                validators.slice(0, 4).map(x => x.platformAddress.value)
            );
        });

        [
            {
                description: "with enough deposit, but without delegation",
                deposit: 100000,
                delegation: 0
            },
            {
                description:
                    "with enough deposit, but without enough delegation",
                deposit: 100000,
                delegation: 900
            },
            {
                description: "without enough deposit and delegation",
                deposit: 5000,
                delegation: 900
            },
            {
                description:
                    "without enough deposit, but with enough delegation",
                deposit: 5000,
                delegation: 5000
            }
        ].forEach(({ description, deposit, delegation }) => {
            describe(description, async function() {
                it("should keep possible authors after a term change", async function() {
                    const termWaiter = setTermTestTimeout(this, {
                        terms: 1
                    });

                    const nominationTx = await nodes[4].sdk.rpc.chain.sendSignedTransaction(
                        stake
                            .createSelfNominateTransaction(
                                nodes[4].sdk,
                                deposit,
                                ""
                            )
                            .sign({
                                secret: validators[4].privateKey,
                                seq: await nodes[4].sdk.rpc.chain.getSeq(
                                    validators[4].platformAddress
                                ),
                                fee: 10
                            })
                    );
                    await nodes[0].waitForTx(nominationTx);

                    if (delegation > 0) {
                        const tx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                            stake
                                .createDelegateCCSTransaction(
                                    nodes[0].sdk,
                                    validators[4].platformAddress,
                                    delegation
                                )
                                .sign({
                                    secret: faucetSecret,
                                    seq: await nodes[0].sdk.rpc.chain.getSeq(
                                        faucetAddress
                                    ),
                                    fee: 10
                                })
                        );
                        await nodes[0].waitForTx(tx);
                    }

                    await termWaiter.waitNodeUntilTerm(nodes[0], {
                        target: 2,
                        termPeriods: 1
                    });
                    const blockNumber = await nodes[0].sdk.rpc.chain.getBestBlockNumber();
                    const termMetadata = await stake.getTermMetadata(
                        nodes[0].sdk,
                        blockNumber
                    );
                    expect(termMetadata).not.to.be.null;
                    expect(termMetadata!.currentTermId).to.be.equals(2);
                    expect(termMetadata!.lastTermFinishedBlockNumber).to.be.lte(
                        blockNumber
                    );
                    const possibleAuthors = (await stake.getPossibleAuthors(
                        nodes[0].sdk,
                        blockNumber
                    ))!;
                    expect(possibleAuthors.length).is.equals(4);
                    expect(
                        possibleAuthors.map(x => x.value)
                    ).contains.all.members(
                        validators.slice(0, 4).map(x => x.platformAddress.value)
                    );
                });
            });
        });
    });

    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
