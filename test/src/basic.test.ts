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
import { wait } from "./helper/promise";
import CodeChain from "./helper/spawn";

const TIMEOUT = 10000;

const secret = "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
const address = SDK.util.getAccountIdFromPrivate(secret);

const instance = new CodeChain();

test("basic scenario", async () => {
  await instance.start("cargo", ["run", "--", "-c", "solo"]);
  const nonce = await instance.sdk.rpc.chain.getNonce(address);
  const parcel = await instance.sdk.core.createPaymentParcel({
    recipient: "3f4aa1fedf1f54eeb03b759deadb36676b184911",
    amount: 0,
  });
  const hash = await instance.sdk.rpc.chain.sendSignedParcel(parcel.sign({ secret, nonce, fee: 10 }));
  const invoice = await instance.sdk.rpc.chain.getParcelInvoice(hash);
  expect(invoice.success).toBe(true);
  await instance.clean();
}, TIMEOUT);
