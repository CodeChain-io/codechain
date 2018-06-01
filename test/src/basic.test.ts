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

import { H160, H256, Parcel, PaymentTransaction, U256, privateKeyToAddress } from "codechain-sdk";
import { waitFor } from "./helper/promise";
import CodeChain from "./helper/spawn";

const secret = new H256("ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd");
const address = new H160(privateKeyToAddress(secret.value));

const instance = new CodeChain();

function payment(nonce: U256): Parcel {
  const networkId = 17;
  const transaction = new PaymentTransaction({
    nonce: nonce.increase(),
    sender: address,
    receiver: new H160("3f4aa1fedf1f54eeb03b759deadb36676b184911"),
    value: new U256("0"),
  });
  return new Parcel(nonce, new U256(10), networkId, transaction);
}

instance.start("cargo", ["run", "--", "-c", "solo"])
.then(() => instance.sdk.getNonce(address))
.then((nonce) => {
  return instance.sdk.sendSignedParcel(payment(nonce!).sign(secret));
})
.then(waitFor(3000))
.then((hash) => instance.sdk.getParcelInvoices(hash))
.then((invoice) => {
  if (invoice) {
    console.log(invoice);
  } else {
    console.log("no invoice");
  }
})
.then(() => instance.clean());
