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

import { validators } from "../../../tendermint.dynval/constants";
import { PromiseExpect } from "../../helper/promise";
import { setTermTestTimeout, withNodes } from "../setup";

chai.use(chaiAsPromised);

describe("Jail state transition test", function() {
    const promiseExpect = new PromiseExpect();

    const alice = validators[1];
    const { nodes } = withNodes(this, {
        promiseExpect,
        validators: validators.slice(0, 5).map((signer, index) => ({
            signer,
            delegation: 5000,
            deposit: 10_000_000 - index // tie-breaker
        })),
        overrideParams: {
            custodyPeriod: 2,
            releasePeriod: 3
        },
        onBeforeEnable: async bootstrappingNodes => {
            await bootstrappingNodes[1].clean(); // alice will be jailed!
        }
    });

    async function isValidator(
        entity: (typeof validators)[number]
    ): Promise<boolean> {
        const activated = await stake.getValidators(nodes[0].sdk);
        return activated.some(v => v.pubkey.toString() === entity.publicKey);
    }

    async function isCandidate(
        entity: (typeof validators)[number]
    ): Promise<boolean> {
        const candidates = await stake.getCandidates(nodes[0].sdk);
        return candidates.some(c => c.pubkey.toString() === entity.publicKey);
    }

    async function isBanned(
        entity: (typeof validators)[number]
    ): Promise<boolean> {
        const banned = await stake.getBanned(nodes[0].sdk);
        return banned.some(
            b => b.getAccountId().toString() === entity.accountId
        );
    }

    async function isPrisoner(
        entity: (typeof validators)[number]
    ): Promise<boolean> {
        const prisoners = await stake.getJailed(nodes[0].sdk);
        return prisoners.some(
            p => p.address.toString() === entity.platformAddress.toString()
        );
    }

    async function isEligible(
        entity: (typeof validators)[number]
    ): Promise<boolean> {
        return !(
            (await isValidator(entity)) ||
            (await isCandidate(entity)) ||
            (await isPrisoner(entity)) ||
            (await isBanned(entity))
        );
    }

    beforeEach(async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 1
        });

        // Wait until alice is sent to jail
        const node = nodes[0];
        await termWaiter.waitNodeUntilTerm(node, { target: 2, termPeriods: 1 });
        expect(await isPrisoner(alice), "Alice should be in prison").to.be.true;
    });

    it("Should be released if RELEASE_PERIOD have passed", async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 3
        });

        const node = nodes[0];
        await termWaiter.waitNodeUntilTerm(node, { target: 5, termPeriods: 3 });

        expect(await isEligible(alice), "Alice should have been released").to.be
            .true;
    });

    it("Should become a candiate if a self-nomination was sent after CUSTODY_PERIOD", async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 3
        });

        const node = nodes[0];

        await termWaiter.waitNodeUntilTerm(node, { target: 4, termPeriods: 2 });

        const nomination = await stake.createSelfNominateTransaction(
            node.sdk,
            10_000_000,
            ""
        );
        const hash = await node.sdk.rpc.chain.sendSignedTransaction(
            nomination.sign({
                secret: alice.privateKey,
                seq: await node.sdk.rpc.chain.getSeq(alice.platformAddress),
                fee: 10
            })
        );
        await node.waitForTx(hash);

        await termWaiter.waitNodeUntilTerm(node, { target: 5, termPeriods: 1 });
        expect(await isCandidate(alice)).to.be.true;
    });

    it("Should stay in jail if a self-nomination was sent to early", async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 1
        });

        const node = nodes[0];
        const nomination = await stake.createSelfNominateTransaction(
            node.sdk,
            10_000_000,
            ""
        );
        const hash = await node.sdk.rpc.chain.sendSignedTransaction(
            nomination.sign({
                secret: alice.privateKey,
                seq: await node.sdk.rpc.chain.getSeq(alice.platformAddress),
                fee: 10
            })
        );
        try {
            await node.waitForTx(hash);
            expect.fail("Self nomination should not be accepted");
        } catch (e) {
            expect(e.message).to.contain("Account is still in custody");
        }
        await termWaiter.waitNodeUntilTerm(node, { target: 3, termPeriods: 1 });
        expect(await isPrisoner(alice)).to.be.true;
    });

    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
