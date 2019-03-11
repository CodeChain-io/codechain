// Copyright 2018-2019 Kodebox, Inc.
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
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import { toHex } from "codechain-sdk/lib/utils";
import "mocha";
import {
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address
} from "../helper/constants";
import { PromiseExpect } from "../helper/promise";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

describe("Staking", function() {
    this.timeout(60_000);
    const promiseExpect = new PromiseExpect();
    let nodes: CodeChain[];

    beforeEach(async function() {
        this.timeout(60_000);

        const validatorAddresses = [
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        nodes = validatorAddresses.map(address => {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint-int.json`,
                argv: [
                    "--engine-signer",
                    address.toString(),
                    "--password-path",
                    "test/tendermint/password.json",
                    "--force-sealing",
                    "--no-discovery"
                ],
                additionalKeysPath: "tendermint/keys"
            });
        });
        await Promise.all(nodes.map(node => node.start()));
    });

    async function getAllStakingInfo() {
        const validatorAddresses = [
            faucetAddress,
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        const amounts = await promiseExpect.shouldFulfill(
            "get customActionData",
            Promise.all(
                validatorAddresses.map(addr =>
                    nodes[0].sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        ["Account", addr.accountId.toEncodeObject()]
                    )
                )
            )
        );
        const stakeholders = await promiseExpect.shouldFulfill(
            "get customActionData",
            nodes[0].sdk.rpc.engine.getCustomActionData(stakeActionHandlerId, [
                "StakeholderAddresses"
            ])
        );
        return { amounts, stakeholders };
    }

    it("should have proper initial stake tokens", async function() {
        const { amounts, stakeholders } = await getAllStakingInfo();
        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(100000)),
            null,
            null,
            null,
            null
        ]);

        expect(stakeholders).to.be.equal(
            toHex(RLP.encode([faucetAddress.accountId.toEncodeObject()]))
        );
    });

    it("should send stake tokens", async function() {
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[0].connect(nodes[2]),
                nodes[0].connect(nodes[3]),
                nodes[1].connect(nodes[2]),
                nodes[1].connect(nodes[3]),
                nodes[2].connect(nodes[3])
            ])
        );
        await promiseExpect.shouldFulfill(
            "wait peers",
            Promise.all([
                nodes[0].waitPeers(4 - 1),
                nodes[1].waitPeers(4 - 1),
                nodes[2].waitPeers(4 - 1),
                nodes[3].waitPeers(4 - 1)
            ])
        );

        const hash = await promiseExpect.shouldFulfill(
            "sendSignTransaction",
            nodes[0].sdk.rpc.chain.sendSignedTransaction(
                nodes[0].sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: Buffer.from(
                            RLP.encode([
                                1,
                                validator0Address.accountId.toEncodeObject(),
                                100
                            ])
                        )
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: await nodes[0].sdk.rpc.chain.getSeq(faucetAddress),
                        fee: 10
                    })
            )
        );

        const result = (await promiseExpect.shouldFulfill(
            "getTransactionResult",
            nodes[0].sdk.rpc.chain.getTransactionResult(hash, {
                timeout: 120 * 1000
            })
        ))!;

        expect(result).to.be.true;

        const { amounts, stakeholders } = await getAllStakingInfo();

        expect(amounts).to.be.deep.equal([
            toHex(RLP.encode(100000 - 100)),
            toHex(RLP.encode(100)),
            null,
            null,
            null
        ]);

        expect(stakeholders).to.be.equal(
            toHex(
                RLP.encode(
                    [
                        faucetAddress.accountId.toEncodeObject(),
                        validator0Address.accountId.toEncodeObject()
                    ].sort()
                )
            )
        );
    }).timeout(60_000);

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }
        await Promise.all(nodes.map(node => node.clean()));
        promiseExpect.checkFulfilled();
    });
});
