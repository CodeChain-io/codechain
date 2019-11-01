// Copyright 2015-2017 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{cmp, mem, str};

use primitives::{H128, H160, H256, H512, H520, U256};
use stream::RlpStream;
use traits::{Decodable, Encodable};
use {DecoderError, Rlp};

pub fn decode_usize(bytes: &[u8]) -> Result<usize, DecoderError> {
    let expected = mem::size_of::<usize>();
    match bytes.len() {
        l if l <= expected => {
            if bytes[0] == 0 {
                return Err(DecoderError::RlpInvalidIndirection)
            }
            let mut res = 0usize;
            for (i, byte) in bytes.iter().enumerate() {
                let shift = (l - 1 - i) * 8;
                res += (*byte as usize) << shift;
            }
            Ok(res)
        }
        got => Err(DecoderError::RlpIsTooBig {
            expected,
            got,
        }),
    }
}

impl Encodable for bool {
    fn rlp_append(&self, s: &mut RlpStream) {
        if *self {
            s.encoder().encode_value(&[1]);
        } else {
            s.encoder().encode_value(&[0]);
        }
    }
}

impl Decodable for bool {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| match bytes.len() {
            0 => Ok(false),
            1 => Ok(bytes[0] != 0),
            got => Err(DecoderError::RlpIsTooBig {
                expected: 1,
                got,
            }),
        })
    }
}

impl<'a> Encodable for &'a [u8] {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(self);
    }
}

impl Encodable for Vec<u8> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(self);
    }
}

impl Decodable for Vec<u8> {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| Ok(bytes.to_vec()))
    }
}

impl Encodable for Vec<Vec<u8>> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.len());
        for e in self {
            s.append(e);
        }
    }
}

impl Decodable for Vec<Vec<u8>> {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.as_list::<Vec<u8>>()
    }
}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        match *self {
            None => {
                s.begin_list(0);
            }
            Some(ref value) => {
                s.begin_list(1);
                s.append(value);
            }
        }
    }
}

impl<T> Decodable for Option<T>
where
    T: Decodable,
{
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let items = rlp.item_count()?;
        match items {
            1 => rlp.val_at(0).map(Some),
            0 => Ok(None),
            got => Err(DecoderError::RlpIncorrectListLen {
                expected: 1,
                got,
            }),
        }
    }
}

impl Encodable for u8 {
    fn rlp_append(&self, s: &mut RlpStream) {
        if *self != 0 {
            s.encoder().encode_value(&[*self]);
        } else {
            s.encoder().encode_value(&[]);
        }
    }
}

impl Decodable for u8 {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| match bytes.len() {
            1 if bytes[0] != 0 => Ok(bytes[0]),
            0 => Ok(0),
            1 => Err(DecoderError::RlpInvalidIndirection),
            got => Err(DecoderError::RlpIsTooBig {
                expected: 1,
                got,
            }),
        })
    }
}

macro_rules! impl_encodable_for_u {
    ($name:ident, $size:expr) => {
        impl Encodable for $name {
            fn rlp_append(&self, s: &mut RlpStream) {
                let leading_empty_bytes = self.leading_zeros() as usize / 8;
                let buffer: [u8; $size] = self.to_be_bytes();
                s.encoder().encode_value(&buffer[leading_empty_bytes..]);
            }
        }
    };
}

macro_rules! impl_decodable_for_u {
    ($name:ident) => {
        impl Decodable for $name {
            fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
                rlp.decoder().decode_value(|bytes| match bytes.len() {
                    0 | 1 => u8::decode(rlp).map($name::from),
                    l if l <= mem::size_of::<$name>() => {
                        if bytes[0] == 0 {
                            return Err(DecoderError::RlpInvalidIndirection)
                        }
                        let mut res = 0 as $name;
                        for (i, byte) in bytes.iter().enumerate() {
                            let shift = (l - 1 - i) * 8;
                            res += $name::from(*byte) << shift;
                        }
                        Ok(res)
                    }
                    got => Err(DecoderError::RlpIsTooBig {
                        expected: mem::size_of::<$name>(),
                        got,
                    }),
                })
            }
        }
    };
}

