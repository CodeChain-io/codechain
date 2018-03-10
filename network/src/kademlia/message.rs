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

use super::NodeId;
use super::contact::Contact;

pub type Id = u32;

pub enum Message {
    Ping { id: Id, sender: NodeId },
    Pong { id: Id, sender: NodeId },
    FindNode { id: Id, sender: NodeId, target: NodeId, bucket_size: u8 },
    Nodes { id: Id, sender: NodeId, contacts: Vec<Contact> },
}

impl Message {
    pub fn id(&self) -> Id {
        match self {
            &Message::Ping{ id, ..} => id,
            &Message::Pong{ id, ..} => id,
            &Message::FindNode{ id, ..} => id,
            &Message::Nodes{ id, ..} => id,
        }
    }

    pub fn sender(&self) -> &NodeId {
        match self {
            &Message::Ping{ ref sender, ..} => sender,
            &Message::Pong{ ref sender, ..} => sender,
            &Message::FindNode{ ref sender, ..} => sender,
            &Message::Nodes{ ref sender, ..} => sender,
        }
    }
}
