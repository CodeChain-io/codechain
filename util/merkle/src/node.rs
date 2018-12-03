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

use elastic_array::ElasticArray1024;
use primitives::H256;
use rlp::*;

use nibbleslice::NibbleSlice;


#[derive(Eq, PartialEq, Debug)]
pub enum Node<'a> {
    Leaf(NibbleSlice<'a>, &'a [u8]),

    Branch(NibbleSlice<'a>, Box<[Option<H256>; 16]>),
}

impl<'a> Node<'a> {
    /// Decode the `node_rlp` and return the Node.
    pub fn decoded(node_rlp: &'a [u8]) -> Option<Self> {
        let r = Rlp::new(node_rlp);
        match r.prototype() {
            // Empty node
            Prototype::Data(0) => None,
            // leaf node - first is nibbles and second is value
            Prototype::List(2) => {
                let slice = NibbleSlice::from_encoded(r.at(0).data());

                Some(Node::Leaf(slice, r.at(1).data()))
            }
            // branch node - first is nibbles (or empty), the rest 16 are nodes.
            Prototype::List(17) => {
                let mut nodes = [None; 16];
                debug_assert_eq!(16, nodes.len());
                for (i, mut node) in nodes.iter_mut().enumerate().map(|(i, node)| (i + 1, node)) {
                    *node = if r.at(i).is_empty() {
                        None
                    } else {
                        Some(r.val_at::<H256>(i))
                    };
                }

                Some(Node::Branch(NibbleSlice::from_encoded(r.at(0).data()), nodes.into()))
            }

            // something went wrong.
            _ => panic!("Rlp data is not valid."),
        }
    }

    /// Encode the node into RLP.
    pub fn encoded(node: Self) -> ElasticArray1024<u8> {
        match node {
            Node::Leaf(slice, value) => {
                let mut stream = RlpStream::new_list(2);
                stream.append(&&*slice.encoded());
                stream.append(&value);
                stream.drain()
            }
            Node::Branch(slice, nodes) => {
                let mut stream = RlpStream::new_list(17);

                stream.append(&&*slice.encoded());

                for child in nodes.iter() {
                    if let Some(hash) = child {
                        stream.append(hash);
                    } else {
                        stream.append_empty_data();
                    }
                }
                stream.drain()
            }
        }
    }

    /// Encode the node into RLP.
    /// What the difference with above `encoded()` is length of nibblepath encoded
    pub fn encoded_until(node: Self, size: usize) -> ElasticArray1024<u8> {
        match node {
            Node::Leaf(slice, value) => {
                let mut stream = RlpStream::new_list(2);
                stream.append(&&*slice.encoded_leftmost(size));
                stream.append(&&*value);
                stream.drain()
            }
            Node::Branch(slice, nodes) => {
                let mut stream = RlpStream::new_list(17);

                stream.append(&&*slice.encoded_leftmost(size));

                for child in nodes.iter() {
                    if let Some(hash) = child {
                        stream.append(hash);
                    } else {
                        stream.append_empty_data();
                    }
                }
                stream.drain()
            }
        }
    }
}
