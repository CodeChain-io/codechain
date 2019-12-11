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
import {
    setTermTestTimeout,
    termThatIncludeTransaction,
    withNodes
} from "../setup";

describe("Dynamic Validator M -> M' (Changed the subset, M, M’ = maximum number)", function() {
    const promiseExpect = new PromiseExpect();

    const maxNumOfValidators = 3;
    const alice = maxNumOfValidators - 1; // will be replaced
    const bob = maxNumOfValidators; // will be elected by doing nothing
    const charlie = maxNumOfValidators + 1; // will be elected by delegating enough
    const dave = maxNumOfValidators + 2; // will be elected by depositing enough

    const nodeParams = {
        promiseExpect,
        overrideParams: {
            maxNumOfValidators,
            delegationThreshold: 1000,
            minDeposit: 10000
        },
        validators: [
            // Validators
            { signer: validators[0], delegation: 4200, deposit: 100000 },
            { signer: validators[1], delegation: 4100, deposit: 100000 },
            { signer: validators[2], delegation: 4000, deposit: 100000 }, // Alice
            // Candidates
            { signer: validators[3], delegation: 3000, deposit: 100000 }, // Bob
            { signer: validators[4], delegation: 100, deposit: 100000 }, // Charlie
            { signer: validators[5], delegation: 4100, deposit: 100 } // Dave
        ]
    };
    const charlieDelegationToCatchBob = 3000;
    const daveDepositToCatchBob = 100000;
    const aliceRevokeToBeLowerThanBob = 2000;
    const charlieDelegationToCatchAlice = 4000;
    const daveDepositToCatchAlice = 100000;

    async function expectAllValidatorsArePossibleAuthors(sdk: SDK) {
        const possibleAuthors = (await stake.getPossibleAuthors(sdk))!;
        expect(possibleAuthors).to.have.lengthOf(maxNumOfValidators);
        expect(possibleAuthors.map(x => x.toString())).to.includes.members(
            validators
                .slice(0, maxNumOfValidators)
                .map(x => x.platformAddress.toString())
        );
    }

    async function expectAliceIsReplacedBy(
        sdk: SDK,
        name: string,
        index: number
    ) {
        const possibleAuthors = await stake.getPossibleAuthors(sdk);
        expect(possibleAuthors).not.to.be.null;
        expect(possibleAuthors!).to.have.lengthOf(maxNumOfValidators);
        const authorAddresses = possibleAuthors!.map(x => x.toString());
        expect(authorAddresses).to.includes.members(
            validators.slice(0, alice).map(x => x.platformAddress.toString()),
            "Contains previous validators except for Alice"
        );
        expect(authorAddresses).not.contains(
            validators[alice].platformAddress.toString(),
            "Alice should not be elected as a validator"
        );
        expect(authorAddresses).contains(
            validators[index].platformAddress.toString(),
            `${name} should be elected as a validator instead of alice`
        );
    }

    describe("1. Jail one of the validator", async function() {
        const { nodes } = withNodes(this, {
            ...nodeParams,
            overrideParams: {
                maxNumOfValidators,
                delegationThreshold: 1000,
                minDeposit: 10000,
                custodyPeriod: 10,
                releasePeriod: 30
            },
            onBeforeEnable: async bootstrappingNodes => {
                await bootstrappingNodes[alice].clean(); // alice will be jailed!
            }
        });

        beforeEach(async function() {
            await expectAllValidatorsArePossibleAuthors(nodes[0].sdk);
        });

        it("Bob should be a validator when doing nothing", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            // Do nothing
            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });

            expect(
                (await stake.getJailed(nodes[0].sdk)).map(x =>
                    x.address.toString()
                )
            ).contains(
                validators[alice].platformAddress.toString(),
                "Alice should be jailed for doing nothing"
            );
            await expectAliceIsReplacedBy(nodes[0].sdk, "Bob", bob);
        });

        it("Charlie should be a validator when gets enough delegation", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const delegateToCharlie = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[charlie].platformAddress,
                        charlieDelegationToCatchBob
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(delegateToCharlie);
            await expect(
                termThatIncludeTransaction(nodes[0].sdk, delegateToCharlie)
            ).eventually.equal(1);
            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });

            expect(
                (await stake.getJailed(nodes[0].sdk)).map(x =>
                    x.address.toString()
                )
            ).contains(
                validators[alice].platformAddress.toString(),
                "Alice should be jailed for doing nothing"
            );
            await expectAliceIsReplacedBy(nodes[0].sdk, "Charlie", charlie);
        });

        it("Dave should be a validator when deposit enough", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const depositDave = await nodes[
                dave
            ].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createSelfNominateTransaction(
                        nodes[dave].sdk,
                        daveDepositToCatchBob,
                        ""
                    )
                    .sign({
                        secret: validators[dave].privateKey,
                        seq: await nodes[dave].sdk.rpc.chain.getSeq(
                            validators[dave].platformAddress
                        ),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(depositDave);
            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });

            expect(
                (await stake.getJailed(nodes[0].sdk)).map(x =>
                    x.address.toString()
                )
            ).contains(
                validators[alice].platformAddress.toString(),
                "Alice should be jailed for doing nothing"
            );
            await expectAliceIsReplacedBy(nodes[0].sdk, "Dave", dave);
        });
    });

    describe("2. Revoke the validator", async function() {
        const { nodes } = withNodes(this, nodeParams);

        beforeEach(async function() {
            this.timeout(5000);

            await expectAllValidatorsArePossibleAuthors(nodes[0].sdk);

            const revokeTx = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createRevokeTransaction(
                        nodes[0].sdk,
                        validators[alice].platformAddress,
                        aliceRevokeToBeLowerThanBob
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(revokeTx);
        });

        it("Bob should be a validator when doing nothing", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            // Do nothing
            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            await expectAliceIsReplacedBy(nodes[0].sdk, "Bob", bob);
        });

        it("Charlie should be a validator when gets enough delegation", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const delegateToCharlie = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[charlie].platformAddress,
                        charlieDelegationToCatchBob
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(delegateToCharlie);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            await expectAliceIsReplacedBy(nodes[0].sdk, "Charlie", charlie);
        });

        it("Dave should be a validator when deposit enough", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const depositDave = await nodes[
                dave
            ].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createSelfNominateTransaction(
                        nodes[dave].sdk,
                        daveDepositToCatchBob,
                        ""
                    )
                    .sign({
                        secret: validators[dave].privateKey,
                        seq: await nodes[dave].sdk.rpc.chain.getSeq(
                            validators[dave].platformAddress
                        ),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(depositDave);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            await expectAliceIsReplacedBy(nodes[0].sdk, "Dave", dave);
        });
    });

    describe("3. Change the order of candidates", async function() {
        const { nodes } = withNodes(this, nodeParams);

        beforeEach(async function() {
            await expectAllValidatorsArePossibleAuthors(nodes[0].sdk);
        });

        it("Charlie should be a validator when gets enough delegation", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const delegateToCharlie = await nodes[0].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createDelegateCCSTransaction(
                        nodes[0].sdk,
                        validators[charlie].platformAddress,
                        charlieDelegationToCatchAlice
                    )
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(delegateToCharlie);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            await expectAliceIsReplacedBy(nodes[0].sdk, "Charlie", charlie);
        });

        it("Dave should be a validator when deposit enough", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1
            });

            const depositDave = await nodes[
                dave
            ].sdk.rpc.chain.sendSignedTransaction(
                stake
                    .createSelfNominateTransaction(
                        nodes[dave].sdk,
                        daveDepositToCatchAlice,
                        ""
                    )
                    .sign({
                        secret: validators[dave].privateKey,
                        seq: await nodes[dave].sdk.rpc.chain.getSeq(
                            validators[dave].platformAddress
                        ),
                        fee: 10
                    })
            );
            await nodes[0].waitForTx(depositDave);

            await termWaiter.waitNodeUntilTerm(nodes[0], {
                target: 2,
                termPeriods: 1
            });
            await expectAliceIsReplacedBy(nodes[0].sdk, "Dave", dave);
        });
    });

    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
