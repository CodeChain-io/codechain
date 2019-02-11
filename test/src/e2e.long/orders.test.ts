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
import { $anything } from "../helper/chai-similar";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;
import {
    Asset,
    AssetTransferAddress,
    H160,
    U64
} from "codechain-sdk/lib/core/classes";
import * as _ from "lodash";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { ERROR } from "../helper/error";
import CodeChain from "../helper/spawn";

describe("orders", function() {
    const BASE = 750;
    let node: CodeChain;

    before(async function() {
        node = new CodeChain({ base: BASE });
        await node.start();
    });

    describe("AssetTransfer with orders", function() {
        describe("Mint one asset", function() {
            let aliceAddress: AssetTransferAddress;

            let gold: Asset;

            beforeEach(async function() {
                aliceAddress = await node.createP2PKHAddress();
                gold = (await node.mintAsset({
                    supply: 10000,
                    recipient: aliceAddress
                })).asset;
            });

            it("Wrong order - originOutputs are wrong (asset type from/to is same)", async function() {
                const splitTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(2, () => ({
                            recipient: aliceAddress,
                            quantity: 5000,
                            assetType: gold.assetType,
                            shardId: 0
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendAssetTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0]).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map((g: Asset) =>
                    g.createTransferInput()
                );

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: H160.zero(), // Fake asset type
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 5000,
                    assetQuantityTo: 5000,
                    expiration,
                    originOutputs: [splitGoldInputs[0].prevOut],
                    recipientFrom: aliceAddress
                });

                (order.assetTypeTo as any) = gold.assetType;

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(splitGoldInputs)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 5000,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 5000,
                            assetType: gold.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 5000,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });
                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.INVALID_ORDER_ASSET_TYPES);
                }
            });
        });

        describe("Mint two assets", function() {
            let aliceAddress: AssetTransferAddress;
            let bobAddress: AssetTransferAddress;

            let gold: Asset;
            let silver: Asset;

            beforeEach(async function() {
                aliceAddress = await node.createP2PKHAddress();
                bobAddress = await node.createP2PKHAddress();
                gold = (await node.mintAsset({
                    supply: 10000,
                    recipient: aliceAddress
                })).asset;
                silver = (await node.mintAsset({
                    supply: 10000,
                    recipient: bobAddress
                })).asset;
            });

            it("Correct order, correct transfer", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });

            it("Correct order, correct transfer - Many originOutputs", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: gold.assetType,
                            shardId: 0
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendAssetTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0]).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(g =>
                    g.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs.map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(splitGoldInputs)
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: _.range(10),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range((transferTx as any)._transaction.inputs.length).map(
                        i => node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            }).timeout(10_000);

            it("Correct order, correct transfer - Output(to) is empty", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 10000,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 10000,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 10000,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });

            it("Correct two orders, correct transfer - Ratio is same", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const aliceOrder = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const bobOrder = node.sdk.core.createOrder({
                    assetTypeFrom: silver.assetType,
                    assetTypeTo: gold.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 1000,
                    assetQuantityTo: 100,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: bobAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order: aliceOrder,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    })
                    .addOrder({
                        order: bobOrder,
                        spentQuantity: 1000,
                        inputIndices: [1],
                        outputIndices: [2, 3]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });

            it("Correct two orders, correct transfer - Ratio is different", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const aliceOrder = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const bobOrder = node.sdk.core.createOrder({
                    assetTypeFrom: silver.assetType,
                    assetTypeTo: gold.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 1000,
                    assetQuantityTo: 50,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: bobAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        // Bob gets more gold than he wanted.
                        // If there's a relayer, relayer may take it.
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order: aliceOrder,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    })
                    .addOrder({
                        order: bobOrder,
                        spentQuantity: 1000,
                        inputIndices: [1],
                        outputIndices: [2, 3]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            });

            it("Correct order, wrong transfer - Output(from) is empty", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 10000,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 10000,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 10000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 0,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                    );
                }
            });

            it("Correct order, wrong transfer - Ratio is wrong", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000 - 10,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000 + 10,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                    );
                }
            });

            it("Correct order, wrong transfer - Lock script hash of maker is wrong", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });

                (transferTx.orders()[0].order
                    .lockScriptHashFrom as any) = new H160(
                    "0000000000000000000000000000000000000000"
                );

                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INVALID_ORDER_LOCK_SCRIPT_HASH
                    );
                }
            });

            it("Correct order, wrong transfer - Parameters of maker are wrong", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });

                (transferTx.orders()[0].order.parametersFrom as any) = [];

                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.INVALID_ORDER_PARAMETERS);
                }
            });

            it("Correct order, wrong transfer - Too many outputs (from)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900 - 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                    );
                }
            });

            it("Correct order, wrong transfer - Too many outputs (to)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000 - 100,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 100,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                    );
                }
            });

            it("Correct order, wrong transfer - Too many outputs (both)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900 - 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000 - 100,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 100,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2, 3]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                    );
                }
            });

            it("Wrong order - originOutputs are wrong (empty)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.originOutputs as any) = [];

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.INVALID_ORIGIN_OUTPUTS);
                }
            });

            it("Wrong order - originOutputs are wrong (prevOut does not match)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.INVALID_ORIGIN_OUTPUTS);
                }
            });

            it("Wrong order - originOutputs are wrong (few outputs)", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: gold.assetType,
                            shardId: 0
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendAssetTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0]).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(g =>
                    g.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs
                        .slice(0, 9)
                        .map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(splitGoldInputs)
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: _.range(10),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range((transferTx as any)._transaction.inputs.length).map(
                        i => node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.false;
            }).timeout(10_000);

            it("Wrong order - originOutputs are wrong (many outputs)", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: gold.assetType,
                            shardId: 0
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendAssetTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0]).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(g =>
                    g.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs.map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(splitGoldInputs.slice(0, 9))
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 8900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: _.range(9),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range((transferTx as any)._transaction.inputs.length).map(
                        i => node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.false;
            }).timeout(10_000);

            it("Wrong order - Ratio is wrong (from is zero)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.assetQuantityFrom as any) = new U64(0);

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INVALID_ORDER_ASSET_QUANTITIES
                    );
                }
            });

            it("Wrong order - Ratio is wrong (to is zero)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.assetQuantityTo as any) = new U64(0);

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(
                        ERROR.INVALID_ORDER_ASSET_QUANTITIES
                    );
                }
            });

            it("Wrong order - Expiration is old", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = 0;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 1000,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 100,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const signed = transferTx.sign({
                    secret: faucetSecret,
                    fee: 10,
                    seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                });

                try {
                    await node.sdk.rpc.chain.sendSignedTransaction(signed);
                    expect.fail();
                } catch (e) {
                    expect(e).is.similarTo(ERROR.ORDER_EXPIRED);
                }
            });

            it("Successful partial fills", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx1 = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9950,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 500,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 50,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9500,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx1, 0);
                await node.signTransactionInput(transferTx1, 1);

                const invoices1 = await node.sendAssetTransaction(transferTx1);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0]).to.be.true;

                const orderConsumed = order.consume(50);
                const transferTx2 = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(
                        transferTx1.getTransferredAsset(0),
                        transferTx1.getTransferredAsset(3)
                    )
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9900,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 500,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 50,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9000,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order: orderConsumed,
                        spentQuantity: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                // Sign on input 0 is not needed
                await node.signTransactionInput(transferTx2, 1);

                const invoices2 = await node.sendAssetTransaction(transferTx2);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0]).to.be.true;
            }).timeout(10_000);

            it("Successful partial cancel", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    shardIdFrom: 0,
                    shardIdTo: 0,
                    assetQuantityFrom: 100,
                    assetQuantityTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx1 = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9950,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: aliceAddress,
                            quantity: 500,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 50,
                            assetType: gold.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9500,
                            assetType: silver.assetType,
                            shardId: 0
                        }
                    )
                    .addOrder({
                        order,
                        spentQuantity: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx1, 0);
                await node.signTransactionInput(transferTx1, 1);

                const invoices1 = await node.sendAssetTransaction(transferTx1);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0]).to.be.true;

                const transferTx2 = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(
                        transferTx1.getTransferredAsset(0),
                        transferTx1.getTransferredAsset(3)
                    )
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            quantity: 9500,
                            assetType: silver.assetType,
                            shardId: 0
                        },
                        {
                            recipient: bobAddress,
                            quantity: 9950,
                            assetType: gold.assetType,
                            shardId: 0
                        }
                    );
                await node.signTransactionInput(transferTx2, 0);
                await node.signTransactionInput(transferTx2, 1);

                const invoices2 = await node.sendAssetTransaction(transferTx2);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0]).to.be.true;
            });
        }).timeout(10_000);

        describe("Mint five assets", function() {
            let addresses: AssetTransferAddress[];
            let assets: Asset[];

            beforeEach(async function() {
                addresses = [];
                assets = [];
                for (let i = 0; i < 5; i++) {
                    const address = await node.createP2PKHAddress();
                    const { asset } = await node.mintAsset({
                        supply: 10000,
                        recipient: address
                    });
                    addresses.push(address);
                    assets.push(asset);
                }
            });

            it("Multiple orders", async function() {
                const inputs = assets.map(asset => asset.createTransferInput());

                const transferTx = node.sdk.core
                    .createTransferAssetTransaction()
                    .addInputs(inputs)
                    .addOutputs([
                        ..._.range(5).map(i => ({
                            recipient: addresses[i],
                            quantity: 50,
                            assetType: assets[(i + 1) % 5].assetType,
                            shardId: 0
                        })),
                        ..._.range(5).map(i => ({
                            recipient: addresses[i],
                            quantity: 9950,
                            assetType: assets[i].assetType,
                            shardId: 0
                        }))
                    ]);

                for (let i = 0; i < 5; i++) {
                    const order = node.sdk.core.createOrder({
                        assetTypeFrom: assets[i].assetType,
                        assetTypeTo: assets[(i + 1) % 5].assetType,
                        shardIdFrom: 0,
                        shardIdTo: 0,
                        assetQuantityFrom: 100,
                        assetQuantityTo: 100,
                        expiration: U64.MAX_VALUE,
                        originOutputs: [inputs[i].prevOut],
                        recipientFrom: addresses[i]
                    });
                    transferTx.addOrder({
                        order,
                        spentQuantity: 50,
                        inputIndices: [i],
                        outputIndices: [i, i + 5]
                    });
                }

                await Promise.all(
                    _.range((transferTx as any)._transaction.inputs.length).map(
                        i => node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendAssetTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0]).to.be.true;
            }).timeout(10_000);
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
