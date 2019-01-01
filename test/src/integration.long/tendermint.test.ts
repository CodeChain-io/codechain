// Copyright 2018 Kodebox, Inc.
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

import CodeChain from "../helper/spawn";
import {
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address,
    stakeActionHandlerId,
    faucetAddress,
    faucetSecret
} from "../helper/constants";
import { toHex } from "codechain-sdk/lib/utils";

import "mocha";
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

const RLP = require("rlp");

describeSkippedInTravis("Tendermint ", function() {
    const BASE = 800;
    let nodes: CodeChain[];

    beforeEach(async function() {
        this.timeout(60_000);

        let validatorAddresses = [
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        nodes = validatorAddresses.map(address => {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint.json`,
                argv: [
                    "--engine-signer",
                    address.toString(),
                    "--password-path",
                    "test/tendermint/password.json",
                    "--force-sealing",
                    "--no-discovery"
                ],
                base: BASE,
                additionalKeysPath: "tendermint/keys"
            });
        });
        await Promise.all(nodes.map(node => node.start()));
    });

    it("Block generation", async function() {
        await Promise.all([
            nodes[0].connect(nodes[1]),
            nodes[0].connect(nodes[2]),
            nodes[0].connect(nodes[3]),
            nodes[1].connect(nodes[2]),
            nodes[1].connect(nodes[3]),
            nodes[2].connect(nodes[3])
        ]);
        await Promise.all([
            nodes[0].waitPeers(4 - 1),
            nodes[1].waitPeers(4 - 1),
            nodes[2].waitPeers(4 - 1),
            nodes[3].waitPeers(4 - 1)
        ]);

        await nodes[0].waitBlockNumber(2);
        await nodes[1].waitBlockNumber(2);
        await nodes[2].waitBlockNumber(2);
        await nodes[3].waitBlockNumber(2);
        await expect(
            nodes[0].sdk.rpc.chain.getBestBlockNumber()
        ).to.eventually.greaterThan(1);
    }).timeout(20_000);

    it("Block sync", async function() {
        await Promise.all([
            nodes[0].connect(nodes[1]),
            nodes[0].connect(nodes[2]),
            nodes[1].connect(nodes[2])
        ]);
        await Promise.all([
            nodes[0].waitPeers(3 - 1),
            nodes[1].waitPeers(3 - 1),
            nodes[2].waitPeers(3 - 1)
        ]);

        await nodes[0].waitBlockNumber(2);
        await nodes[1].waitBlockNumber(2);
        await nodes[2].waitBlockNumber(2);

        await Promise.all([
            nodes[0].disconnect(nodes[1]),
            nodes[0].disconnect(nodes[2])
        ]);

        // Now create blocks without nodes[0]. To create new blocks, the
        // nodes[4] should sync all message and participate in the network.

        await Promise.all([
            nodes[3].connect(nodes[1]),
            nodes[3].connect(nodes[2])
        ]);

        const best_number = await nodes[1].getBestBlockNumber();
        await nodes[1].waitBlockNumber(best_number + 1);
        await nodes[2].waitBlockNumber(best_number + 1);
        await nodes[3].waitBlockNumber(best_number + 1);
        await expect(
            nodes[3].sdk.rpc.chain.getBestBlockNumber()
        ).to.eventually.greaterThan(best_number);
    }).timeout(30_000);

    it("Gossip", async function() {
        await Promise.all([
            nodes[0].connect(nodes[1]),
            nodes[1].connect(nodes[2]),
            nodes[2].connect(nodes[3])
        ]);

        await nodes[0].waitBlockNumber(3);
        await nodes[1].waitBlockNumber(3);
        await nodes[2].waitBlockNumber(3);
        await nodes[3].waitBlockNumber(3);
        await expect(
            nodes[0].sdk.rpc.chain.getBestBlockNumber()
        ).to.eventually.greaterThan(1);
    }).timeout(20_000);

    describe("Staking", function() {
        async function getAllStakingInfo() {
            const validatorAddresses = [
                faucetAddress,
                validator0Address,
                validator1Address,
                validator2Address,
                validator3Address
            ];
            const amounts = await Promise.all(
                validatorAddresses.map(addr =>
                    nodes[0].sdk.rpc.engine.getCustomActionData(
                        stakeActionHandlerId,
                        [addr.accountId.toEncodeObject()]
                    )
                )
            );
            const stakeholders = await nodes[0].sdk.rpc.engine.getCustomActionData(
                stakeActionHandlerId,
                ["StakeholderAddresses"]
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
            await Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[0].connect(nodes[2]),
                nodes[0].connect(nodes[3]),
                nodes[1].connect(nodes[2]),
                nodes[1].connect(nodes[3]),
                nodes[2].connect(nodes[3])
            ]);
            await Promise.all([
                nodes[0].waitPeers(4 - 1),
                nodes[1].waitPeers(4 - 1),
                nodes[2].waitPeers(4 - 1),
                nodes[3].waitPeers(4 - 1)
            ]);

            const hash = await nodes[0].sdk.rpc.chain.sendSignedParcel(
                nodes[0].sdk.core
                    .createCustomParcel({
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
            );

            const invoice = (await nodes[0].sdk.rpc.chain.getParcelInvoice(
                hash,
                {
                    timeout: 120 * 1000
                }
            ))!;

            expect(invoice.error).to.be.undefined;
            expect(invoice.success).to.be.true;

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
        }).timeout(20_000);
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.testFailed(this.currentTest!.fullTitle()));
        }
        await Promise.all(nodes.map(node => node.clean()));
    });
});
