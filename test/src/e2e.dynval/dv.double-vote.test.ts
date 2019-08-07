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
import { SDK } from "codechain-sdk";
import * as stake from "codechain-stakeholder-sdk";
import { Custom } from "codechain-sdk/lib/core/transaction/Custom";
import {
    U64,
    H256,
    signSchnorr,
    recoverSchnorr,
    blake256,
    H512,
    PlatformAddress
} from "codechain-primitives/lib";
import "mocha";
import * as RLP from "rlp";

import { validators as originalValidators } from "../../tendermint.dynval/constants";
import { faucetAddress, faucetSecret } from "../helper/constants";
import { PromiseExpect } from "../helper/promise";
import { Signer } from "../helper/spawn";
import CodeChain from "../helper/spawn";
import { withNodes, setTermTestTimeout, findNode } from "./setup";

chai.use(chaiAsPromised);

const HANDLER_ID = 2;
const REPORT_DOUBLE_VOTE_ACTION_ID = 5;

type MessageData = {
    height: number;
    view: number;
    step: "propose" | "prevote" | "precommit" | "commit";
    blockHash: H256 | null;
    privKey: string;
    signerIdx: number;
};

function encodableMessage(data: MessageData): RLP.Input {
    const { height, view, step, blockHash, privKey, signerIdx } = data;
    const encodableStep = (stepString => {
        switch (stepString) {
            case "propose":
                return 0;
            case "prevote":
                return 1;
            case "precommit":
                return 2;
            case "commit":
                return 3;
        }
    })(step);
    const encodableVoteStep = [
        U64.ensure(height).toEncodeObject(),
        U64.ensure(view).toEncodeObject(),
        encodableStep
    ];
    const encodableBlockHash =
        blockHash === null ? [] : [blockHash.toEncodeObject()];
    const encodableVoteOn = [encodableVoteStep, encodableBlockHash];

    const rlpVoteOn = RLP.encode(encodableVoteOn);
    const messageForSchnorr = blake256(rlpVoteOn);
    console.log(messageForSchnorr);
    const schnorrSignature = signSchnorr(messageForSchnorr, privKey);
    // pad because signSchnorr function does not guarantee the length of r and s to be 64.
    const encodableSchnorrSignature = new H512(
        schnorrSignature.r.padStart(64, "0") +
            schnorrSignature.s.padStart(64, "0")
    ).toEncodeObject();
    console.log(schnorrSignature.r, schnorrSignature.s, encodableSchnorrSignature);
    const recovered = recoverSchnorr(messageForSchnorr, {
        r: encodableSchnorrSignature.slice(2, 66),
        s: encodableSchnorrSignature.slice(66)
    })
    console.log(recovered);

    return [
        encodableVoteOn,
        encodableSchnorrSignature,
        U64.ensure(signerIdx).toEncodeObject()
    ];
}

function createDoubleVoteMessages(
    data: Omit<MessageData, "blockHash">,
    blockHash1: H256 | null,
    blockHash2: H256 | null
) {
    return [
        encodableMessage({ ...data, blockHash: blockHash1 }),
        encodableMessage({ ...data, blockHash: blockHash2 })
    ];
}

function createReportDoubleVoteTransaction(
    sdk: SDK,
    message1: RLP.Input,
    message2: RLP.Input
): Custom {
    return sdk.core.createCustomTransaction({
        handlerId: HANDLER_ID,
        bytes: RLP.encode([REPORT_DOUBLE_VOTE_ACTION_ID, message1, message2])
    });
}

const allDynValidators = originalValidators.slice(0, 4);
const [alice, ...otherDynValidators] = allDynValidators;

async function expectPossibleAuthors(
    sdk: SDK,
    expected: Signer[],
    blockNumber?: number
) {
    const authors = (await stake.getPossibleAuthors(sdk, blockNumber))!.map(
        author => author.toString()
    );
    expect(authors)
        .to.have.lengthOf(expected.length)
        .and.to.include.members(
            expected.map(signer => signer.platformAddress.toString())
        );
}

// FIXME: neeeds to use common refactored function when gets banned state accounts
async function ensureAliceIsBanned(sdk: SDK, blockNumber: number) {
    const bannedAfter = (await stake.getBanned(sdk, blockNumber)).map(
        platformAddr => platformAddr.toString()
    );
    expect(bannedAfter).to.includes(alice.platformAddress.toString());
    const delegteesAfter = (await stake.getDelegations(
        sdk,
        faucetAddress,
        blockNumber
    )).map(delegation => delegation.delegatee.toString());
    expect(delegteesAfter).not.to.includes(alice.platformAddress.toString());
}

