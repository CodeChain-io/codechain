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
import { ChildProcess, spawn } from "child_process";
import { SDK } from "codechain-sdk";
import {
    Asset,
    AssetAddress,
    AssetTransferInput,
    ComposeAsset,
    DecomposeAsset,
    H256,
    PlatformAddress,
    SignedTransaction,
    Transaction,
    TransferAsset,
    U64,
    UnwrapCCC
} from "codechain-sdk/lib/core/classes";
import { AssetTransaction } from "codechain-sdk/lib/core/Transaction";
import { P2PKH } from "codechain-sdk/lib/key/P2PKH";
import { P2PKHBurn } from "codechain-sdk/lib/key/P2PKHBurn";
import * as stake from "codechain-stakeholder-sdk";
import { createWriteStream, mkdtempSync, unlinkSync } from "fs";
import * as mkdirp from "mkdirp";
import { ncp } from "ncp";
import { createInterface as createReadline } from "readline";
import { faucetAddress, faucetSecret } from "./constants";
import { wait } from "./promise";

const projectRoot = `${__dirname}/../../..`;

export type SchemeFilepath = string;
export type ChainType =
    | "solo"
    | "simple_poa"
    | "tendermint"
    | "cuckoo"
    | "blake_pow"
    | "husky"
    | SchemeFilepath;
export type ProcessState =
    | "stopped"
    | "initializing"
    | "running"
    | "stopping"
    | { state: "error"; message: string; source?: Error };
export class ProcessStateError extends Error {
    constructor(state: ProcessState) {
        if (typeof state === "string") {
            super(`Process state is invalid: ${state}`);
        } else {
            super(
                `Process state is invalid: ${JSON.stringify(
                    state,
                    undefined,
                    4
                )}`
            );
        }
    }
}
export default class CodeChain {
    private static idCounter = 0;
    private readonly _id: number;
    private readonly _sdk: SDK;
    private readonly _localKeyStorePath: string;
    private readonly _dbPath: string;
    private readonly _ipcPath: string;
    private readonly _keysPath: string;
    private readonly _logFile: string;
    private readonly _logPath: string;
    private readonly _chain: ChainType;
    private readonly _rpcPort: number;
    private readonly argv: string[];
    private readonly env: { [key: string]: string };
    private process?: ChildProcess;
    private _processState: ProcessState;
    private _keepLogs: boolean;
    private readonly keyFileMovePromise?: Promise<{}>;

    public get id(): number {
        return this._id;
    }
    public get sdk(): SDK {
        if (this._processState === "running") {
            return this._sdk;
        } else {
            throw new ProcessStateError(this._processState);
        }
    }
    public get localKeyStorePath(): string {
        return this._localKeyStorePath;
    }
    public get dbPath(): string {
        return this._dbPath;
    }
    public get ipcPath(): string {
        return this._ipcPath;
    }
    public get keysPath(): string {
        return this._keysPath;
    }
    public get logFile(): string {
        return this._logFile;
    }
    public get logPath(): string {
        return this._logPath;
    }
    public get rpcPort(): number {
        return this._rpcPort;
    }
    public get port(): number {
        return 3486 + this.id;
    }
    public get secretKey(): number {
        return 1 + this.id;
    }
    public get chain(): ChainType {
        return this._chain;
    }
    public get processState(): ProcessState {
        return this._processState;
    }

    constructor(
        options: {
            chain?: ChainType;
            argv?: string[];
            additionalKeysPath?: string;
            rpcPort?: number;
            env?: { [key: string]: string };
        } = {}
    ) {
        const { chain, argv, additionalKeysPath, env } = options;
        this._id = CodeChain.idCounter++;

        const { rpcPort = 8081 + this.id } = options;
        this._rpcPort = rpcPort;

        mkdirp.sync(`${projectRoot}/db/`);
        mkdirp.sync(`${projectRoot}/keys/`);
        mkdirp.sync(`${projectRoot}/test/log/`);
        this._dbPath = mkdtempSync(`${projectRoot}/db/`);
        this._ipcPath = `/tmp/jsonrpc.${new Date()
            .toISOString()
            .replace(/[-:.]/g, "_")}.${this.id}.ipc`;
        this._keysPath = mkdtempSync(`${projectRoot}/keys/`);
        if (additionalKeysPath) {
            this.keyFileMovePromise = new Promise((resolve, reject) => {
                ncp(additionalKeysPath, this._keysPath, err => {
                    if (err) {
                        console.error(err);
                        reject(err);
                        return;
                    }
                    resolve();
                });
            });
        }
        this._localKeyStorePath = `${this.keysPath}/keystore.db`;
        this._logFile = `${new Date().toISOString().replace(/[-:.]/g, "_")}.${
            this.id
        }.log`;
        this._logPath = `${projectRoot}/test/log/${this._logFile}`;
        this._sdk = new SDK({ server: `http://localhost:${this.rpcPort}` });
        this._chain = chain || "solo";
        this.argv = argv || [];
        this.env = env || {};
        this._processState = "stopped";
        this._keepLogs = false;
    }

