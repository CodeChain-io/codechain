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

const projectRoot = `${__dirname}/../../..`;
let idCounter = 0;

function zeroPad(val: string, totalLength: number): string {
  const padded = `${"0".repeat(totalLength)}${val}`;
  return padded.substr(padded.length - 64);
}

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

    this._dbPath = mkdtempSync(`${projectRoot}/db/`);
    this._sdk = new SDK({ server: `http://localhost:${this.rpcPort}` });
  }

  public async start(bin: string, argv: string[]) {

    const params = [
      "--db-path", this.dbPath,
      "--jsonrpc-port", this.rpcPort.toString(),
      "--port", this.port.toString(),
      "--secret-key", zeroPad(this.secretKey.toString(), 64),
      "--instance-id", this.id.toString(),
    ];
    this.process = spawn(bin, [...argv, ...params], { cwd: projectRoot, env: process.env });

    // wait until codechain is initialized
    return new Promise((resolve, reject) => {
      const onError = (error: Error) => {
        this.process!.off("error", onError);
        reject(error);
      };
      this.process!.on("error", onError);

      const readline = createReadline({ input: this.process!.stderr });
      readline.on("line", (line: string) => {
        if (line.includes("Initialization complete")) {
          readline.close();
          this.process!.off("error", onError);
          resolve();
        }
      });
    });
  }

  public async clean() {
    if (this.process !== undefined) {
      // wait until process is killed
      await new Promise((resolve) => {
        this.process!.on("exit", () => resolve());
        this.process!.kill();
        this.process = undefined;
      });
    }
  }

  public async restart(bin: string, argv: string[]) {
    await this.clean();
    return this.start(bin, argv);
  }
}
