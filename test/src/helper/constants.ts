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

export const faucetSecret =
    "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
export const faucetAccointId = SDK.util.getAccountIdFromPrivate(faucetSecret); // 6fe64ffa3a46c074226457c90ccb32dc06ccced1
export const faucetAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    faucetAccointId
); // tccq9h7vnl68frvqapzv3tujrxtxtwqdnxw6yamrrgd

export const aliceSecret =
    "4aa026c5fecb70923a1ee2bb10bbfadb63d228f39c39fe1da2b1dee63364aff1";
export const aliceAccountId = SDK.util.getAccountIdFromPrivate(aliceSecret); // 40c1f3a9da4acca257b7de3e7276705edaff074a
export const aliceAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    aliceAccountId
); // tccq9qvruafmf9vegjhkl0ruunkwp0d4lc8fgxknzh5

export const bobSecret =
    "91580d24073185b91904514c23663b1180090cbeefc24b3d2e2ab1ba229e2620";
export const bobAccountId = SDK.util.getAccountIdFromPrivate(bobSecret); // e1361974625cbbcbbe178e77b510d44d59c9ca9d
export const bobAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    bobAccountId
); // tccq8snvxt5vfwthja7z7880dgs63x4njw2n5e5zm4h

export const invalidSecret =
    "0000000000000000000000000000000000000000000000000000000000000000";
export const invalidAddress = "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqhhn9p3";
