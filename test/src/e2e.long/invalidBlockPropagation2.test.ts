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

import { expect } from "chai";
import { U256 } from "codechain-primitives/lib";
import "mocha";
import { createTestSuite } from "./invalidBlockPropagation.helper";

const INVALID_NUMBER = new U256(2);
const params = {
    tnumber: INVALID_NUMBER
};
createTestSuite(2, "OnChain invalid number block propagation test", params);
