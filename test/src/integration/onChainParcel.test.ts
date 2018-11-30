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

import { TestHelper } from "codechain-test-helper/lib/testHelper";
import CodeChain from "../helper/spawn";
import { faucetSecret } from "../helper/constants";

import "mocha";
import { expect } from "chai";

describe("Test onChain parcel communication", function() {
    let nodeA: CodeChain;

    const VALID_FEE = 10;
    const INVALID_FEE = 1606202993013799;
    const VALID_SEQ = 0;
    const INVALID_SEQ = 1;
    const VALID_NETWORKID = "tc";
    const INVALID_NETWORKID = "a";
    const VALID_SIG =
        "0x6dbde483ac39847466ad85919e9c09df0c1f8d7f71628c1664f1d7ffc494385857b778a51d9c049fd4609f2aed6b7f28e1fdcc0e4ef30e41393b38b12f8cd2e101";
    const INVALID_SIG = "0x1221fzcv441";
    const testArray = [
        {
            testName: "OnChain invalid fee PaymentParcel propagation test",
            tfee: INVALID_FEE,
            tseq: VALID_SEQ,
            tnetworkId: VALID_NETWORKID,
            tsig: VALID_SIG
        },
        {
            testName: "OnChain invalid seq PaymentParcel propagation test",
            tfee: VALID_FEE,
            tseq: INVALID_SEQ,
            tnetworkId: VALID_NETWORKID,
            tsig: VALID_SIG
        },
        {
            testName:
                "OnChain invalid networkId PaymentParcel propagation test",
            tfee: VALID_FEE,
            tseq: VALID_SEQ,
            tnetworkId: INVALID_NETWORKID,
            tsig: VALID_SIG
        },
        {
            testName:
                "OnChain invalid signature PaymentParcel propagation test",
            tfee: VALID_FEE,
            tseq: VALID_SEQ,
            tnetworkId: VALID_NETWORKID,
            tsig: INVALID_SIG
        }
    ];

    beforeEach(async function() {
        nodeA = new CodeChain();
        await nodeA.start();
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodeA.testFailed(this.currentTest!.fullTitle());
        }
        await nodeA.clean();
    });

    it("OnChain PaymentParcel propagation test", async function() {
        const TH = new TestHelper("0.0.0.0", nodeA.port);
        await TH.establish();

        const sdk = nodeA.sdk;

        const ACCOUNT_SECRET = process.env.ACCOUNT_SECRET || faucetSecret;
        const parcel = sdk.core.createPaymentParcel({
            recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
            amount: 10000
        });
        const signedparcel = parcel.sign({
            secret: ACCOUNT_SECRET,
            fee: 10,
            seq: 0
        });
        await sdk.rpc.devel.stopSealing();
        await TH.sendEncodedParcel([signedparcel.toEncodeObject()]);

        const parcels = await sdk.rpc.chain.getPendingParcels();
        expect(parcels.length).to.equal(1);

        await TH.end();
    }).timeout(20_000);

    describe("OnChain invalid PaymentParcel test", async function() {
        testArray.forEach(function(params: {
            testName: string;
            tfee: number;
            tseq: number;
            tnetworkId: string;
            tsig: string;
        }) {
            const { testName, tfee, tseq, tnetworkId, tsig } = params;
            it(testName, async function() {
                const TH = new TestHelper("0.0.0.0", nodeA.port);
                await TH.establish();

                const sdk = nodeA.sdk;

                const ACCOUNT_SECRET =
                    process.env.ACCOUNT_SECRET || faucetSecret;
                const parcel = sdk.core.createPaymentParcel({
                    recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
                    amount: 10000
                });
                const signedparcel = parcel.sign({
                    secret: ACCOUNT_SECRET,
                    fee: tfee,
                    seq: tseq
                });
                await sdk.rpc.devel.stopSealing();

                const data = signedparcel.toEncodeObject();
                data[2] = tnetworkId;
                data[4] = tsig;

                await TH.sendEncodedParcel([data]);
                const parcels = await sdk.rpc.chain.getPendingParcels();
                expect(parcels.length).to.equal(0);

                await TH.end();
            }).timeout(20_000);
        });
    });
});
