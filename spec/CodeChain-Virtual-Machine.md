CodeChain Virtual Machine (CCVM) is a stack machine with limited memory size, and can save a byte array of arbitrary length as an item.

# Machine specification
* All stack items are byte arrays of arbitrary length
* Maximum stack depth is 1024
* Maximum memory occupation of stack is 1KB
* If the memory grows to be larger than 1KB, the machine must fail immediately
* If the script tries to push when the stack has 1024 items, the machine must fail immediately
* If the script tries to pop when the stack is empty, the machine must fail immediately

# Type Conversion
Although CCVM itself doesn’t have any type notations, some instructions treat stack items as a specific type (e.g. Integer, boolean). The following rules are applied when the instruction tries to convert a byte array to other desired types.

## Integer

### Byte array -> Integer:

* A byte array MUST fit in 8 bytes.
* A byte array is decoded with little-endian byte ordering.
* All items are decoded as an unsigned integer.
* An empty array is decoded as a 0 in an integer.

### Integer -> Byte array:

Leading zeros must be truncated. Note that it is allowed to decode a value with leading zeros as an integer.

## Boolean

### Byte array -> Boolean:
* false if the byte array is empty, or all the elements of the array are zeros
* true otherwise

### Boolean -> Byte array:
* true is encoded as [0x01]
* false is encoded as []

# Instructions

## Special instructions
* NOP(0x00): Do nothing
* BURN(0x01): Stop script execution, and return `BURN` as the result.
* SUCCESS(0x02): Stop script execution, and return `SUCCESS` as the result.
* FAIL(0x03): Stop script execution, and return `FAIL` as the result.

## Boolean computation
* NOT(0x10): Pop one value from the stack as a boolean, and push the negated value.
* EQ(0x11): Pop two values from the stack. Push true if the two blobs were exactly equal. Push false otherwise.

## Flow Control
* JMP(0x20)
 1. Read the next script byte(refer this value as n)
 1. Skip n - 1 instructions
* JNZ(0x21)
 1. Read the next script byte(refer this value as n)
 1. Pop one value from the stack as a boolean.
 1. If the popped value is true, skip n instructions. If the value was false, go to the next instruction.
* JZ(0x22)
 1. Read the next script byte(refer this value as n)
 1. Pop one value from the stack as a boolean.
 1. If the popped value is false, skip n instructions. If the value was true, go to the next instruction.

## Stack manipulation
* PUSH(0x30): Read the next script byte, and push a single element array containing that value to the stack.
* POP(0x31): Pop one item from the stack.
* PUSHB(0x32)
 1. Read the next script byte(refer to this value as n)
 1. Read the next n script bytes from the script
 1. Push the values from 2. as an array to the stack
* DUP(0x33): Push a copy of the topmost value in the stack.
* SWAP(0x34): Swap the two topmost values in the stack.
* COPY(0x35):
 1. Read the next script byte(refer to this value as n)
 1. Duplicate the nth stack item (the stack's top is the 0th value) to the stack's top.
* DROP(0x36):
 1. Read the next script byte(refer this value as n)
 1. Remove the nth stack item (stack top is 0th value).

## Verification
* CHKSIG(0x80)
 1. Pop three values, the first one as the public key, the second one as the tag and the last one as the signature.
 1. Verify the signature over the transaction message filtered by the tag, excluding the script parameter.
 1. Push true on success, false otherwise.
* CHKMULTISIG(0x81)
 1. Pop one value, the value is n in the m-of-n Multisig.
 1. Pop n values, which are distinct public keys.
 1. Pop one value, the value is m in the m-of-n Multisig. The value must be less than or equal to the value n.
 1. Pop m values, which are distinct signatures. The signature scheme is the same as CHKSIG.
 1. Pop the tag value. 
 1. Verify the signatures over the transaction message filtered by the tag. The signatures must be ordered the same way as the public keys.
 1. Push true on success, false otherwise.
The specification about the tag is [here](Tag-encoding.md)

## Hashing

* BLAKE256(0x90): Pop one value from the stack, and push the blake-256 hash of it. Blake-256 here refers to blake2b with a 32 byte output.
* SHA256(0x91): Pop one value from the stack, and push the sha-256 hash of it.
* RIPEMD160(0x92): Pop one value from the stack, and push the ripemd160 hash of it.
* KECCAK256(0x93): Pop one value from the stack, and push the keccak-256 hash of it.
* BLAKE160(0x94): Pop one value from the stack, and push the blake-160 hash of it. Blake-160 here refers to blake2b with a 20 byte output.

## Environment
* BLKNUM(0xa0): Push the block number specified in the transaction to the stack as an integer. If there's no specified block number, the machine must fail immediately.

## Timelock
* CHKTIMELOCK(0xb0)
 1. Pop one item from the stack, which is the encoded number for the 4 types of timelock. It must be between 1 and 4. The script will fail otherwise.
   - 1: Block
   - 2: BlockAge
   - 3: Time
   - 4: TimeAge
 2. Pop one more item from stack, which is the value of the timelock. It must be a big-endian encoded, 64-bit unsigned integer. The script will fail if the length of the item exceeds 8.
 2. Check the condition given the type and the value referring to the block number and the timestamp of the best block. See the `Timelock` section in [Transaction](Transaction.md) for more details.
 3. Push true if the condition is met, false otherwise.
