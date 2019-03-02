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
import { H160 } from "codechain-primitives";
import { H256 } from "codechain-primitives";
import { U256 } from "codechain-primitives";
import { SignedTransaction } from "codechain-sdk/lib/core/SignedTransaction";
import { BlockSyncMessage, Emitter, IBodiesq, IHeadersq, MessageType, ResponseMessage } from "./blockSyncMessage";
import { Header } from "./cHeader";
import { P2pLayer } from "./p2pLayer";
import { TransactionSyncMessage } from "./transactionSyncMessage";

type EncodedHeaders = Array<Array<Buffer>>;
type EncodedTransactions = Array<Array<Buffer>>;
type EncodedBodies = Array<Array<Array<Buffer>>>;

export class Mock {

    get genesisHash() {
        return this.p2psocket.getGenesisHash();
    }
    private p2psocket: P2pLayer;
    private log: boolean;

    constructor(ip: string, port: number, networkId: string) {
        this.p2psocket = new P2pLayer(ip, port, networkId);
        this.log = false;
    }

    public setLog() {
        this.log = true;
        this.p2psocket.enableLog();
    }

    public async establish(bestHash?: H256, bestScore?: U256) {
        await this.p2psocket.connect();

        let isStatusArrived;
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const responseBody = msg.getBody();
            if (responseBody.type === "status") {
                isStatusArrived = true;
            }
        }
        if (!isStatusArrived) { await this.waitStatusMessage(); }

        const score = bestScore == undefined ? new U256("99999999999999999") : bestScore;
        const best =
            bestHash == undefined
                ? new H256("0x649fb35c0e304eb601ae71fe330729a2c1a27687ae7e2b0170866b86047a7bb9")
                : bestHash;
        const genesis = this.p2psocket.getGenesisHash();
        this.sendStatus(score, best, genesis);

        await this.waitHeaderRequest();

