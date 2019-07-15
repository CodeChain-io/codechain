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

import { expect } from "chai";
import {
    AssetAddress,
    MintAsset,
    PlatformAddress,
    SignedTransaction,
    U64
} from "codechain-sdk/lib/core/classes";
import "mocha";
import {
    aliceAddress,
    faucetAccointId,
    faucetAddress,
    faucetSecret
} from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("Unwrap CCC", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    describe("Wrap CCC with P2PKHBurnAddress", function() {
        let recipient: AssetAddress;
        let wrapTransaction: SignedTransaction;
        const quantity = 100;
        beforeEach(async function() {
            recipient = await node.createP2PKHBurnAddress();
            wrapTransaction = node.sdk.core
                .createWrapCCCTransaction({
                    shardId: 0,
                    recipient,
                    quantity,
                    payer: PlatformAddress.fromAccountId(faucetAccointId, {
                        networkId: "tc"
                    })
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            const blockNumber = await node.getBestBlockNumber();
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                wrapTransaction
            );
            await node.waitBlockNumber(blockNumber + 1);
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        });

        it("Unwrap successful", async function() {
            const beforeAliceBalance = await node.sdk.rpc.chain.getBalance(
                aliceAddress
            );
            const tx = node.sdk.core.createUnwrapCCCTransaction({
                burn: wrapTransaction.getAsset(),
                receiver: aliceAddress
            });
            await node.signTransactionBurn(tx, 0);
            const hash = await node.sendAssetTransaction(tx);
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
            expect(
                (await node.sdk.rpc.chain.getBalance(
                    aliceAddress
                )).toEncodeObject()
            ).eq(
                U64.plus(
                    beforeAliceBalance,
                    wrapTransaction.getAsset().quantity
                )
                    .plus(2 /* stake share */)
                    .toEncodeObject()
            );
        });
    });

    describe("with P2PKHAddress", function() {
        let recipient: AssetAddress;
        let wrapTransaction: SignedTransaction;
        const quantity = 100;
        beforeEach(async function() {
            recipient = await node.createP2PKHAddress();
            wrapTransaction = node.sdk.core
                .createWrapCCCTransaction({
                    shardId: 0,
                    recipient,
                    quantity,
                    payer: PlatformAddress.fromAccountId(faucetAccointId, {
                        networkId: "tc"
                    })
                })
                .sign({
                    secret: faucetSecret,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });

            const blockNumber = await node.getBestBlockNumber();
            const hash = await node.sdk.rpc.chain.sendSignedTransaction(
                wrapTransaction
            );
            await node.waitBlockNumber(blockNumber + 1);
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        });

        it("Transfer then Unwrap successful", async function() {
            const recipientBurn = await node.createP2PKHBurnAddress();
            const asset1 = wrapTransaction.getAsset();

            const transferTx = node.sdk.core.createTransferAssetTransaction();
            transferTx.addInputs(asset1);
            transferTx.addOutputs({
                assetType: asset1.assetType,
                shardId: asset1.shardId,
                recipient: recipientBurn,
                quantity
            });
            await node.signTransactionInput(transferTx, 0);
            const hash1 = await node.sendAssetTransaction(transferTx);
            expect(await node.sdk.rpc.chain.getTransaction(hash1)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash1)).be.true;

            const asset2 = await node.sdk.rpc.chain.getAsset(
                transferTx.tracker(),
                0,
                asset1.shardId
            );

            const beforeAliceBalance = await node.sdk.rpc.chain.getBalance(
                aliceAddress
            );
            const unwrapTx = node.sdk.core.createUnwrapCCCTransaction({
                burn: asset2!,
                receiver: aliceAddress
            });
            await node.signTransactionBurn(unwrapTx, 0);
            const hash2 = await node.sendAssetTransaction(unwrapTx);
            expect(await node.sdk.rpc.chain.getTransaction(hash2)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash2)).be.true;

            expect(
                (await node.sdk.rpc.chain.getBalance(
                    aliceAddress
                )).toEncodeObject()
            ).eq(
                U64.plus(beforeAliceBalance, asset2!.quantity)
                    .plus(2 /* stake share */)
                    .toEncodeObject()
            );
        });
    });

    describe("With minted asset (not wrapped CCC)", function() {
        let recipient: AssetAddress;
        let mintTx: MintAsset;
        const supply = 100;
        beforeEach(async function() {
            recipient = await node.createP2PKHBurnAddress();
            const scheme = node.sdk.core.createAssetScheme({
                shardId: 0,
                metadata: "",
                supply
            });
            mintTx = node.sdk.core.createMintAssetTransaction({
                scheme,
                recipient
            });
            const hash = await node.sendAssetTransaction(mintTx);
            expect(await node.sdk.rpc.chain.getTransaction(hash)).not.null;
            expect(await node.sdk.rpc.chain.containsTransaction(hash)).be.true;
        });

        it("Invalid asset type", async function() {
            const tx = node.sdk.core.createUnwrapCCCTransaction({
                burn: mintTx.getMintedAsset(),
                receiver: aliceAddress
            });
            await node.signTransactionBurn(tx, 0);
            const beforeAliceBalance = await node.sdk.rpc.chain.getBalance(
                aliceAddress
            );
            try {
                await node.sendAssetTransaction(tx);
                expect.fail();
            } catch (e) {
                expect(e).is.similarTo(ERROR.INVALID_TX_ASSET_TYPE);
            }
            expect(
                (await node.sdk.rpc.chain.getBalance(
                    aliceAddress
                )).toEncodeObject()
            ).eq(beforeAliceBalance.toEncodeObject());
        });
    });

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
    });

    after(async function() {
        await node.clean();
    });
});