    public async start(params?: {
        argv?: string[];
        logLevel?: string;
        disableLog?: boolean;
        disableIpc?: boolean;
    }) {
        if (this._processState !== "stopped") {
            throw new ProcessStateError(this._processState);
        }

        const {
            argv = [],
            logLevel = "trace,mio=warn,tokio=warn,hyper=warn,timer=warn",
            disableLog = false,
            disableIpc = true
        } = params || {};
        if (this.keyFileMovePromise) {
            await this.keyFileMovePromise;
        }
        const useDebugBuild = process.env.NODE_ENV !== "production";
        process.env.RUST_LOG = logLevel;

        const baseArgs = [...this.argv, ...argv];
        if (disableIpc) {
            baseArgs.push("--no-ipc");
        } else {
            baseArgs.push("--ipc-path");
            baseArgs.push(this.ipcPath);
        }

        // Resolves when CodeChain initialization completed.
        return new Promise((resolve, reject) => {
            this._keepLogs = true;
            this._processState = "initializing";
            this.process = spawn(
                `target/${useDebugBuild ? "debug" : "release"}/codechain`,
                [
                    ...baseArgs,
                    "--chain",
                    this.chain,
                    "--db-path",
                    this.dbPath,
                    "--keys-path",
                    this.keysPath,
                    "--no-ws",
                    "--jsonrpc-port",
                    this.rpcPort.toString(),
                    "--port",
                    this.port.toString(),
                    "--instance-id",
                    this.id.toString()
                ],
                {
                    cwd: projectRoot,
                    env: {
                        ...process.env,
                        ...this.env
                    }
                }
            );
            if (!disableLog) {
                const logStream = createWriteStream(this.logPath, {
                    flags: "a"
                });
                this.process!.stdout!.pipe(logStream);
                this.process!.stderr!.pipe(logStream);
            }

            const readline = createReadline({ input: this.process!.stderr! });
            const self = this;
            function clearListeners() {
                if (self.process) {
                    self.process
                        .removeListener("error", onError)
                        .removeListener("exit", onExit);
                    readline.removeListener("line", onLine);
                }
            }
            function onError(e: Error) {
                clearListeners();
                self.process = undefined;
                self._processState = {
                    state: "error",
                    message: "Error while spawning CodeChain",
                    source: e
                };
                self._keepLogs = true;
                reject(new ProcessStateError(self._processState));
            }
            function onExit(code: number, signal: number) {
                clearListeners();
                self.process = undefined;
                self._processState = {
                    state: "error",
                    message: `CodeChain unexpectedly exited on start: code ${code}, signal ${signal}`
                };
                self._keepLogs = true;
                reject(new ProcessStateError(self._processState));
            }
            function onLine(line: string) {
                if (line.includes("Initialization complete")) {
                    clearListeners();
                    self._processState = "running";
                    self._keepLogs = false;
                    self.process!.on("exit", (code, signal) => {
                        self.process = undefined;
                        self._processState = {
                            state: "error",
                            message: `CodeChain unexpectedly exited while running: code ${code}, signal ${signal}`
                        };
                        self._keepLogs = true;
                    });
                    resolve();
                }
            }

            this.process.on("error", onError).on("exit", onExit);
            readline.on("line", onLine);
        });
    }

    public keepLogs() {
        console.log(`Keep log file: ${this._logPath}`);
        this._keepLogs = true;
    }

    public async clean() {
        return new Promise((resolve, reject) => {
            if (!this.process) {
                return resolve();
            }
            this.process
                .removeAllListeners("error")
                .on("error", e => {
                    this._processState = {
                        state: "error",
                        message: "CodeChain unexpectedly exited on clean",
                        source: e
                    };
                    reject(new ProcessStateError(this._processState));
                })
                .removeAllListeners("exit")
                .on("exit", (code, signal) => {
                    if (code !== 0) {
                        console.error(
                            `CodeChain(${this.id}) exited with code ${code}, ${signal}`
                        );
                    } else if (!this._keepLogs) {
                        unlinkSync(this.logPath);
                    }
                    this._processState = "stopped";
                    resolve();
                });
            this._processState = "stopping";
            this.process.kill();
            this.process = undefined;
        });
    }

