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
import { H256, PlatformAddress, U64 } from "codechain-primitives/lib";
import { blake256 } from "codechain-sdk/lib/utils";
import "mocha";
import {
    aliceAddress,
    aliceSecret,
    bobSecret,
    carolAddress,
    carolSecret,
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address
} from "../helper/constants";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

chai.use(chaiAsPromised);

describe("ChangeParams", function() {
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

    it("change", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
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

        await expect(node.sendPayTx({ fee: 10 })).rejectedWith(/Too Low Fee/);

        const params = await node.sdk.rpc.sendRpcRequest(
            "chain_getCommonParams",
            [null]
        );
        expect(U64.ensure(params.minPayCost)).to.be.deep.equal(new U64(11));
    });

    it("cannot change the network id", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "cc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams
        ];
        const message = blake256(RLP.encode(changeParams).toString("hex"));
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, aliceSecret)}`);
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, carolSecret)}`);

        const tx = node.sdk.core
            .createCustomTransaction({
                handlerId: stakeActionHandlerId,
                bytes: RLP.encode(changeParams)
            })
            .sign({
                secret: faucetSecret,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                fee: 10
            });
        await expect(node.sdk.rpc.chain.sendSignedTransaction(tx)).rejectedWith(
            /network id/
        );
    });

    it("should keep default common params value", async function() {
        const params = await node.sdk.rpc.sendRpcRequest(
            "chain_getCommonParams",
            [null]
        );
        expect(U64.ensure(params.minPayCost)).to.be.deep.equal(new U64(10));
    });

    it("the parameter is applied from the next block", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams
        ];
        const message = blake256(RLP.encode(changeParams).toString("hex"));
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, aliceSecret)}`);
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, bobSecret)}`);
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, carolSecret)}`);

        {
            await node.sdk.rpc.devel.stopSealing();
            const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const changeHash = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq,
                        fee: 10
                    })
            );
            const pay = await node.sendPayTx({ seq: seq + 1, fee: 10 });
            await node.sdk.rpc.devel.startSealing();
            expect(await node.sdk.rpc.chain.containsTransaction(changeHash)).be
                .true;
            expect(await node.sdk.rpc.chain.containsTransaction(pay.hash())).be
                .true;
            expect(await node.sdk.rpc.chain.getBestBlockNumber()).equal(
                blockNumber + 1
            );
        }

        await expect(node.sendPayTx({ fee: 10 })).rejectedWith(/Too Low Fee/);
    });

    it("the parameter changed twice in the same block", async function() {
        const newParams1 = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const newParams2 = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            5, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams1: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams1
        ];
        const changeParams2: (number | string | (number | string)[])[] = [
            0xff,
            1,
            newParams2
        ];
        const message1 = blake256(RLP.encode(changeParams1).toString("hex"));
        changeParams1.push(
            `0x${node.sdk.util.signEcdsa(message1, aliceSecret)}`
        );
        changeParams1.push(`0x${node.sdk.util.signEcdsa(message1, bobSecret)}`);
        changeParams1.push(
            `0x${node.sdk.util.signEcdsa(message1, carolSecret)}`
        );
        const message2 = blake256(RLP.encode(changeParams2).toString("hex"));
        changeParams2.push(
            `0x${node.sdk.util.signEcdsa(message2, aliceSecret)}`
        );
        changeParams2.push(`0x${node.sdk.util.signEcdsa(message2, bobSecret)}`);
        changeParams2.push(
            `0x${node.sdk.util.signEcdsa(message2, carolSecret)}`
        );

        {
            await node.sdk.rpc.devel.stopSealing();
            const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const changeHash1 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams1)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq,
                        fee: 10
                    })
            );
            const changeHash2 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams2)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: seq + 1,
                        fee: 10
                    })
            );
            await node.sdk.rpc.devel.startSealing();
            expect(await node.sdk.rpc.chain.containsTransaction(changeHash1)).be
                .true;
            expect(await node.sdk.rpc.chain.containsTransaction(changeHash2)).be
                .true;
            expect(await node.sdk.rpc.chain.getBestBlockNumber()).equal(
                blockNumber + 1
            );
        }

        const pay = await node.sendPayTx({ fee: 5 });
        expect(await node.sdk.rpc.chain.containsTransaction(pay.hash())).be
            .true;
        await expect(node.sendPayTx({ fee: 4 })).rejectedWith(/Too Low Fee/);
    });

    it("cannot reuse the same signature", async function() {
        const newParams1 = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const newParams2 = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            5, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams1: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams1
        ];
        const changeParams2: (number | string | (number | string)[])[] = [
            0xff,
            1,
            newParams2
        ];
        const message1 = blake256(RLP.encode(changeParams1).toString("hex"));
        changeParams1.push(
            `0x${node.sdk.util.signEcdsa(message1, aliceSecret)}`
        );
        changeParams1.push(`0x${node.sdk.util.signEcdsa(message1, bobSecret)}`);
        changeParams1.push(
            `0x${node.sdk.util.signEcdsa(message1, carolSecret)}`
        );
        const message2 = blake256(RLP.encode(changeParams2).toString("hex"));
        changeParams2.push(
            `0x${node.sdk.util.signEcdsa(message2, aliceSecret)}`
        );
        changeParams2.push(`0x${node.sdk.util.signEcdsa(message2, bobSecret)}`);
        changeParams2.push(
            `0x${node.sdk.util.signEcdsa(message2, carolSecret)}`
        );

        {
            await node.sdk.rpc.devel.stopSealing();
            const blockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
            const changeHash1 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams1)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq,
                        fee: 10
                    })
            );
            const changeHash2 = await node.sdk.rpc.chain.sendSignedTransaction(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams2)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq: seq + 1,
                        fee: 10
                    })
            );
            await node.sdk.rpc.devel.startSealing();
            expect(await node.sdk.rpc.chain.containsTransaction(changeHash1)).be
                .true;
            expect(await node.sdk.rpc.chain.containsTransaction(changeHash2)).be
                .true;
            expect(await node.sdk.rpc.chain.getBestBlockNumber()).equal(
                blockNumber + 1
            );
        }

        const pay = await node.sendPayTx({ fee: 5 });
        expect(await node.sdk.rpc.chain.containsTransaction(pay.hash())).be
            .true;
        await expect(node.sendPayTx({ fee: 4 })).rejectedWith(/Too Low Fee/);
    });

    it("cannot change params with insufficient stakes", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
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

        {
            await node.sendSignedTransactionExpectedToFail(
                node.sdk.core
                    .createCustomTransaction({
                        handlerId: stakeActionHandlerId,
                        bytes: RLP.encode(changeParams)
                    })
                    .sign({
                        secret: faucetSecret,
                        seq:
                            (await node.sdk.rpc.chain.getSeq(faucetAddress)) +
                            1,
                        fee: 10
                    }),
                { error: "Invalid transaction seq Expected 1, found 0" }
            );
        }
    });

    it("the amount of stakes not the number of stakeholders", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];
        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams
        ];
        const message = blake256(RLP.encode(changeParams).toString("hex"));
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, bobSecret)}`);
        changeParams.push(`0x${node.sdk.util.signEcdsa(message, carolSecret)}`);

        const tx = node.sdk.core
            .createCustomTransaction({
                handlerId: stakeActionHandlerId,
                bytes: RLP.encode(changeParams)
            })
            .sign({
                secret: faucetSecret,
                seq: (await node.sdk.rpc.chain.getSeq(faucetAddress)) + 1,
                fee: 10
            });
        await node.sendSignedTransactionExpectedToFail(tx, {
            error: "Insufficient stakes:"
        });
    });

    it("needs more than half to change params", async function() {
        const newParams = [
            0x20, // maxExtraDataSize
            0x0400, // maxAssetSchemeMetadataSize
            0x0100, // maxTransferMetadataSize
            0x0200, // maxTextContentSize
            "tc", // networkID
            11, // minPayCost
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
            16384 // snapshotPeriod
        ];

        const changeParams: (number | string | (number | string)[])[] = [
            0xff,
            0,
            newParams
        ];
        {
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, bobSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: (await node.sdk.rpc.chain.getSeq(faucetAddress)) + 1,
                    fee: 10
                });
            await node.sendSignedTransactionExpectedToFail(tx, {
                error: "Insufficient"
            });
        }

        await sendStakeToken({
            node,
            senderAddress: aliceAddress,
            senderSecret: aliceSecret,
            receiverAddress: carolAddress,
            quantity: 1,
            fee: 1000
        });

        {
            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(tx);
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.be.null;
        }

        await expect(node.sendPayTx({ fee: 10 })).rejectedWith(/Too Low Fee/);
    });

    describe("with stake parameters", async function() {
        it("change", async function() {
            const newParams = [
                0x20, // maxExtraDataSize
                0x0400, // maxAssetSchemeMetadataSize
                0x0100, // maxTransferMetadataSize
                0x0200, // maxTextContentSize
                "tc", // networkID
                11, // minPayCost
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
                16384 // snapshotPeriod
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

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
                expect(await node.sdk.rpc.chain.containsTransaction(hash)).be
                    .true;
            }

            await expect(node.sendPayTx({ fee: 10 })).rejectedWith(
                /Too Low Fee/
            );

            const params = await node.sdk.rpc.sendRpcRequest(
                "chain_getCommonParams",
                [null]
            );
            expect(U64.ensure(params.minPayCost)).to.be.deep.equal(new U64(11));
        });

        it("nomination expiration cannot be zero", async function() {
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
                100, // termSeconds
                0, // nominationExpiration
                10, // custodyPeriod
                30, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/nomination expiration/);
        });

        it.skip("custody period cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                0, // custodyPeriod
                30, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/custody period/);
        });

        it.skip("release period cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                0, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/release period/);
        });

        it.skip("A release period cannot be equal to a custody period", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                10, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            try {
                await node.sdk.rpc.chain.sendSignedTransaction(tx);
                expect.fail("The transaction must fail");
            } catch (err) {
                expect(err.message).contains("release period");
                expect(err.message).contains("custody period");
            }
        });

        it("min deposit cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                0, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/minimum deposit/);
        });

        it("delegation threshold cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                101, // maxNumOfValidators
                100, // minNumOfValidators
                0, // delegationThreshold
                100, // minDeposit
                100 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/delegation threshold/);
        });

        it("min number of validators cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                101, // maxNumOfValidators
                0, // minNumOfValidators
                100, // delegationThreshold
                100, // minDeposit
                100 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/minimum number of validators/);
        });

        it("max number of validators cannot be zero", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                0, // maxNumOfValidators
                10, // minNumOfValidators
                100, // delegationThreshold
                100, // minDeposit
                100 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            await expect(
                node.sdk.rpc.chain.sendSignedTransaction(tx)
            ).rejectedWith(/maximum number of validators/);
        });

        it("The maximum number of candidates cannot be equal to the minimum number of candidates", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                99, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                128 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            try {
                await node.sdk.rpc.chain.sendSignedTransaction(tx);
                expect.fail("The transaction must fail");
            } catch (err) {
                expect(err.message).contains("maximum number of validators");
                expect(err.message).contains("minimum number of validators");
            }
        });

        it("The candidate metadata limit should be smaller than the text size limit", async function() {
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
                100, // termSeconds
                10, // nominationExpiration
                10, // custodyPeriod
                20, // releasePeriod
                100, // maxNumOfValidators
                100, // minNumOfValidators
                4, // delegationThreshold
                1000, // minDeposit
                0x0200 // maxCandidateMetadataSize
            ];
            const changeParams: (number | string | (number | string)[])[] = [
                0xff,
                0,
                newParams
            ];
            const message = blake256(RLP.encode(changeParams).toString("hex"));
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, aliceSecret)}`
            );
            changeParams.push(
                `0x${node.sdk.util.signEcdsa(message, carolSecret)}`
            );

            const tx = node.sdk.core
                .createCustomTransaction({
                    handlerId: stakeActionHandlerId,
                    bytes: RLP.encode(changeParams)
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            try {
                await node.sdk.rpc.chain.sendSignedTransaction(tx);
                expect.fail("The transaction must fail");
            } catch (err) {
                expect(err.message).contains("candidate metadata size");
                expect(err.message).contains("text limit");
            }
        });
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

async function sendStakeToken(params: {
    node: CodeChain;
    senderAddress: PlatformAddress;
    senderSecret: string;
    receiverAddress: PlatformAddress;
    quantity: number;
    fee?: number;
    seq?: number;
}): Promise<H256> {
    const {
        fee = 10,
        node,
        senderAddress,
        receiverAddress,
        senderSecret,
        quantity
    } = params;
    const { seq = await node.sdk.rpc.chain.getSeq(senderAddress) } = params;

    return node.sdk.rpc.chain.sendSignedTransaction(
        node.sdk.core
            .createCustomTransaction({
                handlerId: stakeActionHandlerId,
                bytes: Buffer.from(
                    RLP.encode([
                        1,
                        receiverAddress.accountId.toEncodeObject(),
                        quantity
                    ])
                )
            })
            .sign({
                secret: senderSecret,
                seq,
                fee
            })
    );
}
