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
import {
    aliceAddress,
    aliceSecret,
    faucetAddress,
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address
} from "../helper/constants";
import { U256 } from "codechain-sdk/lib/core/U256";
import { PlatformAddress } from "codechain-sdk/lib/core/classes";
import { wait } from "../helper/promise";

const describeSkippedInTravis = process.env.TRAVIS ? describe.skip : describe;

describeSkippedInTravis("Tendermint ", () => {
    let nodes: CodeChain[];

    beforeEach(async () => {
        let validatorAddresses = [
            validator0Address,
            validator1Address,
            validator2Address,
            validator3Address
        ];
        nodes = validatorAddresses.map(address => {
            return new CodeChain({
                chain: `${__dirname}/../scheme/tendermint.json`,
                argv: [
                    "--engine-signer",
                    address.toString(),
                    "--password-path",
                    "test/tendermint/password.json",
                    "--force-sealing"
                ],
                additionalKeysPath: "tendermint/keys"
            });
        });
        await Promise.all(nodes.map(node => node.start()));
        nodes[0].connect(nodes[1]);
        nodes[0].connect(nodes[2]);
        nodes[0].connect(nodes[3]);
        nodes[1].connect(nodes[2]);
        nodes[1].connect(nodes[3]);
        nodes[2].connect(nodes[3]);
        await Promise.all([
            nodes[0].waitPeers(4 - 1),
            nodes[1].waitPeers(4 - 1),
            nodes[2].waitPeers(4 - 1),
            nodes[3].waitPeers(4 - 1)
        ]);
    }, 60 * 1000);

    test(
        "Wait block generation",
        async () => {
            await expect(
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            ).resolves.toBeLessThan(1);
            await wait(30 * 1000);
            await expect(
                nodes[0].sdk.rpc.chain.getBestBlockNumber()
            ).resolves.toBeGreaterThanOrEqual(1);
        },
        60 * 1000
    );

    afterEach(async () => {
        await Promise.all(nodes.map(node => node.clean()));
    });
});
