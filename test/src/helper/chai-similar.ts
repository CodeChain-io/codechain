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

import { U64 } from "codechain-primitives";
import * as _ from "lodash";

export const $else = Symbol("else");

interface Predicate {
    (_: any): boolean;
    toString(): string;
}

export function $anything() {
    return true;
}
$anything.toString = () => "$anything";

export function $anyOf(values: any[]): Predicate {
    function pred(actual: any): boolean {
        for (const expected of values) {
            if (deepCompare(actual, expected)) {
                return true;
            }
        }
        return false;
    }
    pred.toString = function toString(): string {
        return "$anyOf(" + JSON.stringify(normalize(values)) + ")";
    };
    return pred;
}

export function $allOf(values: any[]): Predicate {
    function pred(actual: any): boolean {
        for (const expected of values) {
            if (!deepCompare(actual, expected)) {
                return false;
            }
        }
        return true;
    }
    pred.toString = function toString(): string {
        return "$allOf(" + JSON.stringify(normalize(values)) + ")";
    };
    return pred;
}

export function $containsWord(word: string): Predicate {
    const escape = /([()])/g;
    let wordPattern: string = word;
    if (escape.test(word)) {
        wordPattern = word.replace(escape, "\\$1");
    }
    const regex = new RegExp("\\b" + wordPattern + "\\b");
    function pred(actual: any): boolean {
        return regex.test(actual);
    }
    pred.toString = function toString(): string {
        return "$containsWord(" + word + ")";
    };
    return pred;
}

declare global {
    namespace Chai {
        type Similar = (value: any, message?: string) => Assertion;

        interface Assertion {
            similarTo: Similar;
        }
    }
}

export function similar(chai: any, utils: any): void {
    function assertSimilar(this: any, expected: any, msg: boolean) {
        if (msg) {
            utils.flag(this, "message", msg);
        }
        const actual = utils.flag(this, "object");
        const result = deepCompare(actual, expected);

        this.assert(
            result,
            "not similar",
            "similar",
            normalize(expected),
            normalize(actual),
            true
        );
    }

    utils.addMethod(chai.Assertion.prototype, "similarTo", assertSimilar);
}

function deepCompare(actual: any, expected: any): boolean {
    if (expected === null) {
        return actual === null;
    }
    if (expected === undefined) {
        return actual === undefined;
    }
    if (typeof expected === "function") {
        return expected(actual);
    }
    if (typeof expected !== "object") {
        return expected === actual;
    }

    if (Array.isArray(expected)) {
        if (Array.isArray(actual)) {
            return arraysEqual(actual, expected);
        }
        if (Buffer.isBuffer(actual)) {
            return arraysEqual([...actual], expected);
        }
        throw new Error("Not implemented");
    }
    if (expected instanceof U64) {
        return expected.isEqualTo(actual);
    }
    if (expected instanceof RegExp) {
        return expected.test(actual);
    }
    // object
    return objectEqual(actual, expected);
}

function arraysEqual(a: Array<any>, b: Array<any>): boolean {
    if (a === b) {
        return true;
    }
    if (a == null || b == null) {
        return false;
    }
    if (a.length !== b.length) {
        return false;
    }

    for (let i = 0; i < a.length; ++i) {
        if (!deepCompare(a[i], b[i])) {
            return false;
        }
    }
    return true;
}

function objectEqual(
    actual: { [name: string]: any },
    expected: { [name: string]: any }
): boolean {
    /// actual >= expected
    for (const key of Object.keys(expected)) {
        if (actual[key] === undefined) {
            console.log("actual < expected");
            return false;
        }
    }

    /// actual <= expected
    if (expected[$else as any] === undefined) {
        for (const key of Object.keys(actual)) {
            if (expected[key] === undefined) {
                console.log("actual > expected");
                return false;
            }
        }
    }

    for (const key of Object.keys(expected)) {
        if (!deepCompare(actual[key], expected[key])) {
            return false;
        }
    }

    return true;
}

function normalize(value: any): any {
    if (value === null || value === undefined) {
        return value;
    } else if (typeof value === "function") {
        return value.toString();
    } else if (Array.isArray(value)) {
        const result: any[] = [];
        for (const x of value) {
            result.push(normalize(x));
        }
        return result;
    } else if (typeof value === "object") {
        const result: { [key: string]: any } = {};
        for (const key of Object.keys(value)) {
            result[key] = normalize(value[key]);
        }
        if (value[$else as any] !== undefined) {
            // stringify symbol
            result.$else = normalize(value[$else as any]);
        }
        return result;
    }
    return value;
}
