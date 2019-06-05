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
import { blake256 } from "codechain-sdk/lib/utils";
import "mocha";
import {
    aliceAddress,
    aliceSecret,
    carolSecret,
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address
} from "../helper/constants";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("Term change", function() {
    const chain = `${__dirname}/../scheme/solo-block-reward-50.json`;
    let node: CodeChain;

    beforeEach(async function() {
        node = new CodeChain({
            chain,
            argv: ["--author", validator0Address.toString(), "--force-sealing"]
        });
        await node.start();

        const tx = await node.sendPayTx({
            fee: 10,
            quantity: 100_000,
            recipient: aliceAddress
        });
        expect(await node.sdk.rpc.chain.containsTransaction(tx.hash())).be.true;
    });

    async function changeTermSeconds(metadataSeq: number, termSeconds: number) {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            10, // minPayCost
            10, // minSetRegularKeyCost
            10, // minCreateShardCost
            10, // minSetShardOwnersCost
            10, // minSetShardUsersCost
            10, // minWrapCccCost
            10, // minCustomCost
            10, // minStoreCost
            10, // minRemoveCost
            10, // minMintAssetCost
            10, // minTransferAssetCost
            10, // minChangeAssetSchemeCost
            10, // minIncreaseAssetSupplyCost
            10, // minComposeAssetCost
            10, // minDecomposeAssetCost
            10, // minUnwrapCccCost
            4194304, // maxBodySize
            16384, // snapshotPeriod
            termSeconds, // termSeconds
            0, // nominationExpiration
            0, // custodyPeriod
            0, // releasePeriod
            0, // maxNumOfValidators
            0, // minNumOfValidators
            0, // delegationThreshold
            0 // minDeposit
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            metadataSeq,
            newParams
        ];
        const message = blake256(RLP.encode(changeParams).toString("hex"));
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, aliceSecret)}`);
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, carolSecret)}`);

        {
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            );
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        }
    }

    it("initial term metadata", async function() {
        const params = await node.sdk.rpc.sendRpcRequest(
            "chain_getTermMetadata",
            [null]
        );
        expect(params).to.be.deep.equals([0, 0]);
    });

    async function waitForTermPeriodChange(termSeconds: number) {
        const lastBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const lastBlock = (await node.sdk.rpc.chain.getBlock(lastBlockNumber))!;

        let previousTs = lastBlock.timestamp;
        for (let count = 0; count < 20; count++) {
            await node.sdk.rpc.devel.startSealing();
            const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            const block = (await node.sdk.rpc.chain.getBlock(blockNumber))!;

            const currentTs = block.timestamp;
            const previousTermPeriod = Math.floor(previousTs / termSeconds);
            const currentTermPeriod = Math.floor(currentTs / termSeconds);
            if (previousTermPeriod !== currentTermPeriod) {
                return blockNumber;
            }
            previousTs = currentTs;
            await new Promise(resolve => setTimeout(resolve, 1000));
        }

        throw new Error("Timeout on waiting term period change");
    }

    it("can turn on term change", async function() {
        const TERM_SECONDS = 3;
        await changeTermSeconds(0, TERM_SECONDS);

        const blockNumber1 = await waitForTermPeriodChange(TERM_SECONDS);

        const params1 = await node.sdk.rpc.sendRpcRequest(
            "chain_getTermMetadata",
            [blockNumber1]
        );
        expect(params1).to.be.deep.equals([blockNumber1, 1]);

        const blockNumber2 = await waitForTermPeriodChange(TERM_SECONDS);

        const params2 = await node.sdk.rpc.sendRpcRequest(
            "chain_getTermMetadata",
            [blockNumber2]
        );
        expect(params2).to.be.deep.equals([blockNumber2, 2]);
    }).timeout(10_000);

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
        await node.clean();
    });
});
