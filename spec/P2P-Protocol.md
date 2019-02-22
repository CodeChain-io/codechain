CodeChain P2P Protocol works on TCP (Transmission Control Protocol). There are three kinds of messages; `Handshake`, `Negotiation` and `Extension`.

`Negotiation` and `Extension` messages have a checksum on the tail. This is the BLAKE2b hash of `Body` with a nonce.

# Handshaking
Checking whether two nodes agree on the same key and sharing the nonce is the purpose of the handshaking process.

The initiator of the P2P protocol connection must send a sync message.
There are two kinds of sync messages.
One has the public key of the recipient and the other one doesn't.

```
Sync1 := 0x01 . initiator-pub-key . network-id . initiator-port
Sync2 := 0x02 . initiator-pub-key . recipient-pub-key . network-id . initiator-port
Ack := 0x03 . recipient-pub-key . encrypt(nonce, secret-key)
Nack := 0x04
```

The `Nack` message is introduced to ensure there is only one node between two nodes.
The recipient must not send a `Nack` when the decryption of the nonce has failed.
The recipient should give a `Nack` if it had requested a connection to the initiator.
The initiator should retry after a few random seconds. The range of retry time should be `[0, T1)`.

The initiator must close the connection for situations described below:
1. It didn't send a `Sync`, but the recipient sends an `Ack`.
2. It receives an `Ack`, but the nonce cannot be decrypted.
3. It sent a `Sync`, but there is neither `Ack` nor `Nack` during timeout(`T2`).

The recipient must close the connection for situations described below:
1. The network id received is not the same as the recipent's.
2. It already knows the public key of the initiator, but the key received is different from that.
3. It received Sync2, but the recipient-pub-key is unfamiliar.
4. If there is a timeout(`T3`) without a sync message.

* `T2` must be larger than the RTT.
* `T3` must be larger than `T1` + `T2`.

## FSM
### Initiator
```
/----->[ Connected ](sync from recipient)--\
|        (timeout)                         |
|         | send Sync                 [ Closed ]
|         v                                |
\--(nack)[ Sent ](cannot decrypt nonce)----/
          (ack)   (T2)--------------------/
           |
           |send Negotiation
           v
     [ Established ]
```

### Recipient
```
/-------------------->[ Accepted ] (T3) -----------------> [Closed]
|                        (Sync)                                  ^
|                          |                                     |
|send Nack                 v                                     |
\-(sync already sent)[ Received ](invalid key/network id)--------/
                           | (sync is not sent)
                           | send Ack
                           v
                     [ Established ]
```

# Negotiating
The purpose of a negotiation is to check which extensions are contained by a node.
The initiator must send the negotiation messages right after it receives the `Ack` message.
The recipient should respond to the latest version of the extension that both nodes can use.

```
Message := (Body) . sign(nonce, Body)
Body := 0x05 . extension-name . extension-versions
    | 0x06 . extension-name . extension-version
```

The responder should check the signature and it must close the connection if the message doesn't have a valid signature.

# Extension message
Extension messages can be sent after the negotiation is finished.
Extension messages that are not approved by the negotiation must be rejected.

For authentication, all messages have a BLAKE2b signature with the shared nonce like negotiation messages.
Application messages can be optionally encrypted.
An encrypted message provides more secrecy than an unencrypted one by encrypting the whole body.
Each application decides whether to use encryption or not.

# Extension Message Layout

```
Message := (Body) . sign(nonce, Body)
Body := 0x07 . extension-name . encrypted-data
    | 0x08 . extension-name . unencrypted-data

encrypted-data = aes_encrypt(unencrypted-data, shared-secret, shared-nonce)
```
