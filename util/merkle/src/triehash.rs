// Copyright 2018 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Generetes trie root.
//!
//! This module should be used to generate trie root hash.

use std::cmp;
use std::collections::BTreeMap;

use ccrypto::blake256;
use primitives::H256;
use rlp::RlpStream;

fn shared_prefix_len<T: Eq>(first: &[T], second: &[T]) -> usize {
    let len = cmp::min(first.len(), second.len());
    (0..len).take_while(|&i| first[i] == second[i]).count()
}

/// Generates a trie root hash for a vector of key-values
pub fn trie_root<I, A, B>(input: I) -> H256
where
    I: IntoIterator<Item = (A, B)>,
    A: AsRef<[u8]> + Ord,
    B: AsRef<[u8]>, {
    // Make key into hash value, which is blake256(key)
    let gen_input: Vec<_> = input.into_iter().map(|(k, v)| (blake256(k), v)).collect();
    let gen_input: Vec<_> = gen_input
		// first put elements into btree to sort them and to remove duplicates
		.into_iter()
		.collect::<BTreeMap<_, _>>()
		// then move them to a vector
		.into_iter()
		.map(|(k, v)| (as_nibbles(k.as_ref()), v) )
		.collect();

    gen_trie_root(&gen_input)
}

fn gen_trie_root<A: AsRef<[u8]>, B: AsRef<[u8]>>(input: &[(A, B)]) -> H256 {
    let mut stream = RlpStream::new();
    hash256rlp(input, 0, &mut stream);
    blake256(stream.out())
}

/// Hex-prefix Notation. First nibble has flags: oddness = 2^0
///
/// Input values are in range `[0, 0xf]`.
///
/// ```markdown
///  [0,0,1,2,3,4,5]   0x10012345 // 7 > 4
///  [0,1,2,3,4,5]     0x00012345 // 6 > 4
///  [1,2,3,4,5]       0x112345   // 5 > 3
///  [0,0,1,2,3,4]     0x00001234 // 6 > 3
///  [0,1,2,3,4]       0x101234   // 5 > 3
///  [1,2,3,4]         0x001234   // 4 > 3
/// ```
fn hex_prefix_encode(nibbles: &[u8]) -> Vec<u8> {
    let inlen = nibbles.len();
    let oddness_factor = inlen % 2;
    // next even number divided by two
    let reslen = (inlen + 2) >> 1;
    let mut res = Vec::with_capacity(reslen);

    let first_byte = {
        let mut bits = (oddness_factor as u8) << 4;
        if oddness_factor == 1 {
            bits += nibbles[0];
        }
        bits
    };

    res.push(first_byte);

    let mut offset = oddness_factor;
    while offset < inlen {
        let byte = (nibbles[offset] << 4) + nibbles[offset + 1];
        res.push(byte);
        offset += 2;
    }

    res
}

/// Converts slice of bytes to nibbles.
fn as_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(bytes.len() * 2);
    for i in 0..bytes.len() {
        let byte = bytes[i];
        res.push(byte >> 4);
        res.push(byte & 0b1111);
    }
    res
}

fn hash256rlp<A: AsRef<[u8]>, B: AsRef<[u8]>>(input: &[(A, B)], pre_len: usize, stream: &mut RlpStream) {
    let inlen = input.len();

    // in case of empty slice, just append empty data
    if inlen == 0 {
        stream.append_empty_data();
        return
    }

    // take slices
    let key: &[u8] = &input[0].0.as_ref();
    let value: &[u8] = &input[0].1.as_ref();

    // if the slice contains just one item, append the suffix of the key
    // and then append value
    if inlen == 1 {
        stream.begin_list(2);
        stream.append(&hex_prefix_encode(&key[pre_len..]));
        stream.append(&value);
        return
    }

    // get length of the longest shared prefix in slice keys
    let shared_prefix = input.iter()
		// skip first element
		.skip(1)
		// get minimum number of shared nibbles between first and each successive
		.fold(key.len(), | acc, &(ref k, _) | {
			cmp::min(shared_prefix_len(key, k.as_ref()), acc)
		});

    // an item for every possible nibble/suffix
    // + 1 for data
    stream.begin_list(17);

    // Append partial path as a first element of branch
    stream.append(&hex_prefix_encode(&key[pre_len..shared_prefix]));

    let mut begin: usize = 0;

    // iterate over all possible nibbles
    for i in 0..16 {
        // count how many successive elements have same next nibble
        let len = match begin < input.len() {
            true => input[begin..].iter().take_while(|pair| pair.0.as_ref()[shared_prefix] == i).count(),
            false => 0,
        };

        // if at least 1 successive element has the same nibble
        // append their suffixes
        match len {
            0 => {
                stream.append_empty_data();
            }
            _ => hash256aux(&input[begin..(begin + len)], shared_prefix + 1, stream),
        }
        begin += len;
    }
}

fn hash256aux<A: AsRef<[u8]>, B: AsRef<[u8]>>(input: &[(A, B)], pre_len: usize, stream: &mut RlpStream) {
    let mut s = RlpStream::new();
    hash256rlp(input, pre_len, &mut s);
    let out = s.out();

    stream.append(&blake256(out));
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles() {
        let v = vec![0x31, 0x23, 0x45];
        let e = vec![3, 1, 2, 3, 4, 5];
        assert_eq!(as_nibbles(&v), e);

        // A => 65 => 0x41 => [4, 1]
        let v: Vec<u8> = From::from("A");
        let e = vec![4, 1];
        assert_eq!(as_nibbles(&v), e);
    }

    #[test]
    fn _hex_prefix_encode() {
        let v = vec![0, 0, 1, 2, 3, 4, 5];
        let e = vec![0x10, 0x01, 0x23, 0x45];
        let h = hex_prefix_encode(&v);
        assert_eq!(h, e);

        let v = vec![0, 1, 2, 3, 4, 5];
        let e = vec![0x00, 0x01, 0x23, 0x45];
        let h = hex_prefix_encode(&v);
        assert_eq!(h, e);

        let v = vec![1, 2, 3, 4];
        let e = vec![0x00, 0x12, 0x34];
        let h = hex_prefix_encode(&v);
        assert_eq!(h, e);
    }

    #[test]
    fn triehash_out_of_order() {
        assert_eq!(
            trie_root(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
                (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
            ]),
            trie_root(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
                (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
            ])
        );
    }

    #[test]
    fn shared_prefix() {
        let a = vec![1, 2, 3, 4, 5, 6];
        let b = vec![4, 2, 3, 4, 5, 6];
        assert_eq!(shared_prefix_len(&a, &b), 0);
    }

    #[test]
    fn shared_prefix2() {
        let a = vec![1, 2, 3, 3, 5];
        let b = vec![1, 2, 3];
        assert_eq!(shared_prefix_len(&a, &b), 3);
    }

    #[test]
    fn shared_prefix3() {
        let a = vec![1, 2, 3, 4, 5, 6];
        let b = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(shared_prefix_len(&a, &b), 6);
    }
}
