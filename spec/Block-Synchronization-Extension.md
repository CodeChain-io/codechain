* Name := “block-sync”
* Version := 0
* Encrypt := never

# Messages

```
Message :=
  <-> (message_id . special_message)
| <-  (message_id . request_id . request_content)
| ->  (message_id . response_id . response_content)
```

* Every message has `message_id`, which is message type identifier. Identifier of each message can be found in message description.
* Every request and response message has request/response id. Response for certain message MUST have same id as request.

## Special messages

### Status

```
Status(nonce, best_hash, genesis_hash)
```

Send current chain status to peer.

* Identifier: 0x01
* Restriction:
  * `nonce` SHOULD be monotonically increasing every time the message is sent.

## Request messages

### GetHeaders

```
GetHeaders(start_number, max_count)
```

Request at most `max_count` headers, starting from `start_number`.

* Identifier: 0x02
* Restriction: None


### GetBodies

```
GetBodies(hash_0, …)
```

Request corresponding bodies for each hash.

* Identifier: 0x04
* Restriction:
  * MUST include at least one item

### GetStateChunk

```
GetStateChunk(block_hash, [...chunk_roots])
```

Request corresponding snapshot chunk for each `chunk_root`.

* Identifier: 0x0a
* Restriction:
  * All values in `[...chunk_roots]` MUST be included in requested block’s state trie.


## Response messages

### Headers

```
Headers(header_0, …)
```

Response to `GetHeaders` message. This response MAY contain less number of content than requested if sender has no corresponding items.

* Identifier: 0x03
* Restriction:
  * Headers SHOULD be sorted by block number in ascending order.
  * Headers included in message MUST be continuous. I.e. All parent of headers except MUST exist in message except first one.
  * Lowest block number in the list MUST be equal to `start_number` in request.


### Bodies

```
Bodies(body_0, …)
```

Response to `GetBodies` message. Snappy algorithm is used to compress content.

* Identifier: 0x05
* Restriction:
  * Number and order of bodies included in this message MUST be equal to request information.
  * If sender doesn’t have body for requested hash, corresponding body value MUST be [], not omitted.
  * If received body is zero-length array, it means either body value is [], or sender doesn’t have body for requested hash


### StateChunk
```
StateChunk([compressed([terminal_0, …] | []), ...])
```

Response to `GetStateChunk` message. Snappy algorithm is used for compression of content.

* Identifier: 0x0b
* Restriction:
  * Number and order of chunks included in this message MUST be equal to request information.
  * Node corresponding to `chunk_root` in request MUST be included
  * If sender doesn’t have a chunk for the requested hash, corresponding chunk MUST be compressed([]), not omitted.
