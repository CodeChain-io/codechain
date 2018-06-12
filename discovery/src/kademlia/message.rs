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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::NodeId;

pub type Id = u64;

#[derive(Debug)]
pub enum Message {
    FindNode {
        id: Id,
        sender: NodeId,
        target: NodeId,
        bucket_size: u8,
    },
    Nodes {
        id: Id,
        sender: NodeId,
        nodes: Vec<NodeId>,
    },
}

impl Message {
    #[allow(dead_code)]
    pub fn id(&self) -> &Id {
        match self {
            Message::FindNode {
                id,
                ..
            } => id,
            Message::Nodes {
                id,
                ..
            } => id,
        }
    }

    pub fn sender(&self) -> &NodeId {
        match self {
            Message::FindNode {
                sender,
                ..
            } => sender,
            Message::Nodes {
                sender,
                ..
            } => sender,
        }
    }
}

type ProtocolId = u64;

const FIND_NODE_ID: ProtocolId = 0x2;
const NODES_ID: ProtocolId = 0x3;

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::FindNode {
                id,
                sender,
                target,
                bucket_size,
            } => {
                s.begin_list(5).append(&FIND_NODE_ID).append(id).append(sender).append(target).append(bucket_size);
            }
            Message::Nodes {
                id,
                sender,
                nodes,
            } => {
                s.begin_list(3 + nodes.len()).append(&NODES_ID).append(id).append(sender);
                for node_id in nodes.iter() {
                    s.append(node_id);
                }
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let protocol = rlp.val_at::<ProtocolId>(0)?;
        let id = rlp.val_at(1)?;
        let sender = rlp.val_at(2)?;
        match protocol {
            FIND_NODE_ID => {
                if rlp.item_count()? != 5 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let target = rlp.val_at(3)?;
                let bucket_size = rlp.val_at(4)?;
                Ok(Message::FindNode {
                    id,
                    sender,
                    target,
                    bucket_size,
                })
            }
            NODES_ID => {
                if (rlp.item_count()? - 3) % 2 != 0 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let nodes = {
                    let mut nodes: Vec<NodeId> = vec![];
                    let mut i = 3;
                    let len = rlp.item_count()?;
                    while i < len {
                        let node = rlp.val_at(i)?;
                        nodes.push(node);
                    }
                    nodes
                };
                Ok(Message::Nodes {
                    id,
                    sender,
                    nodes,
                })
            }
            _ => Err(DecoderError::Custom("Invalid protocol id")),
        }
    }
}
