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

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

const RLP = require("rlp");

describeSkippedInTravis("Tendermint ", function() {
    const promiseExpect = new PromiseExpect();
    const BASE = 800;
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
                base: BASE,
                additionalKeysPath: "tendermint/keys"
            });
        });
        await Promise.all(nodes.map(node => node.start()));
    });

    it("Block generation", async function() {
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

        await promiseExpect.shouldFulfill(
            "block generation",
            Promise.all([
                nodes[0].waitBlockNumber(2),
                nodes[1].waitBlockNumber(2),
                nodes[2].waitBlockNumber(2),
                nodes[3].waitBlockNumber(2)
            ])
        );

        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(1);
    }).timeout(20_000);

    it("Block generation with restart", async function() {
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

        await promiseExpect.shouldFulfill(
            "block generation",
            Promise.all([
                nodes[0].waitBlockNumber(2),
                nodes[1].waitBlockNumber(2),
                nodes[2].waitBlockNumber(2),
                nodes[3].waitBlockNumber(2)
            ])
        );

        await promiseExpect.shouldFulfill(
            "stop",
            Promise.all([
                nodes[0].clean(),
                nodes[1].clean(),
                nodes[2].clean(),
                nodes[3].clean()
            ])
        );

        await promiseExpect.shouldFulfill(
            "start",
            Promise.all([
                nodes[0].start(),
                nodes[1].start(),
                nodes[2].start(),
                nodes[3].start()
            ])
        );

        const bestBlockNumber = await promiseExpect.shouldFulfill(
            "BestBlockNUmber",
            nodes[0].getBestBlockNumber()
        );

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

        await promiseExpect.shouldFulfill(
            "block generation",
            Promise.all([
                nodes[0].waitBlockNumber(bestBlockNumber + 2),
                nodes[1].waitBlockNumber(bestBlockNumber + 2),
                nodes[2].waitBlockNumber(bestBlockNumber + 2),
                nodes[3].waitBlockNumber(bestBlockNumber + 2)
            ])
        );

        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(bestBlockNumber + 1);
    }).timeout(40_000);

    it("Block generation with transaction", async function() {
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

        await promiseExpect.shouldFulfill(
            "payTx",
            Promise.all([
                nodes[0].sendPayTx({ seq: 0 }),
                nodes[0].sendPayTx({ seq: 1 }),
                nodes[0].sendPayTx({ seq: 2 })
            ])
        );

        await promiseExpect.shouldFulfill(
            "block generation",
            Promise.all([
                nodes[0].waitBlockNumber(2),
                nodes[1].waitBlockNumber(2),
                nodes[2].waitBlockNumber(2),
                nodes[3].waitBlockNumber(2)
            ])
        );

        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(1);
    }).timeout(20_000);

    it("Block sync", async function() {
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[0].connect(nodes[2]),
                nodes[1].connect(nodes[2])
            ])
        );
        await promiseExpect.shouldFulfill(
            "wait peers",
            Promise.all([
                nodes[0].waitPeers(3 - 1),
                nodes[1].waitPeers(3 - 1),
                nodes[2].waitPeers(3 - 1)
            ])
        );

        await promiseExpect.shouldFulfill(
            "wait blocknumber",
            Promise.all([
                nodes[0].waitBlockNumber(2),
                nodes[1].waitBlockNumber(2),
                nodes[2].waitBlockNumber(2)
            ])
        );

        await promiseExpect.shouldFulfill(
            "disconnect",
            Promise.all([
                nodes[0].disconnect(nodes[1]),
                nodes[0].disconnect(nodes[2])
            ])
        );

        // Now create blocks without nodes[0]. To create new blocks, the
        // nodes[4] should sync all message and participate in the network.

        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[3].connect(nodes[1]),
                nodes[3].connect(nodes[2])
            ])
        );

        const bestNumber = await promiseExpect.shouldFulfill(
            "best blocknumber",
            nodes[1].getBestBlockNumber()
        );
        await promiseExpect.shouldFulfill(
            "best blocknumber",
            Promise.all([
                nodes[1].waitBlockNumber(bestNumber + 1),
                nodes[2].waitBlockNumber(bestNumber + 1),
                nodes[3].waitBlockNumber(bestNumber + 1)
            ])
        );
        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[3].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(bestNumber);
    }).timeout(30_000);

    it("Gossip", async function() {
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[0].connect(nodes[1]),
                nodes[1].connect(nodes[2]),
                nodes[2].connect(nodes[3])
            ])
        );

        await promiseExpect.shouldFulfill(
            "wait blocknumber",
            Promise.all([
                nodes[0].waitBlockNumber(3),
                nodes[1].waitBlockNumber(3),
                nodes[2].waitBlockNumber(3),
                nodes[3].waitBlockNumber(3)
            ])
        );
        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(1);
    }).timeout(20_000);

    it("Gossip with not-permissioned node", async function() {
        function createNodeWihtOutSigner() {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint-int.json`,
                argv: [
                    "--no-miner",
                    "--password-path",
                    "test/tendermint/password.json",
                    "--no-discovery"
                ],
                base: BASE,
                additionalKeysPath: "tendermint/keys"
            });
        }

        nodes.push(createNodeWihtOutSigner());
        nodes.push(createNodeWihtOutSigner());
        await Promise.all([nodes[4].start(), nodes[5].start()]);

        // 4 <-> 5
        // 0 <-> 4, 1 <-> 4
        // 2 <-> 5, 3 <-> 5
        await promiseExpect.shouldFulfill(
            "connect",
            Promise.all([
                nodes[4].connect(nodes[5]),
                nodes[4].connect(nodes[0]),
                nodes[4].connect(nodes[1]),
                nodes[5].connect(nodes[2]),
                nodes[5].connect(nodes[3])
            ])
        );

        await promiseExpect.shouldFulfill(
            "wait blocknumber",
            Promise.all([
                nodes[0].waitBlockNumber(3),
                nodes[1].waitBlockNumber(3),
                nodes[2].waitBlockNumber(3),
                nodes[3].waitBlockNumber(3),
                nodes[4].waitBlockNumber(3),
                nodes[5].waitBlockNumber(3)
            ])
        );
        await expect(
            promiseExpect.shouldFulfill(
                "best blocknumber",
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            )
        ).to.eventually.greaterThan(1);
    }).timeout(30_000);

    describe("Staking", function() {
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
                            [addr.accountId.toEncodeObject()]
                        )
                    )
                )
            );
            const stakeholders = await promiseExpect.shouldFulfill(
                "get customActionData",
                nodes[0].sdk.rpc.engine.getCustomActionData(
                    stakeActionHandlerId,
                    ["StakeholderAddresses"]
                )
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
                            seq: await nodes[0].sdk.rpc.chain.getSeq(
                                faucetAddress
                            ),
                            fee: 10
                        })
                )
            );

            const invoice = (await promiseExpect.shouldFulfill(
                "getInvoice",
                nodes[0].sdk.rpc.chain.getInvoice(hash, {
                    timeout: 120 * 1000
                })
            ))!;

            expect(invoice).to.be.true;

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
        promiseExpect.checkFulfilled();
    });
});
