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

describe("Test onChain parcel communication", () => {
    let nodeA: CodeChain;

    const VALID_FEE = 10;
    const INVALID_FEE = 16069380442589902755419620923411626025222029937827928353013799;
    const VALID_NONCE = 0;
    const INVALID_NONCE = 1;
    const VALID_NETWORKID = "tc";
    const INVALID_NETWORKID = "a";
    const VALID_SIG =
        "0x6dbde483ac39847466ad85919e9c09df0c1f8d7f71628c1664f1d7ffc494385857b778a51d9c049fd4609f2aed6b7f28e1fdcc0e4ef30e41393b38b12f8cd2e101";
    const INVALID_SIG = "0x1221fzcv441";
    const testArray = [
        [
            "OnChain invalid fee PaymentParcel propagation test",
            INVALID_FEE,
            VALID_NONCE,
            VALID_NETWORKID,
            VALID_SIG
        ],
        [
            "OnChain invalid nonce PaymentParcel propagation test",
            VALID_FEE,
            INVALID_NONCE,
            VALID_NETWORKID,
            VALID_SIG
        ],
        [
            "OnChain invalid networkId PaymentParcel propagation test",
            VALID_FEE,
            VALID_NONCE,
            INVALID_NETWORKID,
            VALID_SIG
        ],
        [
            "OnChain invalid signature PaymentParcel propagation test",
            VALID_FEE,
            VALID_NONCE,
            VALID_NETWORKID,
            INVALID_SIG
        ]
    ];

    beforeEach(async () => {
        nodeA = new CodeChain({ logFlag: true });
        await nodeA.start();
    });

    afterEach(async () => {
        await nodeA.clean();
    });

    test(
        "OnChain PaymentParcel propagation test",
        async () => {
            const TH = new TestHelper("0.0.0.0", nodeA.port);
            await TH.establish();

            const sdk = nodeA.sdk;

            const ACCOUNT_SECRET =
                process.env.ACCOUNT_SECRET ||
                "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
            const parcel = sdk.core.createPaymentParcel({
                recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
                amount: 10000
            });
            const signedparcel = parcel.sign({
                secret: ACCOUNT_SECRET,
                fee: 10,
                nonce: 0
            });
            await sdk.rpc.devel.stopSealing();
            await TH.sendEncodedParcel([signedparcel.toEncodeObject()]);

            const parcels = await sdk.rpc.chain.getPendingParcels();
            expect(parcels.length).toEqual(1);

            await TH.end();
        },
        20000
    );

    describe("OnChain invalid PaymentParcel test", async () => {
        test.each(testArray)(
            "%s",
            async (_testName, tfee, tnonce, tnetworkId, tsig) => {
                const TH = new TestHelper("0.0.0.0", nodeA.port);
                await TH.establish();

                const sdk = nodeA.sdk;

                const ACCOUNT_SECRET =
                    process.env.ACCOUNT_SECRET ||
                    "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
                const parcel = sdk.core.createPaymentParcel({
                    recipient: "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
                    amount: 10000
                });
                const signedparcel = parcel.sign({
                    secret: ACCOUNT_SECRET,
                    fee: tfee,
                    nonce: tnonce
                });
                await sdk.rpc.devel.stopSealing();

                const data = signedparcel.toEncodeObject();
                data[2] = tnetworkId;
                data[4] = tsig;

                await TH.sendEncodedParcel([data]);
                const parcels = await sdk.rpc.chain.getPendingParcels();
                expect(parcels.length).toEqual(0);

                await TH.end();
            },
            20000
        );
    });
});
