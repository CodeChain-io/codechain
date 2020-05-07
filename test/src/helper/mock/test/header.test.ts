import { Buffer } from "buffer";
import { expect } from "chai";
import "mocha";
import * as RLP from "rlp";
import {
    getPublicFromPrivate,
    H256,
    U256,
    H160,
    blake256,
    blake160,
    signEcdsa,
    signSchnorr,
    getAccountIdFromPublic
} from "codechain-primitives";
import { Header } from "../cHeader";

describe("Check Header RLP encoding", function() {
    it("empty Header RLP encoding test", function() {
        const header = Header.default();
        // Find the empty header's rlp encoded data in the unit test in header.rs file
        expect(header.rlpBytes().toString("hex")).deep.equal(
            "f87ca00000000000000000000000000000000000000000000000000000000000000000940000000000000000000000000000000000000000a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c080808080"
        );
    });

    it("Header RLP encoding test", function() {
        const privateKey =
            "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
        const publicKey = getPublicFromPrivate(privateKey);
        const header = Header.default();
        header.setNumber(new U256(4));
        header.setAuthor(new H160(getAccountIdFromPublic(publicKey)));
        const bitset = Buffer.alloc(100, 0);
        bitset[0] = 4;
        const signature = createPrecommit({
            height: 3,
            view: 0,
            step: 2,
            parentHash: header.getParentHash()!,
            privateKey
        });
        header.setSeal([0, 0, [Buffer.from(signature, "hex")], bitset]);
        // Find the header's rlp encoded data in the unit test in the tendermint/mod.rs file
        expect(header.rlpBytes().toString("hex")).deep.equal(
            "f90128a00000000000000000000000000000000000000000000000000000000000000000946fe64ffa3a46c074226457c90ccb32dc06ccced1a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0a045b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0800480808080f842b8405aa028f952416218d440568ddf58b42a02515d53dbe40aec6d8fbc3a0b9de171bd1fa833c2bf9e7b950414ca4bbc261c662d50372340c0f7b41ab0a12d11a789b86404000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );
    });

    function createPrecommit({
        height,
        view,
        step,
        parentHash,
        privateKey
    }: {
        height: number;
        view: number;
        step: number;
        parentHash: H256;
        privateKey: string;
    }): string {
        const voteOn = [[height, view, step], [parentHash.toEncodeObject()]];
        const serializedVoteOn = RLP.encode(voteOn);
        const message = blake256(serializedVoteOn);
        const { r, s } = signSchnorr(message, privateKey);
        return r + s;
    }
});
