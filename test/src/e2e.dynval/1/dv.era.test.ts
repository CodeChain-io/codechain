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
import { expect } from "chai";
import * as chaiAsPromised from "chai-as-promised";
import "mocha";

import { validators } from "../../../tendermint.dynval/constants";
import { PromiseExpect } from "../../helper/promise";
import { changeParams, setTermTestTimeout, withNodes } from "../setup";

chai.use(chaiAsPromised);

describe("Change era", function() {
    const promiseExpect = new PromiseExpect();
    const { nodes, initialParams } = withNodes(this, {
        promiseExpect,
        overrideParams: {
            minNumOfValidators: 3,
            maxNumOfValidators: 5
        },
        validators: validators.slice(0, 3).map(signer => ({
            signer,
            delegation: 5_000,
            deposit: 10_000_000
        }))
    });

    it("should be enabled", async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 3
        });

        const checkingNode = nodes[0];
        const changeTxHash = await changeParams(checkingNode, 1, {
            ...initialParams,
            era: 1
        });

        await checkingNode.waitForTx(changeTxHash);

        await termWaiter.waitNodeUntilTerm(checkingNode, {
            target: 3,
            termPeriods: 2
        });
    });

    it("must increase monotonically", async function() {
        const termWaiter = setTermTestTimeout(this, {
            terms: 2
        });

        const checkingNode = nodes[0];
        const changeTxHash = await changeParams(checkingNode, 1, {
            ...initialParams,
            era: 1
        });

        await checkingNode.waitForTx(changeTxHash);

        await expect(
            changeParams(checkingNode, 2, {
                ...initialParams,
                era: 0
            })
        ).eventually.rejected;
    });

    afterEach(function() {
        promiseExpect.checkFulfilled();
    });
});
