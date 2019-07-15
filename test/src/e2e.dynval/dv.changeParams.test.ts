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

import { validators as originalValidators } from "../../tendermint.dynval/constants";
import { PromiseExpect } from "../helper/promise";
import { withNodes, changeParams, defaultParams } from "./setup";
import { faucetSecret, faucetAddress } from "../helper/constants";

chai.use(chaiAsPromised);

const [alice, , , ...otherDynValidators] = originalValidators;
const allDynValidators = [alice, ...otherDynValidators];

describe("Change commonParams", function() {
    const margin = 1.3;
    const promiseExpect = new PromiseExpect();
    const nodes = withNodes(this, {
        promiseExpect,
        validators: allDynValidators.map((signer, index) => ({
            signer,
            delegation: 5_000,
            deposit: 10_000_000 - index // tie-breaker
        }))
    });

    describe("Change term seconds", async function() {
        it("Term seconds should be changed", async function() {
            const checkingNode = nodes[0];
            const termSeconds = 10;

            this.slow(termSeconds * margin * 2 * 1000);
            this.timeout(termSeconds * 4 * 1000);

            const changeTxHash = await changeParams(checkingNode, 1, {
                ...defaultParams,
                termSeconds
            });

            await checkingNode.waitForTx(changeTxHash);
            await checkingNode.waitForTermChange(2, termSeconds * margin * 2);

            const termMetadataIn2ndTerm = (await stake.getTermMetadata(
                checkingNode.sdk
            ))!;
            const firstTermSeoncdBlockFromTheLast = (await checkingNode.sdk.rpc.chain.getBlock(
                termMetadataIn2ndTerm.lastTermFinishedBlockNumber - 1
            ))!;
            const firstTermSecondTimeStampFromTheLast =
                firstTermSeoncdBlockFromTheLast.timestamp;
            const firstTermlastBlock = (await checkingNode.sdk.rpc.chain.getBlock(
                termMetadataIn2ndTerm.lastTermFinishedBlockNumber
            ))!;
            const firstTermLastBlockTimeStamp = firstTermlastBlock.timestamp;

            // at least two checks are needed.
            await checkingNode.waitForTermChange(3, termSeconds * margin);

            const termMetadataIn3rdTerm = (await stake.getTermMetadata(
                checkingNode.sdk
            ))!;
            const SecondTermSeoncdBlockFromTheLast = (await checkingNode.sdk.rpc.chain.getBlock(
                termMetadataIn3rdTerm.lastTermFinishedBlockNumber - 1
            ))!;
            const secondTermSecondTimeStampFromTheLast =
                SecondTermSeoncdBlockFromTheLast.timestamp;
            const secondTermLastBlock = (await checkingNode.sdk.rpc.chain.getBlock(
                termMetadataIn3rdTerm.lastTermFinishedBlockNumber
            ))!;
            const secondTermLastBlockTimeStamp = secondTermLastBlock.timestamp;

            expect(
                Math.floor(firstTermSecondTimeStampFromTheLast / termSeconds) +
                    1
            ).to.be.equals(
                Math.floor(firstTermLastBlockTimeStamp / termSeconds)
            );
            expect(
                Math.floor(secondTermSecondTimeStampFromTheLast / termSeconds) +
                    1
            ).to.be.equals(
                Math.floor(secondTermLastBlockTimeStamp / termSeconds)
            );
        });
    });

    describe("Change minimum fee", async function() {
        it("Change minimum fee of pay transaction", async function() {
            const checkingNode = nodes[0];

            const secsPerBlock = 5;
            this.slow(secsPerBlock * 3 * 1000);
            this.timeout(secsPerBlock * 6 * 1000);

            const changeTxHash = await changeParams(checkingNode, 1, {
                ...defaultParams,
                minPayCost: 11
            });

            await checkingNode.waitForTx(changeTxHash);

            const tx = checkingNode.sdk.core
                .createPayTransaction({
                    recipient: allDynValidators[0].platformAddress,
                    quantity: 100
                })
                .sign({
                    secret: faucetSecret,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            try {
                await checkingNode.sdk.rpc.chain.sendSignedTransaction(tx);
            } catch (err) {
                expect(err.message).contains("Too Low Fee");
            }
        });
    });
});