    public async connect(peer: CodeChain) {
        if (!this.process) {
            return Promise.reject(Error("process isn't available"));
        }
        await this.sdk.rpc.network.connect("127.0.0.1", peer.port);
        while (
            (await this.sdk.rpc.network.isConnected("127.0.0.1", peer.port)) ===
            false
        ) {
            await wait(250);
        }
    }

    public async disconnect(peer: CodeChain) {
        if (!this.process) {
            return Promise.reject(Error("process isn't available"));
        }
        return this.sdk.rpc.network.disconnect("127.0.0.1", peer.port);
    }

    public async waitPeers(n: number) {
        while (n > (await this.sdk.rpc.network.getPeerCount())) {
            await wait(500);
        }
        return;
    }

    public async waitBlockNumberSync(peer: CodeChain) {
        while (
            (await this.getBestBlockNumber()) !==
            (await peer.getBestBlockNumber())
        ) {
            await wait(500);
        }
    }

    public async waitBlockNumber(n: number) {
        while ((await this.getBestBlockNumber()) < n) {
            await wait(500);
        }
    }

    public async getBestBlockNumber() {
        return this.sdk.rpc.chain.getBestBlockNumber();
    }

    public async getBestBlockHash() {
        return this.sdk.rpc.chain.getBlockHash(await this.getBestBlockNumber());
    }

    public async createP2PKHAddress() {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        const p2pkh = this.sdk.key.createP2PKH({ keyStore });
        return p2pkh.createAddress();
    }

    public async signTransactionP2PKHBurn(
        txInput: AssetTransferInput,
        txhash: H256
    ) {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        const p2pkhBurn = this.sdk.key.createP2PKHBurn({ keyStore });
        if (txInput.prevOut.parameters === undefined) {
            throw Error(`prevOut.parameters is undefined`);
        }
        const publicKeyHash = Buffer.from(
            txInput.prevOut.parameters[0]
        ).toString("hex");
        txInput.setLockScript(P2PKHBurn.getLockScript());
        txInput.setUnlockScript(
            await p2pkhBurn.createUnlockScript(publicKeyHash, txhash)
        );
    }

    public async signTransactionP2PKH(
        txInput: AssetTransferInput,
        txhash: H256
    ) {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        const p2pkh = this.sdk.key.createP2PKH({ keyStore });
        if (txInput.prevOut.parameters === undefined) {
            throw Error(`prevOut.parameters is undefined`);
        }
        const publicKeyHash = Buffer.from(
            txInput.prevOut.parameters[0]
        ).toString("hex");
        txInput.setLockScript(P2PKH.getLockScript());
        txInput.setUnlockScript(
            await p2pkh.createUnlockScript(publicKeyHash, txhash)
        );
    }

    public async createP2PKHBurnAddress() {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        const p2pkhBurn = this.sdk.key.createP2PKHBurn({ keyStore });
        return p2pkhBurn.createAddress();
    }

    public async createPlatformAddress() {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        return this.sdk.key.createPlatformAddress({ keyStore });
    }

    public async pay(
        recipient: string | PlatformAddress,
        quantity: U64 | string | number
    ): Promise<H256> {
        const tx = this.sdk.core
            .createPayTransaction({
                recipient,
                quantity
            })
            .sign({
                secret: faucetSecret,
                seq: await this.sdk.rpc.chain.getSeq(faucetAddress),
                fee: 10
            });
        return this.sdk.rpc.chain.sendSignedTransaction(tx);
    }

    public async sendTransaction(
        tx: Transaction,
        params: {
            account: string | PlatformAddress;
            fee?: number | string | U64;
            seq?: number;
        }
    ) {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        const { account, fee = 10 } = params;
        const { seq = await this.sdk.rpc.chain.getSeq(account) } = params;
        const signed = await this.sdk.key.signTransaction(tx, {
            keyStore,
            account,
            fee,
            seq
        });
        return this.sdk.rpc.chain.sendSignedTransaction(signed);
    }

    public async sendAssetTransaction(
        tx: AssetTransaction & Transaction,
        options?: {
            seq?: number;
            fee?: number;
            secret?: string;
        }
    ): Promise<H256> {
        const {
            seq = (await this.sdk.rpc.chain.getSeq(faucetAddress)) || 0,
            fee = 10,
            secret = faucetSecret
        } = options || {};
        const signed = tx.sign({
            secret,
            fee: fee + this.id,
            seq
        });
        return this.sdk.rpc.chain.sendSignedTransaction(signed);
    }

