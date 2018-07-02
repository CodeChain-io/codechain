CodeChain Virtual Machine (CCVM) is a stack machine with limited memory size, and can save an arbitrary length byte array as an item.

# Machine specification
* All stack items are byte arrays of arbitrary length
* Maximum stack depth is 1024
* Maximum memory occupation of stack is 1KB
* If memory grows to be larger than 1KB, the machine must fail immediately
* If script tries to push when stack has 1024 items, the machine must fail immediately
* If script tries to pop when stack is empty, the machine must fail immediately

# Type Conversion
Although CCVM itself doesn’t have any type notations, some instructions treat stack items as specific type (e.g. Integer, boolean). Following rules are applied when instruction tries to convert byte array to desired types.

## Integer

### Byte array -> Integer:

* Byte array MUST fit in 8 bytes.
* Byte array is decoded with little-endian byte ordering.
* All items are decoded as unsigned integer.
* Empty array is decoded as 0 in integer.

### Integer -> Byte array:

Leading zeros must be truncated. Note that it is allowed to decode value with leading zeros as integer.

## Boolean

### Byte array -> Boolean:
* false if byte array is empty, or all elements of array is zero
* true otherwise

### Boolean -> Byte array:
* true is encoded as [0x01]
* false is encoded as []

# Instructions

## Special instructions
* NOP(0x00): Do nothing
* BURN(0x01): Stop script execution, and return `BURN` as result.

## Boolean computation
* NOT(0x10): Pop one value from stack as boolean, and push negated value.
* EQ(0x11): Pop two values from stack. Push true if two blobs were exactly equal. Push false otherwise.

## Flow Control
* JMP(0x20)
 1. Read next script byte(refer this value as n)
 1. Skip n - 1 instructions
* JNZ(0x21)
 1. Read next script byte(refer this value as n)
 1. Pop one value from stack as boolean.
 1. If popped value is true, skip n instructions. If value was false, go to next instruction.
* JZ(0x22)
 1. Read next script byte(refer this value as n)
 1. Pop one value from stack as boolean.
 1. If popped value is false, skip n instructions. If value was true, go to next instruction.

## Stack manipulation
* PUSH(0x30): Read next script byte, and push single element array containing that value to stack.
* POP(0x31): Pop one item from stack.
* PUSHB(0x32)
 1. Read next script byte(refer this value as n)
 1. Read next n script bytes from script
 1. Push values from 2. as array to stack
* DUP(0x33): Push copy of topmost value in stack.
* SWAP(0x34): Swap topmost two values in stack.
* COPY(0x35):
 1. Read next script byte(refer this value as n)
 1. Duplicate nth stack item (stack top is 0th value) to stack top.
* DROP(0x36):
 1. Read next script byte(refer this value as n)
 1. Remove nth stack item (stack top is 0th value).

## Cryptography
* CHKSIG(0x80)
 1. Pop two values, first one as signature, second one as public key
 1. Verify signature via full transaction message, excluding script parameter
 1. Push true on success, false otherwise.
* BLAKE256(0x90): Pop one value from stack, and push blake-256 hash of it. Blake-256 here refers to blake2b with 32 byte output.
* SHA256(0x91): Pop one value from stack, and push sha-256 hash of it.
* RIPEMD160(0x92): Pop one value from stack, and push ripemd160 hash of it.
* KECCAK256(0x93): Pop one value from stack, and push keccak-256 hash of it.

## Environment
* BLKNUM(0xa0): Push block number specified in parcel to stack as integer. If there's no specified block number, machine must fail immediately. 