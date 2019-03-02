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
// along with this program. If not, see <https://www.gnu.org/licenses/>.
const RLP = require("rlp");

type transactionSyncMessageBody = ITransactions;

interface ITransactions {
    type: "transactions";
    data: Array<Array<Buffer>>;
}

export class TransactionSyncMessage {

    public static fromBytes(bytes: Buffer): TransactionSyncMessage {
        const decodedmsg = RLP.decode(bytes);
        return new TransactionSyncMessage({
            type: "transactions",
            data: decodedmsg
        });
    }
    private body: transactionSyncMessageBody;

    constructor(body: transactionSyncMessageBody) {
        this.body = body;
    }

    public getBody(): transactionSyncMessageBody {
        return this.body;
    }

    public toEncodeObject(): Array<Array<any>> {
        return this.body.data;
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}