        if (this.log) { console.log("Connected\n"); }
    }

    public async end() {
        await this.p2psocket.close();
    }

    // Get block headers from the most recent header response
    public getBlockHeaderResponse(): EncodedHeaders | null {
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const responseBody = msg.getBody();
            if (responseBody.type === "response") {
                const responseMsgBody = responseBody.message.getBody();
                if (responseMsgBody.type === "headers") {
                    return responseMsgBody.data;
                }
            }
        }
        return null;
    }

    // Get block bodies from the most recent body response
    public getBlockBodyResponse(): EncodedBodies | null {
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const responseBody = msg.getBody();
            if (responseBody.type === "response") {
                const responseMsgBody = responseBody.message.getBody();
                if (responseMsgBody.type === "bodies") {
                    return responseMsgBody.data;
                }
            }
        }
        return null;
    }

    // Get the most recent transaction sync message from the node
    public getTransactionSyncMessage(): EncodedHeaders | null {
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const requestBody = msg.getBody();
            if (requestBody.type === "transactions") {
                return requestBody.data;
            }
        }
        return null;
    }

    // Get the most recent block header request from the node
    public getBlockHeaderRequest(): IHeadersq | null {
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const requestBody = msg.getBody();
            if (requestBody.type === "request") {
                const requestMsgBody = requestBody.message.getBody();
                if (requestMsgBody.type === "headers") {
                    return requestMsgBody;
                }
            }
        }
        return null;
    }

    // Get the most recent block body request from the node
    public getBlockBodyRequest(): IBodiesq | null {
        for (const msg of this.p2psocket.getArrivedExtensionMessage().reverse()) {
            const requestBody = msg.getBody();
            if (requestBody.type === "request") {
                const requestMsgBody = requestBody.message.getBody();
                if (requestMsgBody.type === "bodies") {
                    return requestMsgBody;
                }
            }
        }
        return null;
    }

    public async sendStatus(score: U256, bestHash: H256, genesisHash: H256) {
        const msg = new BlockSyncMessage({
            type: "status",
            totalScore: score,
            bestHash,
            genesisHash
        });
        await this.p2psocket.sendExtensionMessage("block-propagation", msg.rlpBytes(), false);
    }

    public async sendBlockHeaderResponse(headers: EncodedHeaders) {
        const message = new ResponseMessage({ type: "headers", data: headers });
        const msg = new BlockSyncMessage({
            type: "response",
            id: this.p2psocket.getHeaderNonce(),
            message
        });
        await this.p2psocket.sendExtensionMessage("block-propagation", msg.rlpBytes(), false);
    }

    public async sendBlockBodyResponse(bodies: EncodedBodies) {
        const message = new ResponseMessage({ type: "bodies", data: bodies });
        const msg = new BlockSyncMessage({
            type: "response",
            id: this.p2psocket.getBodyNonce(),
            message
        });
        await this.p2psocket.sendExtensionMessage("block-propagation", msg.rlpBytes(), false);
    }

    public async sendTransactionSyncMessage(transactions: EncodedTransactions) {
        const message = new TransactionSyncMessage({
            type: "transactions",
            data: transactions
        });
        await this.p2psocket.sendExtensionMessage("transaction-propagation", message.rlpBytes(), false);
    }

    public async sendEncodedBlock(header: EncodedHeaders, body: EncodedBodies, bestBlockHash: H256, bestBlockScore: U256) {
        if (this.log) { console.log("Send blocks"); }
        const score = bestBlockScore;
        const best = bestBlockHash;
        const genesis = this.p2psocket.getGenesisHash();
        await this.sendStatus(score, best, genesis);

        await this.sendBlockHeaderResponse(header);
        if (this.log) { console.log("Send header response"); }

        await this.waitBodyRequest();
        await this.sendBlockBodyResponse(body);
        if (this.log) { console.log("Send body response"); }
    }

    public async sendBlock(header: Array<Header>, body: Array<Array<SignedTransaction>>) {
        if (this.log) { console.log("Send blocks"); }
        const bestBlock = header[header.length - 1];
        const score = bestBlock.getScore();
        const best = bestBlock.hashing();
        const genesis = this.p2psocket.getGenesisHash();
        await this.sendStatus(score, best, genesis);

        await this.sendBlockHeaderResponse(header.map(h => h.toEncodeObject()));
        if (this.log) { console.log("Send header response"); }

        await this.waitBodyRequest();
        await this.sendBlockBodyResponse(body.map(transactions => transactions.map(tx => tx.toEncodeObject())));
        if (this.log) { console.log("Send body response"); }
    }

    public async sendEncodedTransaction(transactions: EncodedTransactions) {
        if (this.log) { console.log("Send transactions"); }
        await this.sendTransactionSyncMessage(transactions);
    }

    public async sendTransaction(transactions: Array<SignedTransaction>) {
        if (this.log) { console.log("Send transactions"); }
        await this.sendTransactionSyncMessage(transactions.map(tx => tx.toEncodeObject()));
    }

    public async waitStatusMessage() {
        try {
            await this.waitForBlockSyncMessage(MessageType.MESSAGE_ID_STATUS);
        } catch (error) {
            console.error(error);
        }
    }

    public async waitHeaderRequest() {
        try {
            await this.waitForBlockSyncMessage(MessageType.MESSAGE_ID_GET_HEADERS);
        } catch (error) {
            console.error(error);
        }
    }

    public async waitBodyRequest() {
        try {
            await this.waitForBlockSyncMessage(MessageType.MESSAGE_ID_GET_BODIES);
        } catch (error) {
            console.error(error);
        }
    }

    public async waitHeaderResponse() {
        try {
            await this.waitForBlockSyncMessage(MessageType.MESSAGE_ID_HEADERS);
        } catch (error) {
            console.error(error);
        }
    }

    public async waitBodyResponse() {
        try {
            await this.waitForBlockSyncMessage(MessageType.MESSAGE_ID_BODIES);
        } catch (error) {
            console.error(error);
        }
    }

    public soloGenesisBlockHeader(): Header {
        const parentHash = new H256("0000000000000000000000000000000000000000000000000000000000000000");
        const timestamp = new U256(0);
        const number = new U256(0);
        const author = new H160("0000000000000000000000000000000000000000");
        const extraData = Buffer.from([
            23,
            108,
            91,
            111,
            253,
            100,
            40,
            143,
            87,
            206,
            189,
            160,
            126,
            135,
            186,
            91,
            4,
            70,
            5,
            195,
            246,
            153,
            51,
            67,
            233,
            113,
            143,
            161,
            0,
            209,
            115,
            124
        ]);
        const transactionsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const stateRoot = new H256("09f943122bfbb85adda8209ba72514374f71826fd874e08855b64bc95498cb02");
        const resultsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const score = new U256(131072);
        const seal: any[] = [];
        const header = new Header(
            parentHash,
            timestamp,
            number,
            author,
            extraData,
            transactionsRoot,
            stateRoot,
            resultsRoot,
            score,
            seal
        );

        return header;
    }

    public soloBlock1(parent: H256): Header {
        const parentHash = parent;
        const timestamp = new U256(1537509963);
        const number = new U256(1);
        const author = new H160("7777777777777777777777777777777777777777");
        const extraData = Buffer.alloc(0);
        const transactionsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const stateRoot = new H256("09f943122bfbb85adda8209ba72514374f71826fd874e08855b64bc95498cb02");
        const resultsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const score = new U256(999999999999999);
        const seal: any[] = [];
        const header = new Header(
            parentHash,
            timestamp,
            number,
            author,
            extraData,
            transactionsRoot,
            stateRoot,
            resultsRoot,
            score,
            seal
        );

        return header;
    }

    public soloBlock2(parent: H256): Header {
        const parentHash = parent;
        const timestamp = new U256(1537944287);
        const number = new U256(2);
        const author = new H160("6666666666666666666666666666666666666666");
        const extraData = Buffer.alloc(0);
        const transactionsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const stateRoot = new H256("09f943122bfbb85adda8209ba72514374f71826fd874e08855b64bc95498cb02");
        const resultsRoot = new H256("45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0");
        const score = new U256(999999999999999);
        const seal: any[] = [];
        const header = new Header(
            parentHash,
            timestamp,
            number,
            author,
            extraData,
            transactionsRoot,
            stateRoot,
            resultsRoot,
            score,
            seal
        );

        return header;
    }

    private async waitForBlockSyncMessage(type: MessageType): Promise<{}> {
        return new Promise((resolve, reject) => {
            switch (type) {
                case MessageType.MESSAGE_ID_STATUS: {
                    Emitter.once("status", () => {
                        resolve();
                    });
                    break;
                }
                case MessageType.MESSAGE_ID_GET_HEADERS: {
                    Emitter.once("headerrequest", () => {
                        resolve();
                    });
                    break;
                }
                case MessageType.MESSAGE_ID_GET_BODIES: {
                    Emitter.once("bodyrequest", () => {
                        resolve();
                    });
                    break;
                }
                case MessageType.MESSAGE_ID_HEADERS: {
                    Emitter.once("headerresponse", () => {
                        resolve();
                    });
                    break;
                }
                case MessageType.MESSAGE_ID_BODIES: {
                    Emitter.once("headerresponse", () => {
                        resolve();
                    });
                    break;
                }
                default: {
                    console.error("Not implemented");
                    reject();
                }
            }
        });
    }
}
