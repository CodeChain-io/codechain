import {
    blake256,
    H256,
    PlatformAddress,
    U64Value
} from "codechain-primitives/lib";
import { SDK } from "codechain-sdk";
import * as stake from "codechain-stakeholder-sdk";
import { Context, Suite } from "mocha";
import {
    aliceSecret,
    bobSecret,
    faucetAddress,
    faucetSecret,
    stakeActionHandlerId,
    validator0Address,
    validator1Address,
    validator2Address,
    validator3Address
} from "../helper/constants";
import { PromiseExpect, wait } from "../helper/promise";
import CodeChain from "../helper/spawn";

const RLP = require("rlp");

interface ValidatorConfig {
    signer: {
        privateKey: string;
        publicKey: string;
        accountId: string;
        platformAddress: PlatformAddress;
    };
    deposit?: U64Value;
    delegation?: U64Value;
}

export function withNodes(
    suite: Suite,
    options: {
        promiseExpect: PromiseExpect;
        validators: ValidatorConfig[];
        overrideParams?: Partial<typeof defaultParams>;
        onBeforeEnable?: (nodes: CodeChain[]) => Promise<void>;
    }
) {
    const nodes: CodeChain[] = [];
    const { overrideParams = {} } = options;
    const initialParams = {
        ...defaultParams,
        ...overrideParams
    };
    suite.beforeEach(async function() {
        const termSeconds = initialParams.termSeconds;
        const secsPerBlock = 5;
        this.slow((secsPerBlock * 3 + termSeconds) * 1000); // createNodes will wait for 3 blocks + at most 1 terms
        this.timeout((secsPerBlock * 3 + termSeconds) * 2 * 1000);
        nodes.length = 0;
        const newNodes = await createNodes({
            ...options,
            initialParams
        });
        nodes.push(...newNodes);
    });
    suite.afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            nodes.map(node => node.keepLogs());
        }
        await Promise.all(nodes.map(node => node.clean()));
    });
    return {
        nodes,
        initialParams
    };
}

