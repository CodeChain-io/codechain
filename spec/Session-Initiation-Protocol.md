Session Initiation Protocol is used to create a session required to create an P2P Protocol. Messages in this protocol are classified into three categories; `Request`, `Allow` and `Deny`. A node which sends `Request` and waits for `Allow` or `Deny` is called initiator, and a node which receives `Allow` or `Deny` is called recipient.

Session Initiation protocol works on UDP (User Datagram Protocol) to make a recipient respond quickly without bookkeeping connection information.

Messages of Session Initiation protocol have `seq` field. The initiator must assign a unique number to `seq`, and the recipient repeats the request `seq` in response. The `seq` must be increased monotonically when messages are sent between the same initiator and recipient.

An initiator must not resend the message immediately while waiting for a response timeout. It is possible that the network is congested or the recipient is busy processing other messages. In both cases, resending the message will end up making the situation worse.

A node should remove information if the request fails several times. The node might be dead or its address might have changed.

`Deny` messages must not disclose the internal states of the node. The denied reason must be general and abstract to avoid leaking sensitive information related to security.

ECDH messages generate a shared-secret between arbitrary nodes. Both initiator and recipient must generate a random key pair using secp256k1. Due to the limitation of ECDH, it is still vulnerable to a man-in-the-middle-attack.

Connection messages are used to share a session-key when a shared-secret already exists between nodes. A session-key is a pair of shared-secret and session-name. A session-name, a 128 bits random string, will be used to generate initialization vector of AES256.

The nonce must be used only once to prevent a replay attack.

# Message Layout

```
Message := version . seq . Body

Body := ConnectionRequestId . ConnectionRequest
	| ConnectionAllowedId . ConnectionAllowed
	| ConnectionDeniedId . reason
        | EcdhRequestId . ECDHRequest
	| EcdhAllowedId . ECDHAllowed
	| EcdhDeniedId . reason

ConnectionRequestId = 0x01
ConnectionAllowedId = 0x02
ConnectionDeniedId = 0x03
EcdhRequestId = 0x04
EcdhAllowedId = 0x05
EcdhDeniedId = 0x06

ConnectionRequest := aes256(rlp(temporary-session-name), (shared-secret * ZERO))
ConnectionAllowed := aes256(rlp(session-name), (shared-secret * temporary-session-name))

ECDHRequest := ephemeral-public-key-of-initiator
ECDHAllowed := ephemeral-public-key-of-recipient

version = u64
seq = u64
reason = string
temporary-session-name = H128
session-name = H128
shared-secret = H256
public-key = H512
reason = string
session-key = shared-secret * session-name

aes256 = bytes -> session-key -> bytes
encrypt = bytes -> public-key -> bytes
decrypt = bytes -> private-key -> bytes
```
