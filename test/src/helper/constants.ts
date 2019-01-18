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

import { SDK } from "codechain-sdk";

export const faucetSecret =
    "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
export const faucetAccointId = SDK.util.getAccountIdFromPrivate(faucetSecret); // 6fe64ffa3a46c074226457c90ccb32dc06ccced1
export const faucetAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    faucetAccointId,
    { networkId: "tc" }
); // tccq9h7vnl68frvqapzv3tujrxtxtwqdnxw6yamrrgd

export const aliceSecret =
    "4aa026c5fecb70923a1ee2bb10bbfadb63d228f39c39fe1da2b1dee63364aff1";
export const alicePublic = SDK.util.getPublicFromPrivate(aliceSecret);
// 2a8a69439f2396c9a328289fdc3905d9736da9e14eb1a282cfd2c036cc21a17a5d05595160b7924e5ecf3f2628b440e601f3a531e92fa81571a70e6c695b2d08
export const aliceAccountId = SDK.util.getAccountIdFromPrivate(aliceSecret); // 40c1f3a9da4acca257b7de3e7276705edaff074a
export const aliceAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    aliceAccountId,
    { networkId: "tc" }
); // tccq9qvruafmf9vegjhkl0ruunkwp0d4lc8fgxknzh5

export const bobSecret =
    "91580d24073185b91904514c23663b1180090cbeefc24b3d2e2ab1ba229e2620";
export const bobPublic = SDK.util.getPublicFromPrivate(bobSecret);
// 545ebdc0b8fb2d0be77a27d843945950db6dbddc60477c0cf001751a797df8a41fc51fe5b76e371c8875ad1d0585a60af2eef2b5d631f7bfba86e7988c25088d
export const bobAccountId = SDK.util.getAccountIdFromPrivate(bobSecret); // e1361974625cbbcbbe178e77b510d44d59c9ca9d
export const bobAddress = SDK.Core.classes.PlatformAddress.fromAccountId(
    bobAccountId,
    { networkId: "tc" }
); // tccq8snvxt5vfwthja7z7880dgs63x4njw2n5e5zm4h

export const invalidSecret =
    "0000000000000000000000000000000000000000000000000000000000000000";
export const invalidAddress = "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqhhn9p3";

export const validator0Secret =
    "b05b7c1e9747330e97676a95f55d3e469794dfa2aaa3c958d2d3eb334da9fb55";
export const validator0Public = SDK.util.getPublicFromPrivate(validator0Secret);
// 4f1541fc6bdec60bf0ac6380a8e3914a469fe6cd4fa817c890d5823cfdda83932f61dc083e1b6736dadeceb5afd3fcfbac915e5fa2c9c20acf1c30b080114d7f
export const validator0AccountId = SDK.util.getAccountIdFromPrivate(
    validator0Secret
); // 6a8e5ec34cdb3cde78ebf4dfd8d84f00f437fddb
export const validator0Address = SDK.Core.classes.PlatformAddress.fromAccountId(
    validator0AccountId,
    { networkId: "tc" }
); // tccq94guhkrfndnehnca06dlkxcfuq0gdlamvw9ga4f

export const validator1Secret =
    "79d26d5788ca5f5ae87e8dd0f057124c2cfda11136aeb140f1d9ac3648d5b703";
export const validator1Public = SDK.util.getPublicFromPrivate(validator1Secret);
// 1ac8248deb29a58c4bdbfce031fb22c7ba3bcc9384bf6de058a1c8bef5a17422cf8ca26666a5505684db7364eabeed6fc678b02658ae7c1848a4ae6e50244cf2
export const validator1AccountId = SDK.util.getAccountIdFromPrivate(
    validator1Secret
); // c25b8e91fccd3b8b137b5faa7f86f656252ba2ee
export const validator1Address = SDK.Core.classes.PlatformAddress.fromAccountId(
    validator1AccountId,
    { networkId: "tc" }
); // tccq8p9hr53lnxnhzcn0d065lux7etz22azaca786tt

export const validator2Secret =
    "83352d249f5fe8d85b792dd26d70050b2f7fab02be9ea33e52c83a2be73a2700";
export const validator2Public = SDK.util.getPublicFromPrivate(validator2Secret);
// db3a858d2bafd2cb5382fcf366b847a86b58b42ce1fc29fec0cb0315af881a2ad495045adbdbc86ef7a777b541c4e62a0747f25ff6068a5ec3a052c690c4ff8a
export const validator2AccountId = SDK.util.getAccountIdFromPrivate(
    validator2Secret
); // d32d7cd32af1703400c9624ea3ba488d7a0e6d17
export const validator2Address = SDK.Core.classes.PlatformAddress.fromAccountId(
    validator2AccountId,
    { networkId: "tc" }
); // tccq8fj6lxn9tchqdqqe93yaga6fzxh5rndzu8k2gdw

export const validator3Secret =
    "0afa81c02fba3671ec9578f3be040e0186b445e9dc37d8bf4a866c8636841836";
export const validator3Public = SDK.util.getPublicFromPrivate(validator3Secret);
// 42829b18de338aa3abf5e6d80cd511121bf9d34be9a135bbace32a3226479e7f3bb6af76c11dcc724a1666a22910d756b075d54d8fdd97be11efd7a0ac3bb222
export const validator3AccountId = SDK.util.getAccountIdFromPrivate(
    validator3Secret
); // 49acbedaea4afa1c00adea94856536fab532d927
export const validator3Address = SDK.Core.classes.PlatformAddress.fromAccountId(
    validator3AccountId,
    { networkId: "tc" }
); // tccq9y6e0k6af9058qq4h4ffpt9xmat2vkeyue23j8y

export const hitActionHandlerId = 1;
export const stakeActionHandlerId = 2;
