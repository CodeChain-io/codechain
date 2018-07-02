# Type Encoding

CodeChain Wire Protocol uses RLP to encode and decode data. Since RLP doesn’t mention how each type is encoded, CodeChain Wire Protocol specifies encodings of each type.

* Unsigned Integer
* Signed Integer
* Boolean
* String
* Binary
* Datetime
* Array

When encoding types, both the field name and type information themselves are not included. Therefore, the encoder and decoder MUST agree on the meaning and the type of each field.

## Unsigned Integer

An unsigned integer is represented as big-endian without prefix zeros. For example, all 16-bit 1(0x0001), 32-bit 1(0x0000 0001) and 64-bit 1(0x0000 0000 0000 0001) are encoded as 0x01.

The size of an unsigned integer is not specified and implementation dependent. The maximum size of an unsigned integer is not defined, but it MUST be able to handle at least 64-bit unsigned integer.

* 32-bit unsigned integer examples

| Data          | Encoded Value             |
| ------------- | ------------------------- |
| 10            | 0x0a                      |
| 1,000         | 0x82 0x03 0xe8            |
| 100,000       | 0x83 0x01 0x86 0xa0       |
| 10,000,000    | 0x83 0x98 0x96 0x80       |
| 1,000,000,000 | 0x84 0x3b 0x9a 0xca 0x00  |

* 64-bit unsigned integer examples

| Data              | Encoded Value                      |
| ----------------- | ---------------------------------- |
| 10                | 0x0a                               |
| 1,000             | 0x82 0x03 0xe8                     |
| 100,000           | 0x83 0x01 0x86 0xa0                |
| 10,000,000        | 0x83 0x98 0x96 0x80                |
| 1,000,000,000     | 0x84 0x3b 0x9a 0xca 0x00           |
| 100,000,000,000   | 0x85 0x17 0x48 0x76 0xe8 0x00      |
| 1,000,000,000,000 | 0x86 0x09 0x18 0x4e 0x72 0xa0 0x00 |

## Signed Integer

CodeChain wire protocol supports two sizes of signed integer: 32-bit signed integer and 64-bit signed integer.

A signed integer is represented as big-endian with a fixed size. For example, 32-bit signed 1(0x0000 0001) is encoded as “0x84 0x00 0x00 0x00 0x01” while 32-bit unsigned 1 is encoded as “0x01”.

* 32-bit signed integer examples

| Data          | Encoded Value             |
| ------------- | ------------------------- |
| 10            | 0x84 0x00 0x00 0x00 0xa0  |
| 1,000         | 0x84 0x00 0x00 0x03 0xe8  |
| 100,000       | 0x84 0x00 0x01 0x86 0xa0  |
| -10           | 0x84 0xff 0xff 0xff 0xf6  |
| -1,000        | 0x84 0xff 0xff 0xfc 0x18  |
| -100,000      | 0x84 0xff 0xfe 0x79 0x60  |

* 64-bit signed integer examples

| Data              | Encoded Value                                |
| ----------------- | -------------------------------------------- |
| 10                | 0x88 0x00 0x00 0x00 0x00 0x00 0x00 0x00 0x0a |
| 1,000             | 0x88 0x00 0x00 0x00 0x00 0x00 0x00 0x03 0xe8 |
| 100,000           | 0x88 0x00 0x00 0x00 0x00 0x00 0x01 0x86 0xa0 |
| -10               | 0x88 0xff 0xff 0xff 0xff 0xff 0xff 0xff 0xf6 |
| -1,000            | 0x88 0xff 0xff 0xff 0xff 0xff 0xff 0xfc 0x18 |
| -100,000          | 0x88 0xff 0xff 0xff 0xff 0xff 0xfe 0x79 0x60 |

## Boolean

There are only two members in boolean. 0x01 represents True, and 0x00 represents false. Anything else is invalid. The handling of invalid value is not specified.

* Boolean examples

| Data   | Encoded Value  |
| ------ | -------------- |
| False  | 0x00           |
| True   | 0x01           |

## String

String is represented as UTF-8 encoding. A string MUST NOT be null terminated. The length of string is prefixed by RLP encoding.

* String examples

| Data      | Encoded Value                                     |
| --------- | ------------------------------------------------- |
| A         | 0x41                                              |
| CodeChain | 0x89 0x43 0x6f 0x64 0x65 0x43 0x68 0x61 0x69 0x6e |

## Binary

Binary is a variable length sequence of arbitrary characters. Unlike String, any character can be an element of Binary. The length of a binary sequence is prefixed as RLP encoding.

## Datetime

Datetime in CodeChain wire protocol is represented as unix timestamp. It will be treated as a unsigned integer. In other words, the leading zeros of timestamp MUST be deleted.

* Datetime examples

| Data                      | Encoded Value            |
| ------------------------- | ------------------------ |
| 2018-03-07T03:28:22+00:00 | 0x84 0x5a 0x9f 0x5c 0x56 |

## Array

An array is encoded as an a RLP array.