impl_encodable_for_u!(u16, 2);
impl_encodable_for_u!(u32, 4);
impl_encodable_for_u!(u64, 8);
impl_encodable_for_u!(u128, 16);

impl_decodable_for_u!(u16);
impl_decodable_for_u!(u32);
impl_decodable_for_u!(u64);
impl_decodable_for_u!(u128);

impl Encodable for usize {
    fn rlp_append(&self, s: &mut RlpStream) {
        (*self as u64).rlp_append(s);
    }
}

impl Decodable for usize {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        u64::decode(rlp).map(|value| value as usize)
    }
}

macro_rules! impl_encodable_for_hash {
    ($name:ident) => {
        impl Encodable for $name {
            fn rlp_append(&self, s: &mut RlpStream) {
                s.encoder().encode_value(self);
            }
        }
    };
}

macro_rules! impl_decodable_for_hash {
    ($name:ident, $size:expr) => {
        impl Decodable for $name {
            fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
                rlp.decoder().decode_value(|bytes| match bytes.len().cmp(&$size) {
                    cmp::Ordering::Less => Err(DecoderError::RlpIsTooShort {
                        expected: $size,
                        got: bytes.len(),
                    }),
                    cmp::Ordering::Greater => Err(DecoderError::RlpIsTooBig {
                        expected: $size,
                        got: bytes.len(),
                    }),
                    cmp::Ordering::Equal => {
                        let mut t = [0u8; $size];
                        t.copy_from_slice(bytes);
                        Ok($name(t))
                    }
                })
            }
        }
    };
}

impl_encodable_for_hash!(H128);
impl_encodable_for_hash!(H160);
impl_encodable_for_hash!(H256);
impl_encodable_for_hash!(H512);
impl_encodable_for_hash!(H520);

impl_decodable_for_hash!(H128, 16);
impl_decodable_for_hash!(H160, 20);
impl_decodable_for_hash!(H256, 32);
impl_decodable_for_hash!(H512, 64);
impl_decodable_for_hash!(H520, 65);

macro_rules! impl_encodable_for_uint {
    ($name:ident, $size:expr) => {
        impl Encodable for $name {
            fn rlp_append(&self, s: &mut RlpStream) {
                let leading_empty_bytes = $size - (self.bits() + 7) / 8;
                let mut buffer = [0u8; $size];
                self.to_big_endian(&mut buffer);
                s.encoder().encode_value(&buffer[leading_empty_bytes..]);
            }
        }
    };
}

macro_rules! impl_decodable_for_uint {
    ($name:ident, $size:expr) => {
        impl Decodable for $name {
            fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
                rlp.decoder().decode_value(|bytes| {
                    if !bytes.is_empty() && bytes[0] == 0 {
                        Err(DecoderError::RlpInvalidIndirection)
                    } else if bytes.len() <= $size {
                        Ok($name::from(bytes))
                    } else {
                        Err(DecoderError::RlpIsTooBig {
                            expected: $size,
                            got: bytes.len(),
                        })
                    }
                })
            }
        }
    };
}

impl_encodable_for_uint!(U256, 32);

impl_decodable_for_uint!(U256, 32);

impl<'a> Encodable for &'a str {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(self.as_bytes());
    }
}

impl Encodable for String {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(self.as_bytes());
    }
}

impl Decodable for String {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| {
            if bytes.contains(&b'\0') {
                return Err(DecoderError::RlpNullTerminatedString)
            }
            match str::from_utf8(bytes) {
                Ok(s) => Ok(s.to_owned()),
                // consider better error type here
                Err(_err) => Err(DecoderError::RlpExpectedToBeData),
            }
        })
    }
}

