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

import "mocha";
import * as chai from "chai";
import { similar, $else, $anyOf, $anything } from "./chai-similar";

chai.use(similar);

describe("SimilarTo", function() {
    it("does not throws on same structure", function() {
        function test() {
            chai.expect({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value"
                    }
                }
            }).similarTo({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value"
                    }
                }
            });
        }
        chai.expect(test).does.not.throws(chai.AssertionError);
    });

    it("throws AssertionError on nested value is different", function() {
        function test() {
            chai.expect({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value",
                        "I'm Different!": "YAY"
                    }
                }
            }).similarTo({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value"
                    }
                }
            });
        }
        chai.expect(test).does.throws(chai.AssertionError);
    });

    it("does not throws on all matcher returns true", function() {
        function test() {
            chai.expect({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value yay",
                        blah: [1, 2, 3],
                        "I'm Different!": "YAY",
                        more: "Difference!"
                    }
                }
            }).similarTo({
                nested: {
                    object: {
                        with: [
                            { array: $anyOf([1, 2, 3, 4]) },
                            { of: $anything }
                        ],
                        other: /^val/,
                        blah: (x: any[]) => x.length == 3,
                        [$else]: $anything
                    }
                }
            });
        }
        chai.expect(test).does.not.throws(chai.AssertionError);
    });

    it("throws AssertionError on some matcher returns false", function() {
        function test() {
            chai.expect({
                nested: {
                    object: {
                        with: [{ array: 1 }, { of: ["object"] }],
                        other: "value yay",
                        "I'mDifferent!": "YAY"
                    }
                }
            }).similarTo({
                nested: {
                    object: {
                        with: [{ array: $anyOf([2, 3, 4]) }, { of: $anything }],
                        [$else]: $anything
                    }
                }
            });
        }
        chai.expect(test).does.throws(chai.AssertionError);
    });
});
