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

import {
    H256,
    H512,
    U64,
    AssetMintTransaction,
    AssetScheme
} from "codechain-sdk/lib/core/classes";
import {
    faucetAddress,
    faucetSecret,
    invalidAddress
} from "../helper/constants";

import CodeChain from "../helper/spawn";

import "mocha";
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;

describe("chain", () => {
    const invalidHash = new H256("0".repeat(64));

    let node: CodeChain;
    before(async () => {
        node = new CodeChain();
        await node.start();
    });

    it("getNetworkId", async () => {
        expect(await node.sdk.rpc.chain.getNetworkId()).to.equal("tc");
    });

    it("getBestBlockNumber", async () => {
        expect(await node.sdk.rpc.chain.getBestBlockNumber()).to.be.a("number");
    });

    it("getBestBlockId", async () => {
        const value = await node.sdk.rpc.sendRpcRequest(
            "chain_getBestBlockId",
            []
        );
        expect(value.hash).to.be.a("string");
        new H256(value.hash);
        expect(value.number).to.be.a("number");
    });

    it("getBlockHash", async () => {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(await node.sdk.rpc.chain.getBlockHash(bestBlockNumber)).not.to.be
            .null;
        expect(await node.sdk.rpc.chain.getBlockHash(bestBlockNumber + 1)).to.be
            .null;
    });

    it("getBlockByHash", async () => {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const blockHash = await node.sdk.rpc.chain.getBlockHash(
            bestBlockNumber
        );
        expect(
            (await node.sdk.rpc.chain.getBlock(blockHash!))!.number
        ).to.equal(bestBlockNumber);
        expect(await node.sdk.rpc.chain.getBlock(invalidHash)).to.be.null;
    });

    it("getSeq", async () => {
        await node.sdk.rpc.chain.getSeq(faucetAddress);
        expect(await node.sdk.rpc.chain.getSeq(invalidAddress)).to.equal(0);
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        await node.sdk.rpc.chain.getSeq(faucetAddress, 0);
        await node.sdk.rpc.chain.getSeq(faucetAddress, bestBlockNumber);
        await expect(
            node.sdk.rpc.chain.getSeq(faucetAddress, bestBlockNumber + 1)
        ).to.be.rejectedWith("chain_getSeq returns undefined");
    });

    it("getBalance", async () => {
        await node.sdk.rpc.chain.getBalance(faucetAddress);
        expect(
            await node.sdk.rpc.chain.getBalance(invalidAddress)
        ).to.deep.equal(new U64(0));
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        await node.sdk.rpc.chain.getBalance(faucetAddress, 0);
        await node.sdk.rpc.chain.getBalance(faucetAddress, bestBlockNumber);
        await node.sdk.rpc.chain.getBalance(faucetAddress, bestBlockNumber + 1);
    });

    it("getGenesisAccounts", async () => {
        // FIXME: Add an API to SDK
        const accounts = await node.sdk.rpc.sendRpcRequest(
            "chain_getGenesisAccounts",
            []
        );
        const expected = [
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqyca3rwt",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqgfrhflv",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqvxf40sk",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqszkma5z",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5duemmc",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqcuzl32l",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqungah99",
            "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqpqc2ul2h",
            "tccq8vapdlstar6ghmqgczp6j2e83njsqq0tsvaxm9u",
            "tccq9h7vnl68frvqapzv3tujrxtxtwqdnxw6yamrrgd"
        ];
        expect(accounts.length).to.equal(expected.length);
        expect(accounts).to.include.members(expected);
    });

    it("getBlockReward", async () => {
        // FIXME: Add an API to SDK
        const reward = await node.sdk.rpc.sendRpcRequest(
            "engine_getBlockReward",
            [10]
        );
        expect(reward).to.equal(0);
    });

    it("getPendingParcels", async () => {
        const pendingParcels = await node.sdk.rpc.chain.getPendingParcels();
        expect(pendingParcels.length).to.equal(0);
    });

    it("sendSignedParcel, getParcelInvoice, getParcel", async () => {
        const parcel = node.sdk.core.createPaymentParcel({
            recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
            amount: 0
        });
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const parcelHash = await node.sdk.rpc.chain.sendSignedParcel(
            parcel.sign({
                secret: faucetSecret,
                fee: 10,
                seq
            })
        );
        const invoice = await node.sdk.rpc.chain.getParcelInvoice(parcelHash);
        expect(invoice).to.deep.equal({ success: true, error: undefined });
        const signedParcel = await node.sdk.rpc.chain.getParcel(parcelHash);
        if (signedParcel == null) {
            throw Error("Cannot get the parcel");
        }
        expect(signedParcel.unsigned).to.deep.equal(parcel);
    });

    it("getRegularKey, getRegularKeyOwner", async () => {
        const key = node.sdk.util.getPublicFromPrivate(
            node.sdk.util.generatePrivateKey()
        );
        expect(await node.sdk.rpc.chain.getRegularKey(faucetAddress)).to.be
            .null;
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key)).to.be.null;

        const parcel = node.sdk.core
            .createSetRegularKeyParcel({
                key
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });
        await node.sdk.rpc.chain.sendSignedParcel(parcel);

        expect(
            await node.sdk.rpc.chain.getRegularKey(faucetAddress)
        ).to.deep.equal(new H512(key));
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key)).to.deep.equal(
            faucetAddress
        );

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(
            await node.sdk.rpc.chain.getRegularKey(
                faucetAddress,
                bestBlockNumber
            )
        ).to.deep.equal(new H512(key));
        expect(await node.sdk.rpc.chain.getRegularKey(faucetAddress, 0)).to.be
            .null;
        expect(
            await node.sdk.rpc.chain.getRegularKey(
                faucetAddress,
                bestBlockNumber + 1
            )
        ).to.be.null;

        expect(
            await node.sdk.rpc.chain.getRegularKeyOwner(key, bestBlockNumber)
        ).to.deep.equal(faucetAddress);
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key, 0)).to.be.null;
        expect(
            await node.sdk.rpc.chain.getRegularKeyOwner(
                key,
                bestBlockNumber + 1
            )
        ).to.be.null;
    });

    describe("Mint an asset", () => {
        let tx: AssetMintTransaction;
        let txAssetScheme: AssetScheme;

        before(async () => {
            const recipient = await node.createP2PKHAddress();
            tx = node.sdk.core.createAssetMintTransaction({
                scheme: {
                    shardId: 0,
                    metadata: "",
                    amount: "0xa"
                },
                recipient
            });
            txAssetScheme = tx.getAssetScheme();

            const parcel = node.sdk.core
                .createAssetTransactionParcel({
                    transaction: tx
                })
                .sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

            await node.sdk.rpc.chain.sendSignedParcel(parcel);
        });

        it("getTransaction", async () => {
            expect(
                await node.sdk.rpc.chain.getTransaction(tx.hash())
            ).to.deep.equal(tx);
            expect(await node.sdk.rpc.chain.getTransaction(invalidHash)).to.be
                .null;
        });

        it("getTransactionInvoices", async () => {
            const invoices = await node.sdk.rpc.chain.getTransactionInvoices(
                tx.hash()
            );
            expect(invoices!.length).to.equal(1);
            expect(invoices[0].success).to.be.true;
        });

        it("getAsset", async () => {
            expect(await node.sdk.rpc.chain.getAsset(invalidHash, 0)).to.be
                .null;
            expect(await node.sdk.rpc.chain.getAsset(tx.hash(), 1)).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAsset(tx.hash(), 0)
            ).to.deep.equal(tx.getMintedAsset());

            const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            expect(
                await node.sdk.rpc.chain.getAsset(tx.hash(), 0, bestBlockNumber)
            ).to.deep.equal(tx.getMintedAsset());
            expect(await node.sdk.rpc.chain.getAsset(tx.hash(), 0, 0)).to.be
                .null;
            expect(
                await node.sdk.rpc.chain.getAsset(
                    tx.hash(),
                    0,
                    bestBlockNumber + 1
                )
            ).to.be.null;
        });

        it("getAssetSchemeByHash", async () => {
            const invalidShardId = 1;
            const validShardId = 0;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByHash(
                    invalidHash,
                    validShardId
                )
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByHash(
                    tx.hash(),
                    invalidShardId
                )
            ).to.be.null;

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByHash(
                tx.hash(),
                validShardId
            );
            if (assetScheme == null) {
                throw Error("Cannot get asset scheme");
            }
            expect(assetScheme.amount).to.deep.equal(txAssetScheme.amount);
            expect(assetScheme.metadata).to.equal(txAssetScheme.metadata);
            expect(assetScheme.registrar).to.deep.equal(
                txAssetScheme.registrar
            );
        });

        it("getAssetSchemeByType", async () => {
            expect(await node.sdk.rpc.chain.getAssetSchemeByType(invalidHash))
                .to.be.null;

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByType(
                tx.getAssetSchemeAddress()
            );
            if (assetScheme == null) {
                throw Error("Cannot get asset scheme");
            }
            expect(assetScheme.amount).to.deep.equal(txAssetScheme.amount);
            expect(assetScheme.metadata).to.equal(txAssetScheme.metadata);
            expect(assetScheme.registrar).to.deep.equal(
                txAssetScheme.registrar
            );
        });
    });

    it("isAssetSpent", async () => {
        const { asset } = await node.mintAsset({ amount: 10 });
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0
            )
        ).to.be.false;

        const recipient = await node.createP2PKHAddress();
        const tx = node.sdk.core.createAssetTransferTransaction();
        tx.addInputs(asset);
        tx.addOutputs({
            assetType: asset.assetType,
            recipient,
            amount: "0xa"
        });
        await node.signTransferInput(tx, 0);
        const invoices = await node.sendTransaction(tx);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0].success).to.be.true;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0
            )
        ).to.be.true;

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0,
                bestBlockNumber
            )
        ).to.be.true;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0,
                bestBlockNumber - 1
            )
        ).to.be.false;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.transactionHash,
                asset.outPoint.index,
                0,
                0
            )
        ).to.be.null;
    });

    // Not implemented
    it("executeTransactions");
    it("getNumberOfShards");
    it("getShardRoot");

    after(async () => {
        await node.clean();
    });
});