    public async mintAsset(params: {
        supply: U64 | number;
        recipient?: string | AssetAddress;
        secret?: string;
        seq?: number;
        metadata?: string;
        registrar?: PlatformAddress | string;
        awaitMint?: boolean;
    }): Promise<Asset> {
        const {
            supply,
            seq,
            recipient = await this.createP2PKHAddress(),
            secret,
            metadata = "",
            registrar,
            awaitMint = true
        } = params;
        const tx = this.sdk.core.createMintAssetTransaction({
            scheme: {
                shardId: 0,
                metadata,
                supply,
                registrar
            },
            recipient
        });
        await this.sendAssetTransaction(tx, {
            secret,
            seq
        });
        return tx.getMintedAsset();
    }

    public async signTransactionInput(
        tx: TransferAsset | ComposeAsset | DecomposeAsset,
        index: number
    ) {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        await this.sdk.key.signTransactionInput(tx, index, { keyStore });
    }

    public async signTransactionBurn(
        tx: TransferAsset | UnwrapCCC,
        index: number
    ) {
        const keyStore = await this.sdk.key.createLocalKeyStore(
            this.localKeyStorePath
        );
        await this.sdk.key.signTransactionBurn(tx, index, { keyStore });
    }

    public async setRegularKey(
        key: any,
        options?: {
            seq?: number;
            secret?: any;
        }
    ): Promise<H256> {
        const {
            seq = (await this.sdk.rpc.chain.getSeq(faucetAddress)) || 0,
            secret = faucetSecret
        } = options || {};
        const tx = this.sdk.core
            .createSetRegularKeyTransaction({
                key
            })
            .sign({
                secret,
                fee: 10,
                seq
            });

        return this.sdk.rpc.chain.sendSignedTransaction(tx);
    }

    public async sendPayTx(options?: {
        seq?: number;
        recipient?: PlatformAddress | string;
        quantity?: number;
        secret?: any;
        fee?: number;
    }): Promise<SignedTransaction> {
        const {
            seq = (await this.sdk.rpc.chain.getSeq(faucetAddress)) || 0,
            recipient = "tccqxv9y4cw0jwphhu65tn4605wadyd2sxu5yezqghw",
            quantity = 0,
            secret = faucetSecret,
            fee = 10 + this.id
        } = options || {};
        const tx = this.sdk.core
            .createPayTransaction({
                recipient,
                quantity
            })
            .sign({
                secret,
                fee,
                seq
            });
        await this.sdk.rpc.chain.sendSignedTransaction(tx);
        return tx;
    }

    // If one only sends certainly failing transactions, the miner would not generate any block.
    // So to clearly check the result failed, insert the failing transactions after succeessful ones.
    public async sendAssetTransactionExpectedToFail(
        tx: Transaction & AssetTransaction,
        options: { seq?: number } = {}
    ): Promise<H256> {
        await this.sdk.rpc.devel.stopSealing();

        const seq =
            options.seq == null
                ? await this.sdk.rpc.chain.getSeq(faucetAddress)
                : options.seq;

        const blockNumber = await this.getBestBlockNumber();
        const signedDummyTxHash = (await this.sendPayTx({
            seq,
            quantity: 1
        })).hash();
        const targetTxHash = await this.sendAssetTransaction(tx, {
            seq: seq + 1
        });

        await this.sdk.rpc.devel.startSealing();
        await this.waitBlockNumber(blockNumber + 1);

        expect(await this.sdk.rpc.chain.containsTransaction(targetTxHash)).be
            .false;
        expect(await this.sdk.rpc.chain.getErrorHint(targetTxHash)).not.null;
        expect(await this.sdk.rpc.chain.getTransaction(targetTxHash)).be.null;

        expect(await this.sdk.rpc.chain.containsTransaction(signedDummyTxHash))
            .be.true;
        expect(await this.sdk.rpc.chain.getErrorHint(signedDummyTxHash)).null;
        expect(await this.sdk.rpc.chain.getTransaction(signedDummyTxHash)).not
            .be.null;
        return targetTxHash;
    }

