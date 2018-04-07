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
use super::contact::Contact;

pub type Id = u64;

pub enum Message {
    Ping {
        id: Id,
        sender: NodeId,
    },
    Pong {
        id: Id,
        sender: NodeId,
    },
    FindNode {
        id: Id,
        sender: NodeId,
        target: NodeId,
        bucket_size: u8,
    },
    Nodes {
        id: Id,
        sender: NodeId,
        contacts: Vec<Contact>,
    },
}

impl Message {
    pub fn id(&self) -> Id {
        match self {
            &Message::Ping { id, .. } => id,
            &Message::Pong { id, .. } => id,
            &Message::FindNode { id, .. } => id,
            &Message::Nodes { id, .. } => id,
        }
    }

    pub fn sender(&self) -> &NodeId {
        match self {
            &Message::Ping { ref sender, .. } => sender,
            &Message::Pong { ref sender, .. } => sender,
            &Message::FindNode { ref sender, .. } => sender,
            &Message::Nodes { ref sender, .. } => sender,
        }
    }
}

type ProtocolId = u64;

const PING_ID: ProtocolId = 0x0;
const PONG_ID: ProtocolId = 0x1;
const FIND_NODE_ID: ProtocolId = 0x2;
const NODES_ID: ProtocolId = 0x3;

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::Ping { id, ref sender } => {
                s.begin_list(3).append(&PING_ID).append(&id).append(sender);
            }
            &Message::Pong { id, ref sender } => {
                s.begin_list(3).append(&PONG_ID).append(&id).append(sender);
            }
            &Message::FindNode {
                id,
                ref sender,
                ref target,
                bucket_size,
            } => {
                s.begin_list(5)
                    .append(&FIND_NODE_ID)
                    .append(&id)
                    .append(sender)
                    .append(target)
                    .append(&bucket_size);
            }
            &Message::Nodes {
                id,
                ref sender,
                ref contacts,
            } => {
                s.begin_list(3 + contacts.len() * 2)
                    .append(&NODES_ID)
                    .append(&id)
                    .append(sender);
                for ref contact in contacts.iter() {
                    s.append(&contact.id());
                    s.append(contact.addr());
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
            PING_ID => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen);
                }
                Ok(Message::Ping { id, sender })
            }
            PONG_ID => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen);
                }
                Ok(Message::Pong { id, sender })
            }
            FIND_NODE_ID => {
                if rlp.item_count()? != 5 {
                    return Err(DecoderError::RlpIncorrectListLen);
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
                    return Err(DecoderError::RlpIncorrectListLen);
                }
                let contacts = {
                    let mut contacts: Vec<Contact> = vec![];
                    let mut i = 3;
                    let len = rlp.item_count()?;
                    while i < len {
                        let id = rlp.val_at(i)?;
                        let addr = rlp.val_at(i + 1)?;
                        let contact = Contact::new(id, addr);
                        contacts.push(contact);
                        i += 2;
                    }
                    contacts
                };
                Ok(Message::Nodes {
                    id,
                    sender,
                    contacts,
                })
            }
            _ => Err(DecoderError::Custom("Invalid protocol id")),
        }
    }
}
