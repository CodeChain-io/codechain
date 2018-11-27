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
import { aliceAddress, aliceSecret, faucetAddress } from "../helper/constants";
import { U64 } from "codechain-sdk/lib/core/classes";

describe("Reward = 50, 1 miner", () => {
    let node: CodeChain;

    beforeEach(async () => {
        node = new CodeChain({
            chain: `${__dirname}/../scheme/solo-block-reward-50.json`,
            argv: ["--author", aliceAddress.toString(), "--force-sealing"]
        });
        await node.start();
    });

    test("Mining an empty block", async () => {
        await node.sdk.rpc.devel.startSealing();
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(50));
    });

    test("Mining a block with 1 parcel", async () => {
        await node.sendSignedParcel({ fee: 10 });
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(50 + 10));
    });

    test("Mining a block with 3 parcels", async () => {
        await node.sdk.rpc.devel.stopSealing();
        await node.sendSignedParcel({
            fee: 10,
            seq: 0,
            awaitInvoice: false
        });
        await node.sendSignedParcel({
            fee: 10,
            seq: 1,
            awaitInvoice: false
        });
        await node.sendSignedParcel({
            fee: 15,
            seq: 2,
            awaitInvoice: false
        });
        await node.sdk.rpc.devel.startSealing();
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(50 + 35));
    });

    test("Mining a block with a parcel that pays the author", async () => {
        await node.payment(aliceAddress, 100);
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(50 + 10 + 100));
    });

    test("Mining a block with a parcel which author pays someone in", async () => {
        await node.sendSignedParcel({ fee: 10 }); // +60
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(60));

        const parcel = await node.sdk.core
            .createPaymentParcel({
                recipient: faucetAddress,
                amount: 50
            })
            .sign({ secret: aliceSecret, seq: 0, fee: 10 }); // -60
        await node.sdk.rpc.chain.sendSignedParcel(parcel); // +60
        await expect(
            node.sdk.rpc.chain.getBalance(aliceAddress)
        ).resolves.toEqual(new U64(60));
    });

    afterEach(async () => {
        await node.clean();
    });
});
