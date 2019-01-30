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
import {
    AssetScheme,
    H160,
    H256,
    H512,
    MintAsset,
    U64
} from "codechain-sdk/lib/core/classes";
import "mocha";
import {
    faucetAddress,
    faucetSecret,
    invalidAddress
} from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("chain", function() {
    const invalidH160 = H160.zero();
    const invalidH256 = H256.zero();

    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("getNetworkId", async function() {
        expect(await node.sdk.rpc.chain.getNetworkId()).to.equal("tc");
    });

    it("getBestBlockNumber", async function() {
        expect(await node.sdk.rpc.chain.getBestBlockNumber()).to.be.a("number");
    });

    it("getBestBlockId", async function() {
        const value = await node.sdk.rpc.sendRpcRequest(
            "chain_getBestBlockId",
            []
        );
        expect(value.hash).to.be.a("string");
        new H256(value.hash);
        expect(value.number).to.be.a("number");
    });

    it("getBlockHash", async function() {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(await node.sdk.rpc.chain.getBlockHash(bestBlockNumber)).not.to.be
            .null;
        expect(await node.sdk.rpc.chain.getBlockHash(bestBlockNumber + 1)).to.be
            .null;
    });

    it("getBlockByHash", async function() {
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        const blockHash = await node.sdk.rpc.chain.getBlockHash(
            bestBlockNumber
        );
        expect(
            (await node.sdk.rpc.chain.getBlock(blockHash!))!.number
        ).to.equal(bestBlockNumber);
        expect(await node.sdk.rpc.chain.getBlock(invalidH256)).to.be.null;
    });

    it("getSeq", async function() {
        await node.sdk.rpc.chain.getSeq(faucetAddress);
        expect(await node.sdk.rpc.chain.getSeq(invalidAddress)).to.equal(0);
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        await node.sdk.rpc.chain.getSeq(faucetAddress, 0);
        await node.sdk.rpc.chain.getSeq(faucetAddress, bestBlockNumber);
        await expect(
            node.sdk.rpc.chain.getSeq(faucetAddress, bestBlockNumber + 1)
        ).to.be.rejectedWith("chain_getSeq returns undefined");
    });

    it("getBalance", async function() {
        await node.sdk.rpc.chain.getBalance(faucetAddress);
        expect(
            await node.sdk.rpc.chain.getBalance(invalidAddress)
        ).to.deep.equal(new U64(0));
        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        await node.sdk.rpc.chain.getBalance(faucetAddress, 0);
        await node.sdk.rpc.chain.getBalance(faucetAddress, bestBlockNumber);
        await node.sdk.rpc.chain.getBalance(faucetAddress, bestBlockNumber + 1);
    });

    it("getGenesisAccounts", async function() {
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

    it("getBlockReward", async function() {
        // FIXME: Add an API to SDK
        const reward = await node.sdk.rpc.sendRpcRequest(
            "engine_getBlockReward",
            [10]
        );
        expect(reward).to.equal(0);
    });

    it("getPendingTransactions", async function() {
        const pending = await node.sdk.rpc.chain.getPendingTransactions();
        expect(pending.length).to.equal(0);
    });

    it("sendPayTx, getInvoice, getTransaction", async function() {
        const tx = node.sdk.core.createPayTransaction({
            recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
            quantity: 0
        });
        const seq = await node.sdk.rpc.chain.getSeq(faucetAddress);
        const hash = await node.sdk.rpc.chain.sendSignedTransaction(
            tx.sign({
                secret: faucetSecret,
                fee: 10,
                seq
            })
        );
        const invoice = (await node.sdk.rpc.chain.getInvoice(hash))!;
        expect(invoice.error).to.be.undefined;
        expect(invoice.success).to.be.true;
        const signed = await node.sdk.rpc.chain.getTransaction(hash);
        if (signed == null) {
            throw Error("Cannot get the transaction");
        }
        expect(signed.unsigned).to.deep.equal(tx);
    });

    it("getRegularKey, getRegularKeyOwner", async function() {
        const key = node.sdk.util.getPublicFromPrivate(
            node.sdk.util.generatePrivateKey()
        );
        expect(await node.sdk.rpc.chain.getRegularKey(faucetAddress)).to.be
            .null;
        expect(await node.sdk.rpc.chain.getRegularKeyOwner(key)).to.be.null;

        const tx = node.sdk.core
            .createSetRegularKeyTransaction({
                key
            })
            .sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });
        await node.sdk.rpc.chain.sendSignedTransaction(tx);

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

    describe("Mint an asset", function() {
        let tx: MintAsset;
        let txAssetScheme: AssetScheme;

        before(async function() {
            const recipient = await node.createP2PKHAddress();
            tx = node.sdk.core.createMintAssetTransaction({
                scheme: {
                    shardId: 0,
                    metadata: "",
                    supply: "0xa"
                },
                recipient
            });
            txAssetScheme = tx.getAssetScheme();

            const signed = tx.sign({
                secret: faucetSecret,
                fee: 10,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
            });

            await node.sdk.rpc.chain.sendSignedTransaction(signed);
        });

        it("getTransactionByTracker", async function() {
            expect(
                ((await node.sdk.rpc.chain.getTransactionByTracker(
                    tx.tracker()
                )) as any).unsigned
            ).to.deep.equal(tx);
            expect(
                await node.sdk.rpc.chain.getTransactionByTracker(invalidH256)
            ).to.be.null;
        });

        it("getInvoicesByTracker", async function() {
            const invoices = await node.sdk.rpc.chain.getInvoicesByTracker(
                tx.tracker()
            );
            expect(invoices!.length).to.equal(1);
            expect(invoices[0].success).to.be.true;
        });

        it("getAsset", async function() {
            const invalidShardId = 1;
            const validShardId = 0;
            expect(
                await node.sdk.rpc.chain.getAsset(invalidH256, 0, validShardId)
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAsset(tx.tracker(), 1, validShardId)
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAsset(
                    tx.tracker(),
                    1,
                    invalidShardId
                )
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAsset(tx.tracker(), 0, validShardId)
            ).to.deep.equal(tx.getMintedAsset());

            const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
            expect(
                await node.sdk.rpc.chain.getAsset(
                    tx.tracker(),
                    0,
                    validShardId,
                    bestBlockNumber
                )
            ).to.deep.equal(tx.getMintedAsset());
            expect(
                await node.sdk.rpc.chain.getAsset(
                    tx.tracker(),
                    0,
                    validShardId,
                    0
                )
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAsset(
                    tx.tracker(),
                    0,
                    validShardId,
                    bestBlockNumber + 1
                )
            ).to.be.null;
        });

        it("getAssetSchemeByTracker", async function() {
            const invalidShardId = 1;
            const validShardId = 0;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByTracker(
                    invalidH256,
                    validShardId
                )
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByTracker(
                    tx.tracker(),
                    invalidShardId
                )
            ).to.be.null;

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByTracker(
                tx.tracker(),
                validShardId
            );
            if (assetScheme == null) {
                throw Error("Cannot get asset scheme");
            }
            expect(assetScheme.supply).to.deep.equal(txAssetScheme.supply);
            expect(assetScheme.metadata).to.equal(txAssetScheme.metadata);
            expect(assetScheme.approver).to.deep.equal(txAssetScheme.approver);
        });

        it("getAssetSchemeByType", async function() {
            const invalidShardId = 1;
            const validShardId = 0;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByType(
                    invalidH160,
                    validShardId
                )
            ).to.be.null;
            expect(
                await node.sdk.rpc.chain.getAssetSchemeByType(
                    tx.getAssetType(),
                    invalidShardId
                )
            ).to.be.null;

            const assetScheme = await node.sdk.rpc.chain.getAssetSchemeByType(
                tx.getAssetType(),
                validShardId
            );
            if (assetScheme == null) {
                throw Error("Cannot get asset scheme");
            }
            expect(assetScheme.supply).to.deep.equal(txAssetScheme.supply);
            expect(assetScheme.metadata).to.equal(txAssetScheme.metadata);
            expect(assetScheme.approver).to.deep.equal(txAssetScheme.approver);
        });
    });

    it("isAssetSpent", async function() {
        const { asset } = await node.mintAsset({ supply: 10 });
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.tracker,
                asset.outPoint.index,
                asset.outPoint.shardId
            )
        ).to.be.false;

        const recipient = await node.createP2PKHAddress();
        const tx = node.sdk.core.createTransferAssetTransaction();
        tx.addInputs(asset);
        tx.addOutputs({
            assetType: asset.assetType,
            shardId: asset.shardId,
            recipient,
            quantity: "0xa"
        });
        await node.signTransactionInput(tx, 0);
        const invoices = await node.sendAssetTransaction(tx);
        expect(invoices!.length).to.equal(1);
        expect(invoices![0].success).to.be.true;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.tracker,
                asset.outPoint.index,
                asset.outPoint.shardId
            )
        ).to.be.true;

        const bestBlockNumber = await node.sdk.rpc.chain.getBestBlockNumber();
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.tracker,
                asset.outPoint.index,
                asset.outPoint.shardId,
                bestBlockNumber
            )
        ).to.be.true;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.tracker,
                asset.outPoint.index,
                asset.outPoint.shardId,
                bestBlockNumber - 1
            )
        ).to.be.false;
        expect(
            await node.sdk.rpc.chain.isAssetSpent(
                asset.outPoint.tracker,
                asset.outPoint.index,
                asset.outPoint.shardId,
                0
            )
        ).to.be.null;
    });

    it("executeTransaction", async function() {
        const scheme = node.sdk.core.createAssetScheme({
            shardId: 0,
            metadata: "",
            supply: 10000
        });
        const tx = node.sdk.core.createMintAssetTransaction({
            scheme,
            recipient: await node.createP2PKHAddress()
        });
        tx.setFee(0);

        const data = tx.toJSON();

        await node.sdk.rpc
            .sendRpcRequest("chain_executeTransaction", [
                data,
                faucetAddress.value
            ])
            .then(result => {
                expect(result).to.deep.equal({ success: true });
            });
    });

    it("getNumberOfShards", async function() {
        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getNumberOfShards", [null])
        ).to.equal(1);

        expect(
            await node.sdk.rpc.sendRpcRequest("chain_getNumberOfShards", [0])
        ).to.equal(1);
    });

    it("getShardRoot", async function() {
        await node.sdk.rpc
            .sendRpcRequest("chain_getShardRoot", [0, null])
            .then(result => {
                expect(result).not.to.be.null;
                H256.ensure(result);
            });

        await node.sdk.rpc
            .sendRpcRequest("chain_getShardRoot", [0, 0])
            .then(result => {
                expect(result).not.to.be.null;
                H256.ensure(result);
            });

        await node.sdk.rpc
            .sendRpcRequest("chain_getShardRoot", [10000, null])
            .then(result => {
                expect(result).to.be.null;
            });
    });

    it("getMiningReward", async function() {
        await node.sdk.rpc
            .sendRpcRequest("chain_getMiningReward", [0])
            .then(result => {
                expect(result).to.equal(0);
            });
    });

    afterEach(function() {
        if (this.currentTest!.state === "failed") {
            node.testFailed(this.currentTest!.fullTitle());
        }
    });

    after(async function() {
        await node.clean();
    });
});
