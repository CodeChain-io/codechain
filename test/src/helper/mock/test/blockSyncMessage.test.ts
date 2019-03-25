import * as BlockSyncMessage from "../blockSyncMessage";
import { expect } from "chai";
import { U256 } from "codechain-primitives";
import "mocha";

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
        expect([...msg.rlpBytes()]).deep.equal([195, 4, 10, 192]);
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
        expect([...msg.rlpBytes()]).deep.equal([196, 5, 10, 193, 192]);
    });
});
