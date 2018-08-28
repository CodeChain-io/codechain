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

import CodeChain from "../helper/spawn";

describe("solo - 1 node", () => {
  const secret = "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
  const address = PlatformAddress.fromAccountId(SDK.util.getAccountIdFromPrivate(secret));

  let node: CodeChain;
  beforeAll(async () => {
    node = new CodeChain();
    await node.start();
  });

  test("ping", async () => {
    expect(await node.sdk.rpc.node.ping()).toBe("pong");
  });

  test("getNodeVersion", async () => {
    expect(await node.sdk.rpc.node.getNodeVersion()).toBe("0.1.0");
  });

  test("getCommitHash", async () => {
    expect(await node.sdk.rpc.node.getCommitHash()).toMatch(/^[a-fA-F0-9]{40}$/);
  });

  test("sendSignedParcel", async () => {
    const parcel = node.sdk.core.createPaymentParcel({
      recipient: "tccqruq09sfgax77nj4gukjcuq69uzeyv0jcs7vzngg",
      amount: 0,
    });
    const nonce = await node.sdk.rpc.chain.getNonce(address);
    const parcelHash = await node.sdk.rpc.chain.sendSignedParcel(parcel.sign({
      secret, fee: 10, nonce
    }));
    const invoice = await node.sdk.rpc.chain.getParcelInvoice(parcelHash);
    expect(invoice).toEqual({ success: true });
  });

  afterAll(async () => {
    await node.clean();
  });
});
