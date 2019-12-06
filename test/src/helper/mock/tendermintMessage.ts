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
// along with this program. If not, see <https://www.gnu.org/licenses/>.
import { H256, U64 } from "codechain-primitives";
import { EventEmitter } from "events";
import { compressSync, uncompressSync } from "snappy";

import { readOptionalRlp, readUIntRLP } from "../rlp";

const RLP = require("rlp");

export const Emitter = new EventEmitter();

enum MessageType {
    MESSAGE_ID_CONSENSUS_MESSAGE = 0x01,
    MESSAGE_ID_PROPOSAL_BLOCK = 0x02,
    MESSAGE_ID_STEP_STATE = 0x03,
    MESSAGE_ID_REQUEST_MESSAGE = 0x04,
    MESSAGE_ID_REQUEST_PROPOSAL = 0x05
}

export enum Step {
    Propose = 0,
    Prevote = 1,
    Precommit = 2,
    Commit = 3
}

interface VoteStep {
    height: number;
    view: number;
    step: Step;
}

interface SortitionRound {
    height: number;
    view: number;
}

interface PriorityInfo {
    signerIdx: number;
    priority: H256;
    subUserIdx: number;
    numberOfElections: number;
    vrfProof: Buffer;
}

export interface ConsensusMessage {
    type: "consensusmessage";
    messages: Array<{
        on: {
            step: VoteStep;
            blockHash: H256 | null;
        };
        signature: string;
        signerIndex: number;
    }>;
}

export interface ProposalBlock {
    type: "proposalblock";
    signature: string;
    priorityInfo: PriorityInfo;
    view: number;
    message: Buffer;
}

interface ProposalSummary {
    priorityInfo: PriorityInfo;
    blockHash: H256;
}

export interface StepState {
    type: "stepstate";
    voteStep: VoteStep;
    proposal: ProposalSummary | null;
    lockView: number | null;
    knownVotes: Buffer;
}

export interface RequestMessage {
    type: "requestmessage";
    voteStep: VoteStep;
    requestedVotes: Buffer;
}

export interface RequestProposal {
    type: "requestproposal";
    round: SortitionRound;
}

type MessageBody =
    | ConsensusMessage
    | ProposalBlock
    | StepState
    | RequestMessage
    | RequestProposal;

export class TendermintMessage {
    public static fromBytes(bytes: Buffer): TendermintMessage {
        const decoded = RLP.decode(bytes);
        const id = readUIntRLP(decoded[0]);
        let message: MessageBody;
        switch (id) {
            case MessageType.MESSAGE_ID_CONSENSUS_MESSAGE: {
                message = {
                    type: "consensusmessage",
                    messages: decoded[1].map((d: any) => {
                        const inner = RLP.decode(d);
                        return {
                            on: {
                                step: {
                                    height: readUIntRLP(inner[0][0][0]),
                                    view: readUIntRLP(inner[0][0][1]),
                                    step: readUIntRLP(inner[0][0][2]) as Step
                                },
                                blockHash: readOptionalRlp(
                                    inner[0][1],
                                    buffer => new H256(buffer.toString("hex"))
                                )
                            },
                            signature: inner[1].toString("hex"),
                            signerIndex: readUIntRLP(inner[2])
                        };
                    })
                };
                break;
            }
            case MessageType.MESSAGE_ID_PROPOSAL_BLOCK: {
                message = {
                    type: "proposalblock",
                    signature: decoded[1].toString("hex"),
                    priorityInfo: {
                        signerIdx: readUIntRLP(decoded[2][0]),
                        priority: new H256(decoded[2][1].toString("hex")),
                        subUserIdx: readUIntRLP(decoded[2][2]),
                        numberOfElections: readUIntRLP(decoded[2][3]),
                        vrfProof: decoded[2][4]
                    },
                    view: readUIntRLP(decoded[3]),
                    message: uncompressSync(decoded[4])
                };
                break;
            }
            case MessageType.MESSAGE_ID_STEP_STATE: {
                message = {
                    type: "stepstate",
                    voteStep: {
                        height: readUIntRLP(decoded[1][0]),
                        view: readUIntRLP(decoded[1][1]),
                        step: readUIntRLP(decoded[1][2]) as Step
                    },
                    proposal: readOptionalRlp(decoded[2], (buffer: any) => {
                        return {
                            priorityInfo: {
                                signerIdx: readUIntRLP(buffer[0][0]),
                                priority: new H256(
                                    buffer[0][1].toString("hex")
                                ),
                                subUserIdx: readUIntRLP(buffer[0][2]),
                                numberOfElections: readUIntRLP(buffer[0][3]),
                                vrfProof: buffer[0][4]
                            },
                            blockHash: new H256(buffer[1].toString("hex"))
                        };
                    }),
                    lockView: readOptionalRlp(decoded[3], readUIntRLP),
                    knownVotes: decoded[4]
                };
                break;
            }
            case MessageType.MESSAGE_ID_REQUEST_MESSAGE: {
                message = {
                    type: "requestmessage",
                    voteStep: {
                        height: readUIntRLP(decoded[1][0]),
                        view: readUIntRLP(decoded[1][1]),
                        step: readUIntRLP(decoded[1][2]) as Step
                    },
                    requestedVotes: decoded[2]
                };
                break;
            }
            case MessageType.MESSAGE_ID_REQUEST_PROPOSAL: {
                message = {
                    type: "requestproposal",
                    round: {
                        height: readUIntRLP(decoded[1][0]),
                        view: readUIntRLP(decoded[1][1])
                    }
                };
                break;
            }
            default: {
                throw new Error(`Unexpected message id ${id}`);
            }
        }
        Emitter.emit(message.type, message);
        return new TendermintMessage(message);
    }
    private body: MessageBody;

