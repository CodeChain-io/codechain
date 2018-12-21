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

import * as _ from "lodash";
import {
    Asset,
    H256,
    AssetTransferAddress,
    AssetOutPoint,
    Order,
    U64,
    H160
} from "codechain-sdk/lib/core/classes";

import CodeChain from "../helper/spawn";
import { ERROR, errorMatcher } from "../helper/error";
import { faucetAddress, faucetSecret } from "../helper/constants";

import "mocha";
import * as chai from "chai";
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
const expect = chai.expect;

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
                    amount: 10000,
                    recipient: aliceAddress
                })).asset;
            });

            it("Wrong order - originOutputs are wrong (asset type from/to is same)", async function() {
                const splitTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(2, () => ({
                            recipient: aliceAddress,
                            amount: 5000,
                            assetType: gold.assetType
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0].success).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(gold =>
                    gold.createTransferInput()
                );

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: new H256(
                        "0000000000000000000000000000000000000000000000000000000000000000"
                    ), // Fake asset type
                    assetAmountFrom: 5000,
                    assetAmountTo: 5000,
                    expiration,
                    originOutputs: [splitGoldInputs[0].prevOut],
                    recipientFrom: aliceAddress
                });

                (order.assetTypeTo as any) = gold.assetType;

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(splitGoldInputs)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 5000,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 5000,
                            assetType: gold.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 5000,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });
                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORDER_ASSET_TYPES)
                    );
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
                    amount: 10000,
                    recipient: aliceAddress
                })).asset;
                silver = (await node.mintAsset({
                    amount: 10000,
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });

            it("Correct order, correct transfer - Many originOutputs", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: gold.assetType
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0].success).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(gold =>
                    gold.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs.map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(splitGoldInputs)
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: _.range(10),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range(transferTx.inputs.length).map(i =>
                        node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            }).timeout(10_000);

            it("Correct order, correct transfer - Output(to) is empty", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 10000,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 10000,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 10000,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });

            it("Correct two orders, correct transfer - Ratio is same", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const aliceOrder = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const bobOrder = node.sdk.core.createOrder({
                    assetTypeFrom: silver.assetType,
                    assetTypeTo: gold.assetType,
                    assetAmountFrom: 1000,
                    assetAmountTo: 100,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: bobAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order: aliceOrder,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    })
                    .addOrder({
                        order: bobOrder,
                        spentAmount: 1000,
                        inputIndices: [1],
                        outputIndices: [2, 3]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });

            it("Correct two orders, correct transfer - Ratio is different", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const aliceOrder = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const bobOrder = node.sdk.core.createOrder({
                    assetTypeFrom: silver.assetType,
                    assetTypeTo: gold.assetType,
                    assetAmountFrom: 1000,
                    assetAmountTo: 50,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: bobAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 50,
                            assetType: gold.assetType
                        },
                        // Bob gets more gold than he wanted.
                        // If there's a relayer, relayer may take it.
                        {
                            recipient: bobAddress,
                            amount: 50,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order: aliceOrder,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    })
                    .addOrder({
                        order: bobOrder,
                        spentAmount: 1000,
                        inputIndices: [1],
                        outputIndices: [2, 4]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
            });

            it("Correct order, wrong transfer - Output(from) is empty", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 10000,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 10000,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 10000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 0,
                        inputIndices: [0],
                        outputIndices: [0]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(
                            ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                        )
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000 - 10,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000 + 10,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(
                            ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                        )
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });

                (transferTx.orders[0].order
                    .lockScriptHashFrom as any) = new H160(
                    "0000000000000000000000000000000000000000"
                );

                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORDER_LOCK_SCRIPT_HASH)
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });

                (transferTx.orders[0].order.parametersFrom as any) = [];

                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORDER_PARAMETERS)
                    );
                }
            });

            it("Correct order, wrong transfer - Too many outputs (from)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900 - 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(
                            ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                        )
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000 - 100,
                            assetType: silver.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 100,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(
                            ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                        )
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900 - 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000 - 100,
                            assetType: silver.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 100,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1, 2, 3]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(
                            ERROR.INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS
                        )
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.originOutputs as any) = [];

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORIGIN_OUTPUTS)
                    );
                }
            });

            it("Wrong order - originOutputs are wrong (prevOut does not match)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [silverInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORIGIN_OUTPUTS)
                    );
                }
            });

            it("Wrong order - originOutputs are wrong (few outputs)", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: gold.assetType
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0].success).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(gold =>
                    gold.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs
                        .slice(0, 9)
                        .map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(splitGoldInputs)
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: _.range(10),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range(transferTx.inputs.length).map(i =>
                        node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.false;
            }).timeout(10_000);

            it("Wrong order - originOutputs are wrong (many outputs)", async function() {
                // Split minted gold asset to 10 assets
                const splitTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(gold)
                    .addOutputs(
                        _.times(10, () => ({
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: gold.assetType
                        }))
                    );
                await node.signTransactionInput(splitTx, 0);

                const splitInvoices = await node.sendTransaction(splitTx);
                expect(splitInvoices!.length).to.equal(1);
                expect(splitInvoices![0].success).to.be.true;

                const splitGolds = splitTx.getTransferredAssets();
                const splitGoldInputs = splitGolds.map(gold =>
                    gold.createTransferInput()
                );
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: splitGoldInputs.map(input => input.prevOut),
                    recipientFrom: aliceAddress
                });

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(splitGoldInputs.slice(0, 9))
                    .addInputs(silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 8900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: _.range(9),
                        outputIndices: [0, 1]
                    });
                await Promise.all(
                    _.range(transferTx.inputs.length).map(i =>
                        node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.false;
            }).timeout(10_000);

            it("Wrong order - Ratio is wrong (from is zero)", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.assetAmountFrom as any) = new U64(0);

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORDER_ASSET_AMOUNTS)
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                (order.assetAmountTo as any) = new U64(0);

                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(
                        errorMatcher(ERROR.INVALID_ORDER_ASSET_AMOUNTS)
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
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 1000,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 100,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 100,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx, 0);
                await node.signTransactionInput(transferTx, 1);

                const parcel = node.sdk.core
                    .createAssetTransactionParcel({
                        transaction: transferTx
                    })
                    .sign({
                        secret: faucetSecret,
                        fee: 10,
                        seq: await node.sdk.rpc.chain.getSeq(faucetAddress)
                    });

                try {
                    await node.sdk.rpc.chain.sendSignedParcel(parcel);
                    expect.fail();
                } catch (e) {
                    expect(e).to.satisfy(errorMatcher(ERROR.ORDER_EXPIRED));
                }
            });

            it("Successful partial fills", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });

                const transferTx1 = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9950,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 500,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 50,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9500,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx1, 0);
                await node.signTransactionInput(transferTx1, 1);

                const invoices1 = await node.sendTransaction(transferTx1);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0].success).to.be.true;

                const orderConsumed = order.consume(50);
                const transferTx2 = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(
                        transferTx1.getTransferredAsset(0),
                        transferTx1.getTransferredAsset(3)
                    )
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9900,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 500,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 50,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9000,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order: orderConsumed,
                        spentAmount: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                // Sign on input 0 is not needed
                await node.signTransactionInput(transferTx2, 1);

                const invoices2 = await node.sendTransaction(transferTx2);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0].success).to.be.true;
            }).timeout(10_000);

            it("Successful partial cancel", async function() {
                const goldInput = gold.createTransferInput();
                const silverInput = silver.createTransferInput();

                const expiration = Math.round(Date.now() / 1000) + 120;
                const order = node.sdk.core.createOrder({
                    assetTypeFrom: gold.assetType,
                    assetTypeTo: silver.assetType,
                    assetAmountFrom: 100,
                    assetAmountTo: 1000,
                    expiration,
                    originOutputs: [goldInput.prevOut],
                    recipientFrom: aliceAddress
                });
                const transferTx1 = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(goldInput, silverInput)
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9950,
                            assetType: gold.assetType
                        },
                        {
                            recipient: aliceAddress,
                            amount: 500,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 50,
                            assetType: gold.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9500,
                            assetType: silver.assetType
                        }
                    )
                    .addOrder({
                        order,
                        spentAmount: 50,
                        inputIndices: [0],
                        outputIndices: [0, 1]
                    });
                await node.signTransactionInput(transferTx1, 0);
                await node.signTransactionInput(transferTx1, 1);

                const invoices1 = await node.sendTransaction(transferTx1);
                expect(invoices1!.length).to.equal(1);
                expect(invoices1![0].success).to.be.true;

                const transferTx2 = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(
                        transferTx1.getTransferredAsset(0),
                        transferTx1.getTransferredAsset(3)
                    )
                    .addOutputs(
                        {
                            recipient: aliceAddress,
                            amount: 9500,
                            assetType: silver.assetType
                        },
                        {
                            recipient: bobAddress,
                            amount: 9950,
                            assetType: gold.assetType
                        }
                    );
                await node.signTransactionInput(transferTx2, 0);
                await node.signTransactionInput(transferTx2, 1);

                const invoices2 = await node.sendTransaction(transferTx2);
                expect(invoices2!.length).to.equal(1);
                expect(invoices2![0].success).to.be.true;
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
                        amount: 10000,
                        recipient: address
                    });
                    addresses.push(address);
                    assets.push(asset);
                }
            });

            it("Multiple orders", async function() {
                const inputs = assets.map(asset => asset.createTransferInput());

                let transferTx = node.sdk.core
                    .createAssetTransferTransaction()
                    .addInputs(inputs)
                    .addOutputs([
                        ..._.range(5).map(i => ({
                            recipient: addresses[i],
                            amount: 50,
                            assetType: assets[(i + 1) % 5].assetType
                        })),
                        ..._.range(5).map(i => ({
                            recipient: addresses[i],
                            amount: 9950,
                            assetType: assets[i].assetType
                        }))
                    ]);

                for (let i = 0; i < 5; i++) {
                    const order = node.sdk.core.createOrder({
                        assetTypeFrom: assets[i].assetType,
                        assetTypeTo: assets[(i + 1) % 5].assetType,
                        assetAmountFrom: 100,
                        assetAmountTo: 100,
                        expiration: U64.MAX_VALUE,
                        originOutputs: [inputs[i].prevOut],
                        recipientFrom: addresses[i]
                    });
                    transferTx.addOrder({
                        order,
                        spentAmount: 50,
                        inputIndices: [i],
                        outputIndices: [i, i + 5]
                    });
                }

                await Promise.all(
                    _.range(transferTx.inputs.length).map(i =>
                        node.signTransactionInput(transferTx, i)
                    )
                );

                const invoices = await node.sendTransaction(transferTx);
                expect(invoices!.length).to.equal(1);
                expect(invoices![0].success).to.be.true;
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
