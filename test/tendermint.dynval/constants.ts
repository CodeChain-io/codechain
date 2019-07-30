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

import { SDK } from "codechain-sdk";
import { Signer } from "../src/helper/spawn";

const privateKeys = [
  "aa42ea65225bbd39dd0fdb80b855c936d256f33cfcf3ead9146e43c476334cd5", // tccqy0mn5x8y3shes2ncmsjna94nuffrt9msqz27ez6
  "a9c427f7bebac9cf70e517c861e8ce7b6176de592b86a8294488dce6c2e5c13e", // tccq8g96vn3tagkf4hrdzgf6l9nqded4l5j7c5qulst
  "e62853a91b5e444d6e00d9259fbef17da1e631ff36e02e46c746a497c4ea6db0", // tccq83wm6sjyklkd4utk6hjmewsaccgvzk5sck8cs2y
  "63ce52a2b18214d442ba633ba449e72230488ea7820a543b05a72aca6d2d28ac", // tccq9e7k4nm2m3kxls3vqyxh9aast0ufys4ss4mk8lg
  "8b558081ed35cb9ba1ba7d2c6204252b4c86955da3ea52805f16771a0059a9e4", // tccq80vewuacz704whqpcr9e5kjfmtlmpr5xggw66ty
  "bebd1eb30d5f08b26d04d317a240eafb18511d83088fb949db3e8b194a578985", // tccqy5qjlvnv4jplzpkhvxe7pvdv2spmczfvyr7e0yk
  "ac8bba4801210dc01137366e1a321b1e052fb2561cde927584361d6da6eb7549", // tccq9jj73ft3s4taqksv7fxy0qkhy978c9cqydsxy5y
  "e672ee34dd9eff2b62d29f3e4a41400682338fcd5a5007a08781a16063e2486d", // tccq8ad6wxwhsk4aryazef5nawgw98zu6xpe5njs609
  "c8a9b84235155e79934741a3227b77fa70f4571e173c4fdc597e6a51e7c03c81", // tccqx8ltnh22s5a0xdfxf8j9zsg0c6ult03gvc6hcxy
  "01153af55ce89f8ee107d5a0e103f73f6354810142e33ea60753288d57833b4a" // tccq8en43nfkkpjxn534gccpqejzhmx75lx2sxkyj6u
];

export const validators: Signer[] = privateKeys.map(privateKey => {
  const publicKey = SDK.util.getPublicFromPrivate(privateKey);
  const accountId = SDK.util.getAccountIdFromPrivate(privateKey);
  const platformAddress = SDK.Core.classes.PlatformAddress.fromPublic(
    publicKey,
    {
      networkId: "tc"
    }
  );
  return { privateKey, publicKey, accountId, platformAddress };
});
