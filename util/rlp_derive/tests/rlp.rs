extern crate rlp;
#[macro_use]
extern crate rlp_derive;

use rlp::{decode, encode};

#[derive(Debug, PartialEq, RlpEncodable, RlpDecodable)]
struct Foo {
    a: String,
}

#[derive(Debug, PartialEq, RlpEncodableWrapper, RlpDecodableWrapper)]
struct FooWrapper {
    a: String,
}

#[test]
fn encode_foo() {
    let f = Foo {
        a: "cat".into(),
    };

    let expected = vec![0xc4, 0x83, b'c', b'a', b't'];
    let out = encode(&f).into_vec();
    assert_eq!(out, expected);

    let decoded = decode(&expected);
    assert_eq!(f, decoded);
}

#[test]
fn encode_foo_wrapper() {
    let f = FooWrapper {
        a: "cat".into(),
    };

    let expected = vec![0x83, b'c', b'a', b't'];
    let out = encode(&f).into_vec();
    assert_eq!(out, expected);

    let decoded = decode(&expected);
    assert_eq!(f, decoded);
}
