import { expect } from "chai";
import { U256 } from "codechain-primitives";
import "mocha";
import * as BlockSyncMessage from "../blockSyncMessage";

describe("Check BlockSyncMessage RLP encoding", function() {
    it("RequestBodyMessage RLP encoding test", function() {
        const message = new BlockSyncMessage.RequestMessage({
            type: "bodies",
            data: []
        });
        const msg = new BlockSyncMessage.BlockSyncMessage({
            type: "request",
            id: new U256(10),
            message
        });
        expect(msg.rlpBytes().toString("hex")).deep.equal("c3040ac0");
    });

    it("ResponseBodyMessage RLP encoding test", function() {
        const message = new BlockSyncMessage.ResponseMessage({
            type: "bodies",
            data: [[]]
        });
        const msg = new BlockSyncMessage.BlockSyncMessage({
            type: "response",
            id: new U256(10),
            message
        });
        expect(msg.rlpBytes().toString("hex")).deep.equal("c8050ac5840204c1c0");
    });
});
