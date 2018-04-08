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

extern crate rand;

use std::collections::VecDeque;
use std::vec::Vec;

use super::NodeId;
use super::command::Command;
use super::contact::Contact;
use super::event::Event;
use super::message::Id as MessageId;
use super::message::Message;
use super::node_id::{log2_distance_between_nodes, self};
use super::routing_table::RoutingTable;
use super::super::SocketAddr;

pub struct Kademlia {
    alpha: u8,
    k: u8,
    pub t_refresh: u32,
    table: RoutingTable,
    to_be_verified: VecDeque<Contact>,
}

impl Kademlia {
    pub fn new(local_id: Option<NodeId>, alpha: u8, k: u8, t_refresh: u32) -> Self {
        let local_id = local_id.unwrap_or(node_id::random());
        Kademlia {
            alpha,
            k,
            t_refresh,
            table: RoutingTable::new(local_id, k),
            to_be_verified: VecDeque::new(),
        }
    }

    fn local_id(&self) -> NodeId {
        self.table.local_id()
    }

    fn touch_contact(&mut self, contact: Contact) -> bool {
        if let Some(head) = self.table.touch_contact(contact.clone())
                .map(|head| head.clone()) {
            self.add_contact_to_be_verified(head)
        } else {
            false
        }
    }

    fn add_contact_to_be_verified(&mut self, contact: Contact) -> bool {
        if self.to_be_verified.contains(&contact) {
            false
        } else {
            self.to_be_verified.push_back(contact);
            true
        }
    }

    fn pop_contact_to_be_verified(&mut self) -> Option<Contact> {
        while let Some(contact) = self.to_be_verified.pop_front() {
            if self.table.contains(&contact) {
                return Some(contact);
            }
        }
        None
    }


    fn handle_find_node_message(&self, id: MessageId, _sender: NodeId, target: NodeId, bucket_size: u8, sender_address: &SocketAddr) -> Option<Command> {
        let contacts = self.table.get_closest_contacts(&target, bucket_size);
        let message = Message::Nodes {
            id,
            sender: self.local_id(),
            contacts,
        };
        let target = sender_address.clone();
        Some(Command::Send { message, target })
    }

    fn handle_nodes_message(&mut self, sender: NodeId, contacts: &Vec<Contact>) -> Option<Command> {
        let local_id = self.local_id();
        let distance_to_target = log2_distance_between_nodes(&local_id, &sender);
        contacts.into_iter()
            .take(self.k as usize)
            .filter(|contact| contact.log2_distance(&local_id) <= distance_to_target)
            .map(|contact| self.touch_contact(contact.clone()))
            .find(|added| *added)
            .and(Some(Command::Verify))
    }

    pub fn handle_message(&mut self, message: &Message, sender_address: &SocketAddr) -> Option<Command> {
        // FIXME : Check validity of response first.

        let sender_contact = Contact::new(message.sender().clone(), sender_address.clone());
        if self.table.conflicts(&sender_contact) {
            // Duplicated id with different address
            return None
        }

        self.touch_contact(sender_contact);

        match message {
            &Message::FindNode{id, sender, target, bucket_size} => self.handle_find_node_message(id, sender, target, bucket_size, sender_address),
            &Message::Nodes{ref contacts, sender, ..} => self.handle_nodes_message(sender, contacts),
        }
    }

    pub fn find_node_command(&mut self, target: SocketAddr) -> Command {
        let message = Message::FindNode {
            id: 0, // FIXME
            sender: self.local_id(),
            target: self.local_id(),
            bucket_size: self.k,
        };
        Command::Send {
            message,
            target,
        }
    }


    pub fn handle_verify_command(&mut self) -> Option<Command> {
        self.pop_contact_to_be_verified().map(|contact| {
            let message = Message::FindNode { id: 0, sender: self.local_id(), target: self.local_id(), bucket_size: self.k };
            let target = contact.addr().clone();
            Command::Send {
                message,
                target
            }
        })
    }

    fn handle_send_command(&mut self, _message: &Message, _target: &SocketAddr) -> Option<Command> {
        // FIXME: implement it
        None
    }

