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
import { mkdtempSync } from "fs";
import { createInterface as createReadline } from "readline";
import * as mkdirp from "mkdirp";

const projectRoot = `${__dirname}/../../..`;
let idCounter = 0;

export default class CodeChain {
  private _id: number;
  private _sdk: SDK;
  private _dbPath: string;
  private process?: ChildProcess;

  public get id(): number { return this._id; }
  public get sdk(): SDK { return this._sdk; }
  public get dbPath(): string { return this._dbPath; }
  public get rpcPort(): number { return 8080 + this.id; }
  public get port(): number { return 3484 + this.id; }
  public get secretKey(): number { return 1 + this.id; }

  constructor() {
    this._id = idCounter;
    idCounter += 1;

    mkdirp.sync(`${projectRoot}/db/`);
    this._dbPath = mkdtempSync(`${projectRoot}/db/`);
    this._sdk = new SDK({ server: `http://localhost:${this.rpcPort}` });
  }

  public async start(argv: string[], options?: { useDebugBuild?: boolean }) {
    const params = [
      "--db-path", this.dbPath,
      "--jsonrpc-port", this.rpcPort.toString(),
      "--port", this.port.toString(),
      "--instance-id", this.id.toString(),
    ];
    const { useDebugBuild = false } = options || {};

    // Resolves when CodeChain initialization completed.
    return new Promise((resolve, reject) => {
      this.process = spawn(
        `target/${useDebugBuild ? "debug" : "release"}/codechain`,
        [...argv, ...params],
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
      readline.on("line", (line: string) => {
        if (line.includes("Initialization complete")) {
          readline.close();
          resolve();
        }
      });
    });
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
