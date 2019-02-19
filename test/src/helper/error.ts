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

import * as chai from "chai";
import { $anything, $containsWord, similar } from "./chai-similar";

chai.use(similar);

export const ERROR = {
    NOT_ENOUGH_BALANCE: {
        code: -32032,
        data: $anything,
        message: $anything
    },
    KEY_ERROR: {
        code: -32041,
        data: $anything,
        message: $anything
    },
    ALREADY_EXISTS: {
        code: -32042,
        data: $anything,
        message: $anything
    },
    WRONG_PASSWORD: {
        code: -32043,
        data: $anything,
        message: $anything
    },
    NO_SUCH_ACCOUNT: {
        code: -32044,
        data: $anything,
        message: $anything
    },
    NOT_UNLOCKED: {
        code: -32045,
        data: $anything,
        message: $anything
    },
    INVALID_PARAMS: {
        code: -32602,
        message: $anything
    },
    INVALID_RLP_TOO_BIG: {
        code: -32009,
        data: $containsWord("RlpIsTooBig"),
        message: $anything
    },
    INVALID_RLP_TOO_SHORT: {
        code: -32009,
        data: $containsWord("RlpIsTooShort"),
        message: $anything
    },
    INVALID_RLP_INVALID_LENGTH: {
        code: -32009,
        data: $containsWord("RlpInvalidLength"),
        message: $anything
    },
    INVALID_RLP_UNEXPECTED_ACTION_PREFIX: {
        code: -32009,
        data: $containsWord("Unexpected action prefix"),
        message: $anything
    },
    INVALID_RLP_INCORRECT_LIST_LEN: {
        code: -32009,
        data: $containsWord("RlpIncorrectListLen"),
        message: $anything
    },
    TOO_LOW_FEE: {
        code: -32033,
        data: $anything,
        message: $anything
    },
    INVALID_NETWORK_ID: {
        code: -32036,
        data: $anything,
        message: $anything
    },
    INVALID_TX_ZERO_QUANTITY: {
        code: -32099,
        data: $containsWord("Syntax(ZeroQuantity"),
        message: $anything
    },
    INVALID_TX_INCONSISTENT_IN_OUT: {
        code: -32099,
        data: $containsWord("Syntax(InconsistentTransactionInOut"),
        message: $anything
    },
    INVALID_TX_ASSET_TYPE: {
        code: -32099,
        data: $containsWord("Syntax(InvalidAssetType"),
        message: $anything
    },
    INVALID_TX_DUPLICATED_PREV_OUT: {
        code: -32099,
        data: $containsWord("Syntax(DuplicatedPreviousOutput"),
        message: $anything
    },
    ORDER_IS_NOT_EMPTY: {
        code: -32099,
        message: $anything,
        data: $containsWord("OrderIsNotEmpty")
    },
    INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS: {
        code: -32099,
        message: $anything,
        data: $containsWord("InconsistentTransactionInOutWithOrders")
    },
    INVALID_ORIGIN_OUTPUTS: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOriginOutputs")
    },
    INVALID_ORDER_ASSET_QUANTITIES: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOrderAssetQuantities")
    },
    INVALID_ORDER_ASSET_TYPES: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOrderAssetTypes")
    },
    INVALID_ORDER_LOCK_SCRIPT_HASH: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOrderLockScriptHash")
    },
    INVALID_ORDER_PARAMETERS: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOrderParameters")
    },
    INVALID_OUTPUT_WITH_ORDER: {
        code: -32099,
        message: $anything,
        data: $containsWord("InvalidOutputWithOrder")
    },
    ORDER_EXPIRED: {
        code: -32099,
        message: $anything,
        data: $containsWord("OrderExpired")
    },
    STATE_NOT_EXIST: {
        code: -32048,
        message: $anything
    },
    ACTION_DATA_HANDLER_NOT_FOUND: {
        code: -32049,
        message: $anything
    }
};
