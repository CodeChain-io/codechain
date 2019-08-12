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
import "mocha";
import { validators as originalDynValidators } from "../../../tendermint.dynval/constants";
import { PromiseExpect } from "../../helper/promise";
import { withNodes } from "../setup";

chai.use(chaiAsPromised);

// Verifying external blocks take different code path with internal blocks.
// We need both a proposer and a verifier to test them.
const [alice, bob] = originalDynValidators;

describe("one block one term test", function() {
    const promiseExpect = new PromiseExpect();
    const margin = 1.2;

    const { nodes, initialParams } = withNodes(this, {
        promiseExpect,
        overrideParams: {
            termSeconds: 1
        },
        validators: [
            { signer: alice, delegation: 5000, deposit: 100000 },
            { signer: bob }
        ]
    });

    it("Alice should success creating terms", async function() {
        const aliceNode = nodes[0];
        this.slow(initialParams.termSeconds * 2 * margin * 1000 + 5_000);
        this.timeout(initialParams.termSeconds * 3 * 1000 + 10_000);
        await aliceNode.waitForTermChange(
            3,
            initialParams.termSeconds * 2 * margin + 5_000
        );
    });

    afterEach(async function() {
        await promiseExpect.checkFulfilled();
    });
});