    pub fn handle_refresh_command(&mut self) -> Option<Command> {
        self.table.cleanup();
        let distances = self.table.distances();
        let len = distances.len();
        if len == 0 {
            return None
        }
        let index = rand::random::<usize>() % len;

        if let Some(distance) = distances.get(index) {
            for contact in self.table.get_contacts_with_distance(*distance) {
                self.add_contact_to_be_verified(contact);
            }
        }

        Some(Command::Verify)
    }

    pub fn get_closest_addresses(&self, max: usize) -> Vec<SocketAddr> {
        debug_assert!(max <= ::std::u8::MAX as usize);
        let contacts = self.table.get_closest_contacts(&self.local_id(), max as u8);
        contacts.into_iter()
            .map(|contact| contact.addr().clone())
            .collect()
    }

    pub fn remove(&mut self, address: &SocketAddr) {
        let _ = self.table.remove_address(&address);
        let _ = self.to_be_verified.retain(|contact| contact.addr() != address);
    }
}


#[cfg(test)]
mod tests {
    use super::Kademlia;
    use super::NodeId;
    use super::super::contact::Contact;

    use super::super::{ALPHA, K, T_REFRESH};

    pub fn default_kademlia(local_id: NodeId) -> Kademlia {
        let local_id = Some(local_id);
        Kademlia::new(local_id, ALPHA, K, T_REFRESH)
    }


    const ID: &str = "0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

    const ID1: &str = "0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000001";

    const ID4: &str = "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000004";

    const ID5: &str = "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000005";

    #[test]
    fn test_default_alpha() {
        let id = Contact::from_hash(ID).id();
        let kademlia = default_kademlia(id);
        assert_eq!(3, kademlia.alpha);
    }

    #[test]
    fn test_default_k() {
        let id = Contact::from_hash(ID).id();
        let kademlia = default_kademlia(id);
        assert_eq!(16, kademlia.k);
    }

    #[test]
    fn test_default_t_refresh() {
        let id = Contact::from_hash(ID).id();
        let kademlia = default_kademlia(id);
        assert_eq!(60_000, kademlia.t_refresh);
    }

    #[test]
    fn test_add_contact_to_be_verfied_does_not_add_duplicates() {
        let id = Contact::from_hash(ID).id();
        let mut kademlia = default_kademlia(id);

        let new_contact = Contact::from_hash(ID1);

        assert_eq!(0, kademlia.to_be_verified.len());

        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert!(!kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_pop_contact_to_be_verfied() {
        let id = Contact::from_hash(ID).id();
        let mut kademlia = default_kademlia(id);

        let new_contact = Contact::from_hash(ID1);

        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.table.touch_contact(new_contact.clone());
        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert_eq!(Some(new_contact), kademlia.pop_contact_to_be_verified());
        assert_eq!(0, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_pop_contact_to_be_verfied_returns_none_when_empty() {
        let id = Contact::from_hash(ID).id();
        let mut kademlia = default_kademlia(id);

        let new_contact = Contact::from_hash(ID1);

        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.table.touch_contact(new_contact.clone());
        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert_eq!(Some(new_contact), kademlia.pop_contact_to_be_verified());
        assert_eq!(0, kademlia.to_be_verified.len());

        assert_eq!(None, kademlia.pop_contact_to_be_verified());
    }

    #[test]
    fn test_pop_contact_to_be_verfied_skips_the_contact_which_is_not_in_routing_table() {
        let id = Contact::from_hash(ID).id();
        let mut kademlia = default_kademlia(id);

        let new_contact = Contact::from_hash(ID1);

        assert_eq!(0, kademlia.to_be_verified.len());

        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert!(kademlia.pop_contact_to_be_verified().is_none());
        assert_eq!(0, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_add_contact_adds_to_be_verified_when_bucket_is_full() {
        let id = Some(Contact::from_hash(ID).id());
        let mut kademlia = Kademlia::new(id, ALPHA, 1, T_REFRESH);

        let contact4 = Contact::from_hash(ID4);
        let contact5 = Contact::from_hash(ID5);

        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.touch_contact(contact4);
        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.touch_contact(contact5);
        assert_eq!(1, kademlia.to_be_verified.len());
    }

    #[test]
    fn handle_refresh_command_must_not_crash() {
        let mut kademlia = Kademlia::new(None, 3, 8, 60_000);
        kademlia.handle_refresh_command();
    }
}