describe("Report Double Vote", function() {
    const promiseExpect = new PromiseExpect();

    async function getAliceIndex(
        sdk: SDK,
        blockNumber: number
    ): Promise<number> {
        const a =  (await stake.getPossibleAuthors(sdk, blockNumber))!
            .map(platfromAddr => platfromAddr.toString())
            .indexOf(alice.platformAddress.toString());
        console.log(a);
        return a;
    }

    async function waitUntilAliceGetBanned(
        checkingNode: CodeChain,
        reportTxHash: H256
    ): Promise<number> {
        await checkingNode.waitForTx(reportTxHash);
        const blockNumberAfterReport =
            (await checkingNode.sdk.rpc.chain.getBestBlockNumber()) + 1;
        await checkingNode.waitBlockNumber(blockNumberAfterReport);
        return blockNumberAfterReport;
    }

    describe("Ban from the validator state", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: allDynValidators.map(signer => ({
                signer,
                delegation: 5_000,
                deposit: 10_000_000
            }))
        });

        it("alice should be banned from the validators", async function() {
            const secsPerblock = 3;
            this.slow(secsPerblock * 7 * 1000);
            this.timeout(secsPerblock * 14 * 1000);

            const checkingNode = nodes[1];
            const blockNumber = await checkingNode.sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                checkingNode.sdk,
                blockNumber
            );
            const currentTermInitialBlockNumber =
                termMetadata!.lastTermFinishedBlockNumber + 1;
            expect(termMetadata!.currentTermId).to.be.equals(1);
            await expectPossibleAuthors(checkingNode.sdk, allDynValidators);
            await checkingNode.waitBlockNumber(
                currentTermInitialBlockNumber + 1
            );
            const aliceIdx = await getAliceIndex(
                checkingNode.sdk,
                currentTermInitialBlockNumber
            );

            const [message1, message2] = createDoubleVoteMessages(
                {
                    height: currentTermInitialBlockNumber,
                    view: 0,
                    step: "precommit",
                    privKey: alice.privateKey,
                    signerIdx: aliceIdx
                },
                H256.ensure(
                    "730f75dafd73e047b86acb2dbd74e75dcb93272fa084a9082848f2341aa1abb6"
                ),
                H256.ensure(
                    "07f5937c9760f50867a78fa68982b1096cec0798448abf9395cd778fcded6f8d"
                )
            );

            const reportTx = createReportDoubleVoteTransaction(
                checkingNode.sdk,
                message1,
                message2
            );
            const reportTxHash = await checkingNode.sdk.rpc.chain.sendSignedTransaction(
                reportTx.sign({
                    secret: faucetSecret,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                })
            );
            const blockNumberAfterReport = await waitUntilAliceGetBanned(
                checkingNode,
                reportTxHash
            );
            await ensureAliceIsBanned(checkingNode.sdk, blockNumberAfterReport);
        });
    });

    describe("Ban from the jailed state", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: allDynValidators.map(signer => ({
                signer,
                delegation: 5_000,
                deposit: 10_000_000
            })),
            onBeforeEnable: async nodes => {
                // Kill the alice node first to make alice not to participate in the term 1.
                await findNode(nodes, alice).clean();
            }
        });

        async function ensureAliceIsJailed(sdk: SDK, bestBlockNumber: number) {
            const jailedBefore = (await stake.getJailed(
                sdk,
                bestBlockNumber
            )).map(prisoner => prisoner.address.toString());
            expect(jailedBefore).to.includes(alice.platformAddress.toString());
        }

        async function ensureAliceIsReleased(
            sdk: SDK,
            bestBlockNumber: number
        ) {
            const jailedAfter = (await stake.getJailed(
                sdk,
                bestBlockNumber
            )).map(prisoner => prisoner.address.toString());
            expect(jailedAfter).not.to.includes(
                alice.platformAddress.toString()
            );
        }

        it("alice should be banned from the prisoners", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1.5
            });

            const checkingNode = nodes[1];
            await termWaiter.waitNodeUntilTerm(checkingNode, {
                target: 2,
                termPeriods: 1
            });
            const blockNumber = await checkingNode.sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = (await stake.getTermMetadata(
                checkingNode.sdk,
                blockNumber
            ))!;
            expect(termMetadata!.currentTermId).to.be.equals(2);

            await expectPossibleAuthors(checkingNode.sdk, otherDynValidators);
            await ensureAliceIsJailed(
                checkingNode.sdk,
                termMetadata.lastTermFinishedBlockNumber
            );

            const aliceIdxInPrevTerm = await getAliceIndex(
                checkingNode.sdk,
                termMetadata.lastTermFinishedBlockNumber
            );

            const [message1, message2] = createDoubleVoteMessages(
                {
                    height: termMetadata.lastTermFinishedBlockNumber,
                    view: 0,
                    step: "precommit",
                    privKey: alice.privateKey,
                    signerIdx: aliceIdxInPrevTerm
                },
                H256.ensure(
                    "a556240c3ac4f33acbf78b33235ce13bc359bf96a01df5cc674539ae3b4978d0"
                ),
                H256.ensure(
                    "9900a2c6f1166026013f76fd7c366846265fa15edcfe06c88fc1a87b79e9b787"
                )
            );

            console.log(alice.publicKey);

            const reportTx = createReportDoubleVoteTransaction(
                checkingNode.sdk,
                message1,
                message2
            );
            const reportTxHash = await checkingNode.sdk.rpc.chain.sendSignedTransaction(
                reportTx.sign({
                    secret: faucetSecret,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                })
            );
            const blockNumberAfterReport = await waitUntilAliceGetBanned(
                checkingNode,
                reportTxHash
            );
            await ensureAliceIsBanned(checkingNode.sdk, blockNumberAfterReport);
            await ensureAliceIsReleased(
                checkingNode.sdk,
                blockNumberAfterReport
            );
        });
    });

    describe("Ban from the candidate state", async function() {
        const { nodes } = withNodes(this, {
            promiseExpect,
            validators: allDynValidators.map((signer, index) => ({
                signer,
                delegation: 5_000,
                deposit: 10_000_000 - index // tie-breaker
            })),
            overrideParams: {
                minNumOfValidators: 3
            }
        });

        async function ensureAliceIsACandidate(sdk: SDK, blockNumber?: number) {
            const candidatesBefore = (await stake.getCandidates(
                sdk,
                blockNumber
            )).map(candidate =>
                PlatformAddress.fromPublic(candidate.pubkey, {
                    networkId: "tc"
                }).toString()
            );
            expect(candidatesBefore).to.includes(
                alice.platformAddress.toString()
            );
        }

        async function ensureAliceIsNotACandidate(
            sdk: SDK,
            blockNumber?: number
        ) {
            const candidatesAfter = (await stake.getCandidates(
                sdk,
                blockNumber
            )).map(candidate =>
                PlatformAddress.fromPublic(candidate.pubkey, {
                    networkId: "tc"
                }).toString()
            );
            expect(candidatesAfter).not.to.includes(
                alice.platformAddress.toString()
            );
        }

        it("alice should be banned from the candidates", async function() {
            const termWaiter = setTermTestTimeout(this, {
                terms: 1.5
            });

            const checkingNode = nodes[1];
            const blockNumber = await checkingNode.sdk.rpc.chain.getBestBlockNumber();
            const termMetadata = await stake.getTermMetadata(
                checkingNode.sdk,
                blockNumber
            );
            const currentTermInitialBlockNumber =
                termMetadata!.lastTermFinishedBlockNumber + 1;
            await checkingNode.waitBlockNumber(
                currentTermInitialBlockNumber + 1
            );

            const aliceIdx = await getAliceIndex(
                checkingNode.sdk,
                currentTermInitialBlockNumber
            );

            const revoketx = stake
                .createRevokeTransaction(
                    checkingNode.sdk,
                    alice.platformAddress,
                    4_500
                )
                .sign({
                    secret: faucetSecret,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                });
            const revokeTxHash = await checkingNode.sdk.rpc.chain.sendSignedTransaction(
                revoketx
            );
            await checkingNode.waitForTx(revokeTxHash);
            await termWaiter.waitNodeUntilTerm(checkingNode, {
                target: 2,
                termPeriods: 1
            });
            await expectPossibleAuthors(checkingNode.sdk, otherDynValidators);
            await ensureAliceIsACandidate(checkingNode.sdk);

            const [message1, message2] = createDoubleVoteMessages(
                {
                    height: currentTermInitialBlockNumber,
                    view: 0,
                    step: "precommit",
                    privKey: alice.privateKey,
                    signerIdx: aliceIdx
                },
                H256.ensure(
                    "a3ea5219612cde721a61f099fadda0d23007e561b4c3a50d04097e5ac7ef1e24"
                ),
                H256.ensure(
                    "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4"
                )
            );

            const reportTx = createReportDoubleVoteTransaction(
                checkingNode.sdk,
                message1,
                message2
            );
            const reportTxHash = await checkingNode.sdk.rpc.chain.sendSignedTransaction(
                reportTx.sign({
                    secret: faucetSecret,
                    seq: await checkingNode.sdk.rpc.chain.getSeq(faucetAddress),
                    fee: 10
                })
            );
            const blockNumberAfterReport = await waitUntilAliceGetBanned(
                checkingNode,
                reportTxHash
            );
            await ensureAliceIsBanned(checkingNode.sdk, blockNumberAfterReport);
            await ensureAliceIsNotACandidate(
                checkingNode.sdk,
                blockNumberAfterReport
            );
        });
    });

    afterEach(async function() {
        promiseExpect.checkFulfilled();
    });
});
