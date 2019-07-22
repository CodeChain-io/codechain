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

import { expect } from "chai";
import * as _ from "lodash";
import "mocha";
import * as net from "net";
import CodeChain from "../helper/spawn";

describe("ipc", function() {
    let node: CodeChain;
    beforeEach(async function() {
        node = new CodeChain();
        await node.start({ disableIpc: false });
    });

    it("ping", async function() {
        const id = 1;
        const socket = await createConnection(id, node.ipcPath);
        sendPing(socket, id);
        await waitPong(socket, id);
    });

    it("multiple connections at the same time in the random order", async function() {
        const ids = _.shuffle([1, 2, 3, 4, 5]);

        const socket0 = await createConnection(ids[0], node.ipcPath);
        const socket1 = await createConnection(ids[1], node.ipcPath);
        const socket2 = await createConnection(ids[2], node.ipcPath);
        const socket3 = await createConnection(ids[3], node.ipcPath);
        const socket4 = await createConnection(ids[4], node.ipcPath);

        const sockets: [net.Socket, number][] = [
            [socket0, ids[0]],
            [socket1, ids[1]],
            [socket2, ids[2]],
            [socket3, ids[3]],
            [socket4, ids[4]]
        ];
        for (const [socket, id] of _.shuffle(sockets)) {
            sendPing(socket, id);
        }
        await Promise.all(
            _.shuffle(sockets).map(([socket, id]) => waitPong(socket, id))
        );
    });

    afterEach(async function() {
        if (this.currentTest!.state === "failed") {
            node.keepLogs();
        }
        await node.clean();
    });
});

function createConnection(id: number, ipcPath: string): Promise<net.Socket> {
    return new Promise((resolve, reject) => {
        const socket = net.createConnection(ipcPath).on("error", reject);
        socket.on("connect", () => resolve(socket));
    });
}

const jsonrpc = "2.0";

function sendPing(socket: net.Socket, id: number): void {
    const req = { jsonrpc, method: "ping", params: [], id };
    socket.write(JSON.stringify(req));
}

function waitPong(socket: net.Socket, id: number): Promise<void> {
    return new Promise((resolve, reject) => {
        socket.on("data", data => {
            const s = data.toString();
            let response: { id: number; jsonrpc: string; result: any };
            try {
                response = JSON.parse(s.toString());
            } catch (err) {
                reject(Error(s.toString()));
                return;
            }
            expect(response.id).be.equal(id);
            expect(response.jsonrpc).be.equal(jsonrpc);
            expect(response.result).be.equal("pong");
            resolve();
        });
    });
}
