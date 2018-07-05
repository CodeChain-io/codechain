CodeChain uses Merkle Trie to hold the states with authentication.
It's a [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree), so the nodes have [cryptographic hash](https://en.wikipedia.org/wiki/Cryptographic_hash_function) to check the integrity. The hash of the root is used to label the tree.

## Interface
### insert(key, value)
### get(key)
### contains(key)
### remove(key)

## Path
Codechain's Merkle Trie uses a fixed-size path. To guarantee the size of a path, Codechain's Merkle Trie doesn't use the key as the path, but uses the blake2b hash of the key as the path.
```
path = blake2b(key, outlen = K)
```
### Prefix of Path
All nodes have a partial path as the first item. The partial path is the path from the parent to the current node. Because of the Branch Node, the partial path is a set of nibbles. It means some partial path cannot be represented as bytes. This partial path with an odd number of nibbles is prefixed with `0b0001`. A partial path with an even number of nibbles is prefixed with `0b00000000`.

## Nodes
Merkle Trie has two types of nodes: Branch Node and Leaf Node.

### Branch Node
A Branch Node is a node that has children but doesn't have a value. In Codechain's Merkle Trie, there can be 16 children at most. So branch nodes are represented as a 17 element tuple.

The child that does not have any child node is null.

| path | child 0 | child 1 | ... | child 14 | child 15 |

### Leaf Node
A Leaf node is a node that doesn't have children but has value. It is represented as a 2-tuple in Codechain's Merkle Trie.

| path | value |

## Validity
CodeChain's Merkle Trie is valid when:
1. All of the prefix of partial paths are `0b00000000` or `0b0001`
1. All of the lengths of a full path to the leaf node is K.
1. There is no branch node does not have a child.
1. There is no branch node that has only one child.