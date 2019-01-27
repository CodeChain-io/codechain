// Copyright 2018-2019 Kodebox, Inc.
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

import { H256 } from "codechain-primitives/lib";

import { expect } from "chai";
import "mocha";
import { createTestSuite } from "./invalidBlockPropagation.helper";

const INVALID_TRANSACTIONS_ROOT = new H256(
    "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
);
const params = {
    ttransactionRoot: INVALID_TRANSACTIONS_ROOT
};
createTestSuite(
    5,
    "OnChain invalid transactionRoot block propagation test",
    params
);
