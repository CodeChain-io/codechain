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
// along with this program. If not, see <https://www.gnu.org/licenses/>.

import { H256, H512, U128 } from "codechain-primitives";
import { blake256WithKey } from "codechain-sdk/lib/utils";

const RLP = require("rlp");

export enum MessageType {
    SYNC1_ID = 0x01,
    SYNC2_ID = 0x02,
    ACK_ID = 0x03,
    NACK_ID = 0x04,
    REQUEST_ID = 0x05,
    RESPONSE_ID = 0x06,
    ENCRYPTED_ID = 0x07,
    UNENCRYPTED_ID = 0x08
}

export type Message = Sync1 | Sync2 | Ack | Nack | NegotiationRequest | NegotiationResponse | Encrypted | Unencrypted;

export class Sync1 {

    public static fromBytes(bytes: Buffer): Sync1 {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.SYNC1_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Sync1`);
        }

        const initiatorPubKey = new H512(decoded[1].toString("hex"));
        const networkId = decoded[2].toString();
        const initiatorPort = decoded[3].readUIntBE(0, 2);

        return new Sync1(initiatorPubKey, networkId, initiatorPort);
    }
    private readonly initiatorPort: number;
    private readonly initiatorPubKey: H512;
    private readonly networkId: string;

    constructor(initiatorPubKey: H512, networkId: string, initiatorPort: number) {
        this.initiatorPubKey = initiatorPubKey;
        this.networkId = networkId;
        this.initiatorPort = initiatorPort;
    }

    public protocolId(): MessageType {
        return MessageType.SYNC1_ID;
    }

    public toEncodeObject(): Array<any> {
        const { initiatorPubKey, networkId, initiatorPort } = this;
        return [this.protocolId(), initiatorPubKey.toEncodeObject(), networkId, initiatorPort];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class Sync2 {

    public static fromBytes(bytes: Buffer): Sync2 {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.SYNC2_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Sync2`);
        }

        const initiatorPubKey = new H512(decoded[1].toString("hex"));
        const recipientPubKey = new H512(decoded[2].toString("hex"));
        const networkId = decoded[3].toString();
        const initiatorPort = decoded[4].readUIntBE(0, 2);

        return new Sync2(initiatorPubKey, recipientPubKey, networkId, initiatorPort);
    }
    private readonly initiatorPort: number;
    private readonly initiatorPubKey: H512;
    private readonly networkId: string;
    private readonly recipientPubKey: H512;

    constructor(initiatorPubKey: H512, recipientPubKey: H512, networkId: string, initiatorPort: number) {
        this.initiatorPubKey = initiatorPubKey;
        this.recipientPubKey = recipientPubKey;
        this.networkId = networkId;
        this.initiatorPort = initiatorPort;
    }

    public protocolId(): MessageType {
        return MessageType.SYNC2_ID;
    }

    public toEncodeObject(): Array<any> {
        const { initiatorPubKey, recipientPubKey, networkId, initiatorPort } = this;
        return [
            this.protocolId(),
            initiatorPubKey.toEncodeObject(),
            recipientPubKey.toEncodeObject(),
            networkId,
            initiatorPort
        ];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class Ack {

    public static fromBytes(bytes: Buffer): Ack {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.ACK_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Ack`);
        }

        const recipientPubKey = new H512(decoded[1].toString("hex"));
        const encryptedNonce = decoded[2];

        return new Ack(recipientPubKey, encryptedNonce);
    }
    public readonly encryptedNonce: Buffer;
    public readonly recipientPubKey: H512;

    constructor(recipientPubKey: H512, encryptedNonce: Buffer) {
        this.recipientPubKey = recipientPubKey;
        this.encryptedNonce = encryptedNonce;
    }

    public protocolId(): MessageType {
        return MessageType.ACK_ID;
    }

    public toEncodeObject(): Array<any> {
        const { recipientPubKey, encryptedNonce } = this;
        return [this.protocolId(), recipientPubKey.toEncodeObject(), encryptedNonce];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class Nack {
    public static fromBytes(bytes: Buffer): Nack {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.NACK_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Ack`);
        }

        return new Nack();
    }

    public protocolId(): MessageType {
        return MessageType.NACK_ID;
    }

    public toEncodeObject(): Array<any> {
        return [this.protocolId()];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class NegotiationRequest {

    public static fromBytes(bytes: Buffer): NegotiationRequest {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.REQUEST_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for NegotiationRequest`);
        }

        const extensionName = decoded[1].toString();
        const extensionVersions = decoded[2];

        return new NegotiationRequest(extensionName, extensionVersions);
    }
    private readonly extensionName: string;
    private readonly extensionVersions: number[];

    constructor(extensionName: string, extensionVersions: number[]) {
        this.extensionName = extensionName;
        this.extensionVersions = extensionVersions;
    }

    public protocolId(): MessageType {
        return MessageType.REQUEST_ID;
    }

    public toEncodeObject(): Array<any> {
        const { extensionName, extensionVersions } = this;
        return [this.protocolId(), extensionName, extensionVersions];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class NegotiationResponse {

    public static fromBytes(bytes: Buffer): NegotiationResponse {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.RESPONSE_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for NegotiationResponse`);
        }

        const extensionName = decoded[1].toString();
        const extensionVersion = decoded[2];

        return new NegotiationResponse(extensionName, extensionVersion);
    }
    private readonly extensionName: string;
    private readonly extensionVersion: number;

    constructor(extensionName: string, extensionVersion: number) {
        this.extensionName = extensionName;
        this.extensionVersion = extensionVersion;
    }

    public protocolId(): MessageType {
        return MessageType.RESPONSE_ID;
    }

    public toEncodeObject(): Array<any> {
        const { extensionName, extensionVersion } = this;
        return [this.protocolId(), extensionName, extensionVersion];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class Encrypted {

    public static fromBytes(bytes: Buffer): Encrypted {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.ENCRYPTED_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Encrypted message`);
        }

        const extensionName = decoded[1].toString();
        const encrypted = decoded[2];

        return new Encrypted(extensionName, encrypted);
    }
    private readonly extensionName: string;
    private readonly encrypted: Buffer;

    constructor(extensionName: string, encrypted: Buffer) {
        this.extensionName = extensionName;
        this.encrypted = encrypted;
    }

    public protocolId(): MessageType {
        return MessageType.ENCRYPTED_ID;
    }

    public toEncodeObject(): Array<any> {
        const { extensionName, encrypted } = this;
        return [this.protocolId(), extensionName, encrypted];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export class Unencrypted {

    public static fromBytes(bytes: Buffer): Unencrypted {
        const decoded = RLP.decode(bytes);
        const protocolId = parseInt(decoded[0].toString("hex"), 16);
        if (protocolId !== MessageType.UNENCRYPTED_ID) {
            throw Error(`0x${decoded[0].toString("hex")} is not an expected protocol id for Unencrypted message`);
        }

        const extensionName = decoded[1].toString();
        const encrypted = decoded[2];

        return new Unencrypted(extensionName, encrypted);
    }
    public readonly extensionName: string;
    public readonly data: Buffer;

    constructor(extensionName: string, data: Buffer) {
        this.extensionName = extensionName;
        this.data = data;
    }

    public protocolId(): MessageType {
        return MessageType.UNENCRYPTED_ID;
    }

    public toEncodeObject(): Array<any> {
        const { extensionName, data } = this;
        return [this.protocolId(), extensionName, data];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }
}

export function fromBytes(bytes: Buffer): Message {
    const decoded = RLP.decode(bytes);
    const message = decoded[0];

    const protocolId = parseInt(message[0].toString(16), 16);
    switch (protocolId) {
        case MessageType.SYNC1_ID: {
            return Sync1.fromBytes(bytes);
        }
        case MessageType.SYNC2_ID: {
            return Sync2.fromBytes(bytes);
        }
        case MessageType.ACK_ID: {
            return Ack.fromBytes(bytes);
        }
        case MessageType.NACK_ID: {
            return Nack.fromBytes(bytes);
        }
        case MessageType.REQUEST_ID: {
            return NegotiationRequest.fromBytes(bytes);
        }
        case MessageType.RESPONSE_ID: {
            return NegotiationResponse.fromBytes(bytes);
        }
        case MessageType.ENCRYPTED_ID: {
            return Encrypted.fromBytes(bytes);
        }
        case MessageType.UNENCRYPTED_ID: {
            return Unencrypted.fromBytes(bytes);
        }
        default: {
            throw Error(`0x${message[0].toString("hex")} is not a valid protocol id`);
        }
    }
}

export class SignedMessage {

    public static fromBytes(bytes: Buffer, nonce: U128): SignedMessage {
        const decoded = RLP.decode(bytes);
        const message = fromBytes(decoded[0]);
        const signed = new SignedMessage(message, nonce);
        if (!signed.signature().isEqualTo(new H256(decoded[1].toString("hex")))) {
            throw Error(`${nonce} is not a valid nonce for this signed message`);
        }
        return signed;
    }
    public readonly message: Message;
    private readonly _signature: H256;

    constructor(message: Message, nonce: U128) {
        this.message = message;
        const bytes = this.message.rlpBytes();
        const key = new Uint8Array([...Buffer.from(nonce.toString(16).padStart(32, "0"), "hex")]);
        this._signature = new H256(blake256WithKey(bytes, key));
    }

    public protocolId(): MessageType {
        return this.message.protocolId();
    }

    public toEncodeObject(): Array<any> {
        return [`0x${this.message.rlpBytes().toString("hex")}`, this._signature.toEncodeObject()];
    }

    public rlpBytes(): Buffer {
        return RLP.encode(this.toEncodeObject());
    }

    public signature(): H256 {
        return this._signature;
    }
}
