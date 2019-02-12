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
import * as chaiAsPromised from "chai-as-promised";
chai.use(chaiAsPromised);
import { H160, PlatformAddress } from "codechain-primitives";
import "mocha";
import {
    faucetAccointId,
    faucetAddress,
    faucetSecret
} from "../helper/constants";
import CodeChain from "../helper/spawn";

const expect = chai.expect;

describe("WrapCCC", function() {
    let node: CodeChain;
    before(async function() {
        node = new CodeChain();
        await node.start();
    });

    it("WCCC can be burnt", async function() {
        const shardId = 0;
        const wrapCCC = node.sdk.core.createWrapCCCTransaction({
            shardId,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 30,
            payer: PlatformAddress.fromAccountId(faucetAccointId, {
                networkId: "tc"
            })
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
        expect(invoice1).to.be.true;

        const schemeAfterWrap = (await node.sdk.rpc.chain.getAssetSchemeByType(
            H160.zero(),
            shardId
        ))!;
        expect(schemeAfterWrap.supply.isEqualTo(30)).to.be.true;

        const blockNumberBeforeBurn = await node.sdk.rpc.chain.getBestBlockNumber();

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
        expect(invoice2).to.be.true;

        const schemeAfterBurn = (await node.sdk.rpc.chain.getAssetSchemeByType(
            H160.zero(),
            shardId
        ))!;
        expect(schemeAfterBurn.supply.isEqualTo(0)).to.be.true;

        const schemeBeforeBurn = (await node.sdk.rpc.chain.getAssetSchemeByType(
            H160.zero(),
            shardId,
            blockNumberBeforeBurn
        ))!;
        expect(schemeBeforeBurn.supply.isEqualTo(30)).to.be.true;
    }).timeout(30_000);

    it("Changing asset scheme of WCCC causes syntax error", async function() {
        const wrapCCC = node.sdk.core.createWrapCCCTransaction({
            shardId: 0,
            recipient: await node.createP2PKHBurnAddress(),
            quantity: 30,
            payer: PlatformAddress.fromAccountId(faucetAccointId, {
                networkId: "tc"
            })
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
        expect(invoice1).to.be.true;

        const changeAssetScheme = node.sdk.core.createChangeAssetSchemeTransaction(
            {
                shardId: 0,
                assetType: H160.zero(),
                scheme: {
                    metadata: "WCCC",
                    allowedScriptHashes: []
                },
                approvals: []
            }
        );
        const signedChangeAssetScheme = changeAssetScheme.sign({
            secret: faucetSecret,
            seq: seq + 1,
            fee: 10
        });
        await expect(
            node.sdk.rpc.chain.sendSignedTransaction(signedChangeAssetScheme)
        ).to.be.rejected;
    }).timeout(30_000);

    after(async function() {
        await node.clean();
    });
});