impl<T1: Encodable, T2: Encodable, T3: Encodable> Encodable for (T1, T2, T3) {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3).append(&self.0).append(&self.1).append(&self.2);
    }
}

impl<T1: Decodable, T2: Decodable, T3: Decodable> Decodable for (T1, T2, T3) {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 3 {
            return Err(DecoderError::RlpIncorrectListLen {
                expected: 3,
                got: item_count,
            })
        }
        Ok((rlp.val_at(0)?, rlp.val_at(1)?, rlp.val_at(2)?))
    }
}

#[macro_export]
macro_rules! rlp_encode_and_decode_test {
    ($origin:expr) => {
        fn rlp_encode_and_decode_test<T>(origin: T)
        where
            T: $crate::Encodable + $crate::Decodable + ::std::fmt::Debug + PartialEq, {
            let encoded = $crate::encode(&origin);
            let decoded = $crate::decode::<T>(&encoded);
            assert_eq!(Ok(origin), decoded);
        }
        rlp_encode_and_decode_test($origin);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_of_bytes() {
        let origin: Vec<Vec<u8>> = vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7], vec![], vec![8, 9]];

        let encoded = ::encode(&origin);

        let expected = {
            let mut s = RlpStream::new();
            s.begin_list(4);
            s.append::<Vec<u8>>(&origin[0]);
            s.append::<Vec<u8>>(&origin[1]);
            s.append::<Vec<u8>>(&origin[2]);
            s.append::<Vec<u8>>(&origin[3]);
            s.out()
        };
        assert_eq!(expected, encoded.to_vec());

        rlp_encode_and_decode_test!(origin);
    }

    #[test]
    fn rlp_zero_h160() {
        let h = H160::zero();
        let encoded = h.rlp_bytes().to_vec();
        assert_eq!(&[0x80 + 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], encoded.as_slice());
    }

    #[test]
    fn vec_and_hash() {
        let vec: Vec<u8> = {
            let mut vec = Vec::with_capacity(32);
            for i in 0..32 {
                vec.push(i);
            }
            vec
        };
        let hash: H256 = {
            let mut hash = H256::zero();
            assert_eq!(32, hash.iter().len());
            for (i, h) in hash.iter_mut().enumerate().take(32) {
                *h = i as u8;
            }
            hash
        };
        assert_eq!(vec.rlp_bytes(), hash.rlp_bytes());
    }

    #[test]
    fn slice_and_hash() {
        let array: [u8; 32] = {
            let mut array = [0 as u8; 32];
            assert_eq!(32, array.iter().len());
            for (i, a) in array.iter_mut().enumerate().take(32) {
                *a = i as u8;
            }
            array
        };
        let slice: &[u8] = &array;
        let hash: H256 = {
            let mut hash = H256::zero();
            assert_eq!(32, hash.iter().len());
            for (i, h) in hash.iter_mut().enumerate().take(32) {
                *h = i as u8;
            }
            hash
        };
        assert_eq!(slice.rlp_bytes(), hash.rlp_bytes());
    }

    #[test]
    fn empty_bytes() {
        let empty_bytes: Vec<u8> = vec![];
        assert_eq!(&[0x80], &empty_bytes.rlp_bytes().to_vec().as_slice());
        rlp_encode_and_decode_test!(empty_bytes);
    }

    #[test]
    fn empty_slice_of_u8() {
        let empty_slice: &[u8] = &[];
        assert_eq!(&[0x80], &empty_slice.rlp_bytes().to_vec().as_slice());
    }

    #[test]
    fn empty_list() {
        let mut stream = RlpStream::new();
        stream.begin_list(0);
        assert_eq!(vec![0xC0], stream.out());
    }

    #[test]
    fn tuple() {
        let tuple: (u32, u32, u32) = (1, 2, 3);
        rlp_encode_and_decode_test!(tuple);
    }
}