    constructor(body: MessageBody) {
        this.body = body;
    }

    public getBody(): MessageBody {
        return this.body;
    }

    public toEncodeObject() {
        switch (this.body.type) {
            case "consensusmessage": {
                return [
                    MessageType.MESSAGE_ID_CONSENSUS_MESSAGE,
                    this.body.messages.map(m =>
                        RLP.encode([
                            [
                                [
                                    new U64(m.on.step.height).toEncodeObject(),
                                    new U64(m.on.step.view).toEncodeObject(),
                                    new U64(m.on.step.step).toEncodeObject()
                                ],
                                m.on.blockHash == null
                                    ? []
                                    : [m.on.blockHash.toEncodeObject()]
                            ],
                            Buffer.from(m.signature, "hex"),
                            new U64(m.signerIndex).toEncodeObject()
                        ])
                    )
                ];
            }
            case "proposalblock": {
                return [
                    MessageType.MESSAGE_ID_PROPOSAL_BLOCK,
                    Buffer.from(this.body.signature, "hex"),
                    [
                        new U64(
                            this.body.priorityInfo.signerIdx
                        ).toEncodeObject(),
                        this.body.priorityInfo.priority.toEncodeObject(),
                        new U64(
                            this.body.priorityInfo.subUserIdx
                        ).toEncodeObject(),
                        new U64(
                            this.body.priorityInfo.numberOfElections
                        ).toEncodeObject(),
                        this.body.priorityInfo.vrfProof
                    ],
                    new U64(this.body.view).toEncodeObject(),
                    compressSync(this.body.message)
                ];
            }
            case "stepstate": {
                return [
                    MessageType.MESSAGE_ID_STEP_STATE,
                    [
                        new U64(this.body.voteStep.height).toEncodeObject(),
                        new U64(this.body.voteStep.view).toEncodeObject(),
                        new U64(this.body.voteStep.step).toEncodeObject()
                    ],
                    this.body.proposal == null
                        ? []
                        : [
                              [
                                  new U64(
                                      this.body.proposal.priorityInfo.signerIdx
                                  ).toEncodeObject(),
                                  this.body.proposal.priorityInfo.priority.toEncodeObject(),
                                  new U64(
                                      this.body.proposal.priorityInfo.subUserIdx
                                  ).toEncodeObject(),
                                  new U64(
                                      this.body.proposal.priorityInfo.numberOfElections
                                  ).toEncodeObject(),
                                  this.body.proposal.priorityInfo.vrfProof
                              ],
                              this.body.proposal.blockHash.toEncodeObject()
                          ],
                    this.body.lockView == null
                        ? []
                        : [new U64(this.body.lockView).toEncodeObject()],
                    this.body.knownVotes
                ];
            }
            case "requestmessage": {
                return [
                    MessageType.MESSAGE_ID_REQUEST_MESSAGE,
                    [
                        new U64(this.body.voteStep.height).toEncodeObject(),
                        new U64(this.body.voteStep.view).toEncodeObject(),
                        new U64(this.body.voteStep.step).toEncodeObject()
                    ],
                    this.body.requestedVotes
                ];
            }
            case "requestproposal": {
                return [
                    MessageType.MESSAGE_ID_REQUEST_PROPOSAL,
                    [
                        new U64(this.body.round.height).toEncodeObject(),
                        new U64(this.body.round.view).toEncodeObject()
                    ]
                ];
            }
        }
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}