async function createNodes(options: {
    promiseExpect: PromiseExpect;
    validators: ValidatorConfig[];
    initialParams: typeof defaultParams;
    onBeforeEnable?: (nodes: CodeChain[]) => Promise<void>;
}): Promise<CodeChain[]> {
    const chain = `${__dirname}/../scheme/tendermint-dynval.json`;
    const { promiseExpect, validators, initialParams } = options;

    const initialNodes: CodeChain[] = [];
    const initialValidators = [
        validator0Address,
        validator1Address,
        validator2Address,
        validator3Address
    ];
    for (let i = 0; i < initialValidators.length; i++) {
        initialNodes[i] = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                initialValidators[i].value,
                "--password-path",
                "test/tendermint/password.json",
                "--force-sealing"
            ],
            additionalKeysPath: "tendermint/keys"
        });
    }

    const nodes: CodeChain[] = [];
    for (let i = 0; i < validators.length; i++) {
        const { signer: validator } = validators[i];
        nodes[i] = new CodeChain({
            chain,
            argv: [
                "--engine-signer",
                validator.platformAddress.value,
                "--password-path",
                `test/tendermint.dynval/${validator.platformAddress.value}/password.json`,
                "--force-sealing"
            ],
            additionalKeysPath: `tendermint.dynval/${validator.platformAddress.value}/keys`
        });
    }
    let bootstrapFailed = false;
    try {
        await promiseExpect.shouldFulfill(
            "start",
            Promise.all(initialNodes.concat(nodes).map(node => node.start()))
        );
        await fullyConnect(initialNodes.concat(nodes), promiseExpect);

        // Give CCC to deposit
        const faucetSeq = await initialNodes[0].sdk.rpc.chain.getSeq(
            faucetAddress
        );
        const payTxs: (H256 | Promise<H256>)[] = [];
        for (let i = 0; i < validators.length; i++) {
            const { signer: validator } = validators[i];
            const tx = initialNodes[0].sdk.core
                .createPayTransaction({
                    recipient: validator.platformAddress,
                    quantity: 100_000_000
                })
                .sign({
                    secret: faucetSecret,
                    seq: faucetSeq + i,
                    fee: 10
                });
            payTxs.push(
                await initialNodes[0].sdk.rpc.chain.sendSignedTransaction(tx)
            );
        }

        // Self nominate
        const stakeTxs = [];
        for (let i = 0; i < validators.length; i++) {
            const { signer: validator, deposit } = validators[i];
            await promiseExpect.shouldFulfill(
                `node ${i} wait for pay`,
                nodes[i].waitForTx(payTxs)
            );
            if (deposit == null) {
                continue;
            }
            const tx = stake
                .createSelfNominateTransaction(nodes[i].sdk, deposit, "")
                .sign({
                    secret: validator.privateKey,
                    seq: await nodes[i].sdk.rpc.chain.getSeq(
                        validator.platformAddress
                    ),
                    fee: 10
                });
            stakeTxs.push(
                await nodes[i].sdk.rpc.chain.sendSignedTransaction(tx)
            );
        }

        // Delegate CCS to become validators
        const faucetSeq2 = await initialNodes[0].sdk.rpc.chain.getSeq(
            faucetAddress
        );
        const delegateTxs = [];
        for (let i = 0; i < validators.length; i++) {
            const { signer: validator, deposit, delegation = 0 } = validators[
                i
            ];
            await promiseExpect.shouldFulfill(
                `node ${i} wait for stake`,
                nodes[i].waitForTx(stakeTxs)
            );
            if (deposit == null && delegation !== 0) {
                throw new Error(
                    "Cannot delegate to who haven't self-nominated"
                );
            }
            if (delegation === 0) {
                continue;
            }
            const tx = stake
                .createDelegateCCSTransaction(
                    initialNodes[0].sdk,
                    validator.platformAddress,
                    delegation
                )
                .sign({
                    secret: faucetSecret,
                    seq: faucetSeq2 + delegateTxs.length,
                    fee: 10
                });
            delegateTxs.push(
                await initialNodes[0].sdk.rpc.chain.sendSignedTransaction(tx)
            );
        }

        for (let i = 0; i < validators.length; i++) {
            await promiseExpect.shouldFulfill(
                `node ${i} wait for delegate`,
                nodes[i].waitForTx(delegateTxs)
            );
        }

        if (options.onBeforeEnable) {
            await options.onBeforeEnable(nodes);
        }

        const runningNodes = nodes.filter(node => node.isRunning);
        if (runningNodes.length === 0) {
            throw new Error("Cannot proceed with no running nodes");
        }

        // enable!
        const changeTx = await changeParams(initialNodes[0], 0, initialParams);

        for (const node of runningNodes) {
            // nodes can be cleaned in `onBeforeEnable`
            await promiseExpect.shouldFulfill(
                `node ${nodes.findIndex(x => x === node)} wait for changeTx`,
                node.waitForTx(changeTx)
            );
        }
        await runningNodes[0].waitForTermChange(1);

        return nodes;
    } catch (e) {
        initialNodes.concat(nodes).forEach(node => node.keepLogs());
        bootstrapFailed = true;
        throw e;
    } finally {
        await Promise.all(initialNodes.map(node => node.clean()));
        if (bootstrapFailed) {
            await Promise.all(nodes.map(node => node.clean()));
        }
    }
}

export async function selfNominate(
    sdk: SDK,
    validator: ValidatorConfig["signer"],
    deposit: number
): Promise<H256> {
    const tx = stake.createSelfNominateTransaction(sdk, deposit, "").sign({
        secret: validator.privateKey,
        seq: await sdk.rpc.chain.getSeq(validator.platformAddress),
        fee: 10
    });

    return await sdk.rpc.chain.sendSignedTransaction(tx);
}

export async function receiveDelegation(
    sdk: SDK,
    validator: ValidatorConfig["signer"],
    delegation: number
): Promise<H256> {
    const tx = stake
        .createDelegateCCSTransaction(
            sdk,
            validator.platformAddress,
            delegation
        )
        .sign({
            secret: faucetSecret,
            seq: await sdk.rpc.chain.getSeq(faucetAddress),
            fee: 10
        });
    return await sdk.rpc.chain.sendSignedTransaction(tx);
}

export async function fullyConnect(
    nodes: CodeChain[],
    promiseExpect: PromiseExpect
) {
    const graph: { from: number; to: number }[] = [];
    for (let i = 0; i < nodes.length - 1; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
            graph.push({ from: i, to: j });
        }
    }

    await promiseExpect.shouldFulfill(
        "connect each other",
        Promise.all(graph.map(({ from, to }) => nodes[from].connect(nodes[to])))
    );
    await promiseExpect.shouldFulfill(
        "wait for connect",
        Promise.all(nodes.map(node => node.waitPeers(nodes.length - 1)))
    );
}

