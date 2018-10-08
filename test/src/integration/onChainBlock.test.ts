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
import { Header } from "codechain-test-helper/lib/cHeader";
import CodeChain from "../helper/spawn";
import { H256, U256, H160 } from "codechain-primitives/lib";

describe("Test onChain block communication", async () => {
    let nodeA: CodeChain;
    let TH: TestHelper;
    let soloGenesisBlock: Header;
    let soloBlock1: Header;
    let soloBlock2: Header;

    let INVALID_TIMESTAMP: U256;

    beforeAll(async () => {
        const node = new CodeChain({
            logFlag: true,
            argv: ["--force-sealing"]
        });
        await node.start();

        const sdk = node.sdk;

        await sdk.rpc.devel.startSealing();
        await sdk.rpc.devel.startSealing();

        const genesisBlock = await sdk.rpc.chain.getBlock(0);
        const block1 = await sdk.rpc.chain.getBlock(1);
        const block2 = await sdk.rpc.chain.getBlock(2);

        await node.clean();
        soloGenesisBlock = new Header(
            genesisBlock.parentHash,
            new U256(genesisBlock.timestamp),
            new U256(genesisBlock.number),
            genesisBlock.author.accountId,
            Buffer.from(genesisBlock.extraData),
            genesisBlock.parcelsRoot,
            genesisBlock.stateRoot,
            genesisBlock.invoicesRoot,
            genesisBlock.score,
            genesisBlock.seal
        );
        soloBlock1 = new Header(
            soloGenesisBlock.hashing(),
            new U256(block1.timestamp),
            new U256(block1.number),
            block1.author.accountId,
            Buffer.from(block1.extraData),
            block1.parcelsRoot,
            block1.stateRoot,
            block1.invoicesRoot,
            new U256(2222222222222),
            block1.seal
        );
        soloBlock2 = new Header(
            soloBlock1.hashing(),
            new U256(block2.timestamp),
            new U256(block2.number),
            block2.author.accountId,
            Buffer.from(block2.extraData),
            block2.parcelsRoot,
            block2.stateRoot,
            block2.invoicesRoot,
            new U256(33333333333333),
            block2.seal
        );

        INVALID_TIMESTAMP = new U256(block1.timestamp - 1);
    });

    beforeEach(async () => {
        nodeA = new CodeChain({ logFlag: true });
        await nodeA.start();
        TH = new TestHelper("0.0.0.0", nodeA.port);
        await TH.establish();
    });

    afterEach(async () => {
        await TH.end();
        await nodeA.clean();
    });

    test(
        "OnChain invalid timestamp block propagation test",
        async () => {
            const sdk = nodeA.sdk;

            // Genesis block
            const header = soloGenesisBlock;

            // Block 1
            const header1 = soloBlock1;

            // Block 2
            const header2 = soloBlock2;

            const bestHash = header2.hashing();
            const bestScore = header2.getScore();

            header2.setTimestamp(INVALID_TIMESTAMP);

            await TH.sendEncodedBlock(
                [
                    header.toEncodeObject(),
                    header1.toEncodeObject(),
                    header2.toEncodeObject()
                ],
                [[], []],
                bestHash,
                bestScore
            );

            await TH.waitStatusMessage();

            const block1 = await sdk.rpc.chain.getBlock(1);
            const block2 = await sdk.rpc.chain.getBlock(2);

            expect(block1).toEqual(expect.anything());
            expect(block2).toEqual(null);
        },
        30000
    );
});
