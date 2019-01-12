CodeChain P2P Protocol works on TCP (Transmission Control Protocol). There are three kinds of messages; `Handshake`, `Negotiation` and `Extension`.

All messages have a signature on the tail. This is the BLAKE2b hash of `Head` and `Body` with a session-key.

The initiator of the P2P protocol connection must send a `Syn` message. The response of the `Syn` message is called an `Ack` message. The initiator and the recipient must check if the signature on the tail is correct. If the signature is invalid, the node must close the connection.

Extension messages can be sent after the negotiation is finished. Extension messages that are not approved by the negotiation must be rejected.

Application messages can be optionally encrypted. For authentication, all messages have a BLAKE2b signature with the shared key. An encrypted message provides more secrecy than an unencrypted one by encrypting the whole body. Each application decides whether to use encryption or not.

# Handshake Message Layout

## Syn

```
Message := (Body) . sign(session-key, Body)
Body := version . SynProtocolId . session-name

SynProtocolId := 0x00

sign := session-key -> bytes -> H256
BLAKE2b(session-key.session-name, bytes)[0..32]
```

## Ack

```
Message := (Body) . sign(session-key, Body)
Body := version . AckProtocolId
AckProtocolId := 0x01
```

# Negotiation Data Layout

```
Message := (Body) . sign(session-key, Body)
Body := version . RequestProtocolId . RequestBody
	| version . AllowedProtocolId . AllowedBody
	| version . DeniedProtocolId . DeniedBody

RequestProtocolId := 0x02
AllowedProtocolId := 0x03
DeniedProtocolId := 0x04
RequestBody := seq  . extension-name . extension-version
AllowedBody := seq
DeniedBody := seq . [ . version]*

extension-name := string
extension-version := u32
```

# Extension Message Layout

```
Message := (Body) . sign(session-key, Body)
Body := (version . EncryptedProtocolId . extension-name . extension-version)
. aes256(extension-layer, session-key)
	| (version . UnencryptedProtocolId . extension-name . extension-version)
. extension-layer

EncryptedProtocolId := 0x05
UnencryptedProtocolId := 0x06
```