    // If one only sends certainly failing transactions, the miner would not generate any block.
    // So to clearly check the result failed, insert the failing transactions after succeessful ones.
    public async sendTransactionExpectedToFail(
        tx: Transaction,
        options: { account: string | PlatformAddress }
    ): Promise<H256> {
        const { account } = options;
        await this.sdk.rpc.devel.stopSealing();

        const blockNumber = await this.getBestBlockNumber();
        const signedDummyTxHash = (await this.sendPayTx({
            quantity: 1
        })).hash();
        const targetTxHash = await this.sendTransaction(tx, { account });

        await this.sdk.rpc.devel.startSealing();
        await this.waitBlockNumber(blockNumber + 1);

        expect(await this.sdk.rpc.chain.containsTransaction(targetTxHash)).be
            .false;
        expect(await this.sdk.rpc.chain.getErrorHint(targetTxHash)).not.null;
        expect(await this.sdk.rpc.chain.getTransaction(targetTxHash)).be.null;

        expect(await this.sdk.rpc.chain.containsTransaction(signedDummyTxHash))
            .be.true;
        expect(await this.sdk.rpc.chain.getErrorHint(signedDummyTxHash)).null;
        expect(await this.sdk.rpc.chain.getTransaction(signedDummyTxHash)).not
            .be.null;

        return targetTxHash;
    }

    public async sendSignedTransactionExpectedToFail(
        tx: SignedTransaction | (() => Promise<H256>),
        options: { error?: string } = {}
    ): Promise<H256> {
        await this.sdk.rpc.devel.stopSealing();

        const blockNumber = await this.getBestBlockNumber();
        const signedDummyTxHash = (await this.sendPayTx({
            fee: 1000,
            quantity: 1
        })).hash();

        const targetTxHash =
            tx instanceof SignedTransaction
                ? await this.sdk.rpc.chain.sendSignedTransaction(tx)
                : await tx();

        await this.sdk.rpc.devel.startSealing();
        await this.waitBlockNumber(blockNumber + 1);

        expect(await this.sdk.rpc.chain.containsTransaction(targetTxHash)).be
            .false;
        const hint = await this.sdk.rpc.chain.getErrorHint(targetTxHash);
        expect(hint).not.null;
        if (options.error != null) {
            expect(hint).contains(options.error);
        }
        expect(await this.sdk.rpc.chain.getTransaction(targetTxHash)).be.null;

        expect(await this.sdk.rpc.chain.containsTransaction(signedDummyTxHash))
            .be.true;
        expect(await this.sdk.rpc.chain.getErrorHint(signedDummyTxHash)).null;
        expect(await this.sdk.rpc.chain.getTransaction(signedDummyTxHash)).not
            .be.null;

        return targetTxHash;
    }

    public sendSignedTransactionWithRlpBytes(rlpBytes: Buffer): Promise<H256> {
        return new Promise((resolve, reject) => {
            const bytes = Array.from(rlpBytes)
                .map(byte =>
                    byte < 0x10 ? `0${byte.toString(16)}` : byte.toString(16)
                )
                .join("");
            this.sdk.rpc
                .sendRpcRequest("mempool_sendSignedTransaction", [`0x${bytes}`])
                .then(result => {
                    try {
                        resolve(new H256(result));
                    } catch (e) {
                        reject(
                            Error(
                                `Expected sendSignedTransaction() to return a value of H256, but an error occurred: ${e.toString()}`
                            )
                        );
                    }
                })
                .catch(reject);
        });
    }

    public async waitForTx(
        hashlikes: H256 | Promise<H256> | (H256 | Promise<H256>)[],
        option?: { timeout?: number }
    ) {
        const { timeout = 10000 } = option || {};

        const hashes = await Promise.all(
            Array.isArray(hashlikes) ? hashlikes : [hashlikes]
        );

        const containsAll = async () => {
            const contains = await Promise.all(
                hashes.map(hash => this.sdk.rpc.chain.containsTransaction(hash))
            );
            return contains.every(x => x);
        };
        const checkNoError = async () => {
            const errorHints = await Promise.all(
                hashes.map(hash => this.sdk.rpc.chain.getErrorHint(hash))
            );
            for (const errorHint of errorHints) {
                if (errorHint !== null && errorHint !== "") {
                    throw Error(`waitForTx: Error found: ${errorHint}`);
                }
            }
        };

        const start = Date.now();
        while (!(await containsAll())) {
            await checkNoError();

            await wait(500);
            if (Date.now() - start >= timeout) {
                throw Error("Timeout on waitForTx");
            }
        }
        await checkNoError();
    }

    public async waitForTermChange(target: number, timeout?: number) {
        const start = Date.now();
        while (true) {
            const termMetadata = await stake.getTermMetadata(this.sdk);
            if (termMetadata && termMetadata.currentTermId >= target) {
                break;
            }
            await wait(1000);
            if (timeout) {
                if (Date.now() - start > timeout * 1000) {
                    throw new Error(`Term didn't changed in ${timeout} s`);
                }
            }
        }
    }
}
