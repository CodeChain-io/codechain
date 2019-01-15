Although node discovery is an indispensable part of a p2p network, the node discovery protocol is not a part of CodeChain's base protocol. Instead of defining it as a base protocol, CodeChain defines it as an extension to make it easily replaceable. This kind of extension is called a **discovery protocol**.

It is also possible to run without node discovery protocol. In this case, the server only tries to connect to fixed servers.

# Kademlia-discovery

Kademlia-discovery is one of the discovery protocols that CodeChain provides. This is a subset of the kademlia DHT protocol.

## Node Identification

The kademlia-discovery protocol uses 256-bits to distinguish a node. This 256-bit identification is called `NodeId`. CodeChain uses the BLAKE2b hash of the IP address to make them uniformly distributed and prevent a [Sybil attack](https://en.wikipedia.org/wiki/Sybil_attack).

## Xor Distance

The kademlia-discovery protocol uses xor distance. Xor distance is symmetric. In other words, the distance from node A to node B is the same as the distance from node B to node A. This property is important because when node A is one of the closest nodes of node B, node B is also probably one of the closest nodes to node A. It reduces the load for managing networks.

## Message

Because CodeChain doesn’t need features related to distributed storage, kademlia-discovery does not have `STORAGE` and `FIND_VALUE` messages. In addition, there is no need to check the heartbeat since CodeChain uses TCP. Thus, CodeChain has only `FIND_NODE` and `NODE` message.

Every request has a message id. The corresponding response must epoch this id. The message id should not be reused until the response is received or the session is closed.
