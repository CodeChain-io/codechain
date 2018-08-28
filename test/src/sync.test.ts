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

import { SDK } from "codechain-sdk";
import { PlatformAddress } from "codechain-sdk/lib/key/PlatformAddress";

import { wait } from "./helper/promise";
import CodeChain from "./helper/spawn";

describe("2 nodes", () => {
  const secret = "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
  const address = PlatformAddress.fromAccountId(SDK.util.getAccountIdFromPrivate(secret));

  let nodeA: CodeChain;
  let nodeB: CodeChain;

  beforeEach(async () => {
    nodeA = new CodeChain();
    nodeB = new CodeChain();

    await nodeA.start();
    await nodeB.start();
  });

  describe("A-B connected", () => {
    beforeEach(async () => {
      await nodeA.sdk.rpc.network.connect("127.0.0.1", nodeB.port);
      await wait(250);
    });

    test("It should be synced when nodeA created a block", async () => {
      expect(await nodeA.sdk.rpc.network.isConnected("127.0.0.1", nodeB.port)).toBe(true);
      const parcel = await nodeA.sendSignedParcel({ awaitInvoice: true });
      await wait(2000);
      expect(await nodeB.getBestBlockHash()).toEqual(parcel.blockHash);
    });

    describe("A-B diverged", () => {
      beforeEach(async () => {
        await nodeA.sendSignedParcel();
        await nodeB.sendSignedParcel();
        expect(await nodeA.getBestBlockNumber()).toEqual(await nodeB.getBestBlockNumber());
        expect(await nodeA.getBestBlockHash()).not.toEqual(await nodeB.getBestBlockHash());
      });

      // FIXME: It fails on Travis.
      test.skip("It should be synced when nodeA becomes ahead", async () => {
        await nodeA.sendSignedParcel();
        await wait(4000);
        expect(await nodeA.getBestBlockHash()).toEqual(await nodeB.getBestBlockHash());
      });
    });
  });

  describe("nodeA becomes ahead", () => {
    beforeEach(async () => {
      await nodeA.sendSignedParcel();
    });

    test("It should be synced when A-B connected", async () => {
      await nodeA.connect(nodeB);
      await wait(2000);
      expect(await nodeA.getBestBlockHash()).toEqual(await nodeB.getBestBlockHash());
    });
  });

  describe("A-B diverged", () => {
    beforeEach(async () => {
      await nodeA.sendSignedParcel();
      await nodeB.sendSignedParcel();
      expect(await nodeA.getBestBlockNumber()).toEqual(await nodeB.getBestBlockNumber());
      expect(await nodeA.getBestBlockHash()).not.toEqual(await nodeB.getBestBlockHash());
    });

    describe("nodeA becomes ahead", () => {
      beforeEach(async () => {
        await nodeA.sendSignedParcel();
        expect(await nodeA.getBestBlockNumber()).toEqual(await nodeB.getBestBlockNumber() + 1);
      });

      test("It should be synced when A-B connected", async () => {
        await nodeA.connect(nodeB);
        await wait(2000);
        expect(await nodeA.getBestBlockHash()).toEqual(await nodeB.getBestBlockHash());
      });
    });
  });

  afterEach(async () => {
    await nodeA.clean();
    await nodeB.clean();
  });
});