export const defaultParams = {
    maxExtraDataSize: 0x20,
    maxAssetSchemeMetadataSize: 0x0400,
    maxTransferMetadataSize: 0x0100,
    maxTextContentSize: 0x0200,
    networkID: "tc",
    minPayCost: 10,
    minSetRegularKeyCost: 10,
    minCreateShardCost: 10,
    minSetShardOwnersCost: 10,
    minSetShardUsersCost: 10,
    minWrapCccCost: 10,
    minCustomCost: 10,
    minStoreCost: 10,
    minRemoveCost: 10,
    minMintAssetCost: 10,
    minTransferAssetCost: 10,
    minChangeAssetSchemeCost: 10,
    minIncreaseAssetSupplyCost: 10,
    minComposeAssetCost: 10,
    minDecomposeAssetCost: 10,
    minUnwrapCccCost: 10,
    maxBodySize: 4194304,
    snapshotPeriod: 16384,

    termSeconds: 20,
    nominationExpiration: 10,
    custodyPeriod: 10,
    releasePeriod: 30,
    maxNumOfValidators: 8,
    minNumOfValidators: 4,
    delegationThreshold: 1000,
    minDeposit: 10000,
    maxCandidateMetadataSize: 128
};

export async function changeParams(
    node: CodeChain,
    metadataSeq: number,
    params: typeof defaultParams
) {
    const newParams: any[] = [
        params.maxExtraDataSize,
        params.maxAssetSchemeMetadataSize,
        params.maxTransferMetadataSize,
        params.maxTextContentSize,
        params.networkID,
        params.minPayCost,
        params.minSetRegularKeyCost,
        params.minCreateShardCost,
        params.minSetShardOwnersCost,
        params.minSetShardUsersCost,
        params.minWrapCccCost,
        params.minCustomCost,
        params.minStoreCost,
        params.minRemoveCost,
        params.minMintAssetCost,
        params.minTransferAssetCost,
        params.minChangeAssetSchemeCost,
        params.minIncreaseAssetSupplyCost,
        params.minComposeAssetCost,
        params.minDecomposeAssetCost,
        params.minUnwrapCccCost,
        params.maxBodySize,
        params.snapshotPeriod,
        params.termSeconds,
        params.nominationExpiration,
        params.custodyPeriod,
        params.releasePeriod,
        params.maxNumOfValidators,
        params.minNumOfValidators,
        params.delegationThreshold,
        params.minDeposit,
        params.maxCandidateMetadataSize
    ];
    const changeParamsActionRlp: [
        number,
        number,
        (number | string)[],
        ...string[]
    ] = [0xff, metadataSeq, newParams];
    const message = blake256(RLP.encode(changeParamsActionRlp).toString("hex"));
    changeParamsActionRlp.push(
        `0x${SDK.util.signEcdsa(message, faucetSecret)}`
    );
    changeParamsActionRlp.push(`0x${SDK.util.signEcdsa(message, aliceSecret)}`);
    changeParamsActionRlp.push(`0x${SDK.util.signEcdsa(message, bobSecret)}`);

    return await node.sdk.rpc.chain.sendSignedTransaction(
        node.sdk.core
            .createCustomTransaction({
                handlerId: stakeActionHandlerId,
                bytes: RLP.encode(changeParamsActionRlp)
            })
            .sign({
                secret: faucetSecret,
                seq: await node.sdk.rpc.chain.getSeq(faucetAddress),
                fee: 10
            })
    );
}

interface TermWaiter {
    termSeconds: number;
    waitForTermPeriods(termPeriods: number, margin: number): Promise<void>;
    waitNodeUntilTerm(
        node: CodeChain,
        params: {
            target: number;
            termPeriods: number;
        }
    ): Promise<void>;
}

export function setTermTestTimeout(
    context: Context | Suite,
    options: {
        terms: number;
        params?: {
            termSeconds: number;
        };
    }
): TermWaiter {
    const { terms, params: { termSeconds } = defaultParams } = options;
    const slowMargin = 0.5;
    const timeoutMargin = 2.0;
    context.slow(termSeconds * (terms + slowMargin) * 1000);
    context.timeout(termSeconds * (terms + timeoutMargin) * 1000);
    function termPeriodsToTime(termPeriods: number, margin: number): number {
        return (termPeriods + margin) * termSeconds;
    }
    return {
        termSeconds,
        async waitForTermPeriods(termPeriods: number, margin: number) {
            await wait(termPeriodsToTime(termPeriods, margin) * 1000);
        },
        async waitNodeUntilTerm(
            node: CodeChain,
            waiterParams: {
                target: number;
                termPeriods: number;
            }
        ) {
            await node.waitForTermChange(
                waiterParams.target,
                termPeriodsToTime(waiterParams.termPeriods, 0.5)
            );
        }
    };
}
