// Copyright 2018 Kodebox, Inc.
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

import { ChildProcess, spawn } from "child_process";
import { SDK } from "codechain-sdk";
import { SignedParcel } from "codechain-sdk/lib/core/classes";
import { PlatformAddress } from "codechain-sdk/lib/key/classes";
import { mkdtempSync, appendFileSync } from "fs";
import { createInterface as createReadline } from "readline";
import * as mkdirp from "mkdirp";
import { wait } from "./promise";
import { makeRandomFilename } from "./random";

const faucetSecret = `ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd`;
const faucetAddress = PlatformAddress.fromAccountId(SDK.util.getAccountIdFromPrivate(`ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd`));
const projectRoot = `${__dirname}/../../..`;

export type ChainType = "solo" | "solo_authority" | "tendermint" | "cuckoo" | "blake_pow" | "husky";

export default class CodeChain {
  private static idCounter = 0;
  private _id: number;
  private _sdk: SDK;
  private _dbPath: string;
  private _ipcPath: string;
  private _keysPath: string;
  private _logFile: string;
  private _logPath: string;
  private _logFlag: boolean;
  private _chain: ChainType;
  private argv: string[];
  private process?: ChildProcess;

  public get id(): number { return this._id; }
  public get sdk(): SDK { return this._sdk; }
  public get dbPath(): string { return this._dbPath; }
  public get ipcPath(): string { return this._ipcPath; }
  public get keysPath(): string { return this._keysPath; }
  public get logFile(): string { return this._logFile; }
  public get logPath(): string { return this._logPath; }
  public get logFlag(): boolean { return this._logFlag; }
  public get rpcPort(): number { return 8081 + this.id; }
  public get port(): number { return 3486 + this.id; }
  public get secretKey(): number { return 1 + this.id; }
  public get chain(): ChainType { return this._chain; }

  constructor(options: { chain?: ChainType, argv?: string[], logFlag?: boolean } = {}) {
    const { chain, argv, logFlag } = options;
    this._id = CodeChain.idCounter++;

    mkdirp.sync(`${projectRoot}/db/`);
    mkdirp.sync(`${projectRoot}/keys/`);
    mkdirp.sync(`${projectRoot}/test/log/`);
    this._dbPath = mkdtempSync(`${projectRoot}/db/`);
    this._ipcPath = `/tmp/jsonrpc.${this.id}.ipc`;
    this._keysPath = mkdtempSync(`${projectRoot}/keys/`);
    this._logFlag = logFlag || false;
    this._logFile = makeRandomFilename(".log");
    this._logPath = `${projectRoot}/test/log/${this._logFile}`;
    this._sdk = new SDK({ server: `http://localhost:${this.rpcPort}` });
    this._chain = chain || "solo";
    this.argv = argv || [];
  }

  public async start(argv: string[] = [], log_level = "trace") {
    const useDebugBuild = process.env.NODE_ENV !== "production";
    process.env.RUST_LOG = log_level;

    // Resolves when CodeChain initialization completed.
    return new Promise((resolve, reject) => {
      this.process = spawn(
        `target/${useDebugBuild ? "debug" : "release"}/codechain`,
        [
          ...this.argv,
          ...argv,
          "--chain", this.chain,
          "--db-path", this.dbPath,
          "--ipc-path", this.ipcPath,
          "--keys-path", this.keysPath,
          "--jsonrpc-port", this.rpcPort.toString(),
          "--port", this.port.toString(),
          "--instance-id", this.id.toString(),
        ],
        {
          cwd: projectRoot,
          env: process.env
        });

      this.process
        .on("error", e => {
          reject(e);
        })
        .on("close", (code, _signal) => {
          reject(Error(`CodeChain exited with code ${code}`));
        });

      const readline = createReadline({ input: this.process!.stderr });
      let flag = false;
      readline.on("line", (line: string) => {
        if (line.includes("Initialization complete")) {
          flag = true;
          resolve();
        }
        if (this.logFlag && flag) {
          appendFileSync(this.logPath, line + "\n");
        }
      });
    });
  }

  public async connect(peer: CodeChain) {
    if (!this.process) {
      return Promise.reject(Error("process isn't available"));
    }
    return this.sdk.rpc.network.connect("127.0.0.1", peer.port);
  }

  public async disconnect(peer: CodeChain) {
    if (!this.process) {
      return Promise.reject(Error("process isn't available"));
    }
    return this.sdk.rpc.network.disconnect("127.0.0.1", peer.port);
  }

  public async waitPeers(n: number) {
    while (n > await this.sdk.rpc.network.getPeerCount()) {
      wait(500);
    }
    return;
  }

  public async waitBlockNumberSync(peer: CodeChain) {
    while (await this.getBestBlockNumber() !== await peer.getBestBlockNumber()) {
      wait(500);
    }
  }

  public async getBestBlockNumber() {
    return this.sdk.rpc.chain.getBestBlockNumber();
  }

  public async getBestBlockHash() {
    return this.sdk.rpc.chain.getBlockHash(await this.getBestBlockNumber());
  }

  public async sendSignedParcel(options?: { nonce?: number, awaitInvoice?: boolean }): Promise<SignedParcel> {
    const { nonce = await this.sdk.rpc.chain.getNonce(faucetAddress) || 0, awaitInvoice = true } = options || {};
    const parcel = this.sdk.core.createPaymentParcel({
      recipient: "tccqruq09sfgax77nj4gukjcuq69uzeyv0jcs7vzngg",
      amount: 0,
    }).sign({
      secret: faucetSecret,
      fee: 10 + this.id,
      nonce,
    });
    const hash = await this.sdk.rpc.chain.sendSignedParcel(parcel);
    if (awaitInvoice) {
      await this.sdk.rpc.chain.getParcelInvoice(hash, { timeout: 300 * 1000 });
      return await this.sdk.rpc.chain.getParcel(hash) as SignedParcel;
    }
    return parcel;
  }

  public async clean() {
    return new Promise(resolve => {
      if (!this.process) {
        return resolve();
      }
      this.process.on("exit", resolve);
      this.process.kill();
      this.process = undefined;
    });
  }
}
