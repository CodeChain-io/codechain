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

import { expect } from "chai";
import { PlatformAddress } from "codechain-primitives";
import "mocha";
import { faucetAddress, faucetSecret } from "../helper/constants";
import CodeChain from "../helper/spawn";

describe("WrapCCC", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("WCCC can be burnt", async function() {
        const wrapCCC = node.sdk.core.createWrapCCCTransaction({
            shardId: 0,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 30
        });
        const seq = (await node.sdk.rpc.chain.getSeq(faucetAddress))!;
        expect(seq).not.to.be.null;
        const signedWrapCCC = wrapCCC.sign({
            secret: faucetSecret,
            seq,
            fee: 10
        });

        await node.sdk.rpc.chain.sendSignedTransaction(signedWrapCCC);
        const invoice1 = (await node.sdk.rpc.chain.getInvoice(
            signedWrapCCC.hash(),
            {
                timeout: 30_000
            }
        ))!;
        expect(invoice1).not.to.be.null;
        expect(invoice1.success).be.equal(true);

        const WCCC = wrapCCC.getAsset();

        const burn = node.sdk.core
            .createTransferAssetTransaction()
            .addBurns(WCCC);
        await node.signTransactionP2PKHBurn(
            burn.burn(0)!,
            burn.hashWithoutScript()
        );
        const signedBurn = burn.sign({
            secret: faucetSecret,
            seq: seq + 1,
            fee: 10
        });
        await node.sdk.rpc.chain.sendSignedTransaction(signedBurn);
        const invoice2 = (await node.sdk.rpc.chain.getInvoice(
            signedBurn.hash(),
            {
                timeout: 30_000
            }
        ))!;
        expect(invoice2).not.to.be.null;
        expect(invoice2.success).be.equal(true);
    }).timeout(30_000);

    after(async function() {
        await node.clean();
    });
});
