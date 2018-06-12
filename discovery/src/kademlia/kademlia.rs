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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::vec::Vec;

use cnetwork::{IntoSocketAddr, SocketAddr};

use super::command::Command;
use super::message::{self, Message};
use super::node_id::log2_distance_between_nodes;
use super::routing_table::RoutingTable;
use super::NodeId;

pub struct Kademlia {
    k: u8,
    pub t_refresh: u32,
    table: RoutingTable,
    to_be_verified: VecDeque<NodeId>,
    seq: AtomicUsize,
}

impl Kademlia {
    pub fn new(local_id: NodeId, k: u8, t_refresh: u32) -> Self {
        Kademlia {
            k,
            t_refresh,
            table: RoutingTable::new(local_id, k),
            to_be_verified: VecDeque::new(),
            seq: AtomicUsize::new(0),
        }
    }

    fn local_id(&self) -> NodeId {
        self.table.local_id()
    }

    fn touch_contact(&mut self, node_id: NodeId) -> bool {
        if let Some(head) = self.table.touch_contact(node_id.clone()).cloned() {
            self.add_contact_to_be_verified(head)
        } else {
            false
        }
    }

    fn add_contact_to_be_verified(&mut self, node_id: NodeId) -> bool {
        if self.to_be_verified.contains(&node_id) {
            false
        } else {
            self.to_be_verified.push_back(node_id);
            true
        }
    }

    fn pop_contact_to_be_verified(&mut self) -> Option<NodeId> {
        while let Some(node_id) = self.to_be_verified.pop_front() {
            if self.table.contains(&node_id) {
                return Some(node_id)
            }
        }
        None
    }


    fn handle_find_node_message(
        &self,
        id: message::Id,
        _sender: NodeId,
        target: NodeId,
        bucket_size: u8,
        sender_address: &SocketAddr,
    ) -> Option<Command> {
        let nodes = self.table.get_closest_nodes(&target, bucket_size);
        let message = Message::Nodes {
            id,
            sender: self.local_id(),
            nodes,
        };
        let target = sender_address.clone();
        Some(Command::Send {
            message,
            target,
        })
    }

    fn handle_nodes_message(&mut self, sender: NodeId, nodes: &Vec<NodeId>) -> Option<Command> {
        let local_id = self.local_id();
        let distance_to_target = log2_distance_between_nodes(&local_id, &sender);
        let add_aggressive = self.table.len() < self.k as usize;
        nodes
            .into_iter()
            .take(self.k as usize)
            .filter(|node_id| *node_id != &local_id)
            .filter(|node_id| add_aggressive || log2_distance_between_nodes(node_id, &local_id) <= distance_to_target)
            .map(|node_id| self.touch_contact(node_id.clone()))
            .find(|added| *added)
            .and(Some(Command::Verify))
    }

    pub fn handle_message(&mut self, message: &Message, sender_address: &SocketAddr) -> Option<Command> {
        // FIXME : Check validity of response first.

        let sender_contact = message.sender().clone().into();

        self.touch_contact(sender_contact);

        match message {
            Message::FindNode {
                id,
                sender,
                target,
                bucket_size,
            } => self.handle_find_node_message(*id, *sender, *target, *bucket_size, sender_address),
            Message::Nodes {
                nodes,
                sender,
                ..
            } => self.handle_nodes_message(*sender, nodes),
        }
    }

    pub fn find_node_command(&mut self, target: SocketAddr) -> Command {
        let id = self.seq.fetch_add(1, Ordering::SeqCst) as message::Id;
        let message = Message::FindNode {
            id,
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
        self.pop_contact_to_be_verified().map(|node_id| {
            let target = node_id.into_addr();
            self.find_node_command(target)
        })
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
            for node_id in self.table.get_contacts_with_distance(*distance) {
                self.add_contact_to_be_verified(node_id);
            }
        }

        Some(Command::Verify)
    }

    pub fn remove(&mut self, address: &SocketAddr) {
        let _ = self.table.remove_address(&address);
        let _ = self.to_be_verified.retain(|node_id| &node_id.into_addr() != address);
    }
}


#[cfg(test)]
mod tests {
    use super::super::{K, T_REFRESH};
    use super::*;

    pub fn default_kademlia(local_id: NodeId) -> Kademlia {
        Kademlia::new(local_id, K, T_REFRESH)
    }

    lazy_static! {
        static ref IDS: [SocketAddr; 18] = [
            SocketAddr::v4(127, 0, 0, 1, 8000),
            SocketAddr::v4(127, 0, 0, 1, 8001),
            SocketAddr::v4(127, 0, 0, 1, 8002),
            SocketAddr::v4(127, 0, 0, 1, 8003),
            SocketAddr::v4(127, 0, 0, 1, 8004),
            SocketAddr::v4(127, 0, 0, 1, 8005),
            SocketAddr::v4(127, 0, 0, 1, 8006),
            SocketAddr::v4(127, 0, 0, 1, 8007),
            SocketAddr::v4(127, 0, 0, 1, 8008),
            SocketAddr::v4(127, 0, 0, 1, 8009),
            SocketAddr::v4(127, 0, 0, 1, 8010),
            SocketAddr::v4(127, 0, 0, 1, 8011),
            SocketAddr::v4(127, 0, 0, 1, 8012),
            SocketAddr::v4(127, 0, 0, 1, 8013),
            SocketAddr::v4(127, 0, 0, 1, 8014),
            SocketAddr::v4(127, 0, 0, 1, 8015),
            SocketAddr::v4(127, 0, 0, 1, 8016),
            SocketAddr::v4(127, 0, 0, 1, 8017),
        ];
    }

    #[test]
    fn test_default_k() {
        let id = IDS[0].clone().into();
        let kademlia = default_kademlia(id);
        assert_eq!(16, kademlia.k);
    }

    #[test]
    fn test_default_t_refresh() {
        let id = IDS[0].clone().into();
        let kademlia = default_kademlia(id);
        assert_eq!(60_000, kademlia.t_refresh);
    }

    #[test]
    fn test_add_contact_to_be_verfied_does_not_add_duplicates() {
        let id = IDS[0].clone().into();
        let mut kademlia = default_kademlia(id);

        let new_contact: NodeId = IDS[1].clone().into();

        assert_eq!(0, kademlia.to_be_verified.len());

        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert!(!kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_pop_contact_to_be_verfied() {
        let id = IDS[0].clone().into();
        let mut kademlia = default_kademlia(id);

        let new_contact: NodeId = IDS[1].clone().into();

        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.table.touch_contact(new_contact.clone());
        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert_eq!(Some(new_contact), kademlia.pop_contact_to_be_verified());
        assert_eq!(0, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_pop_contact_to_be_verfied_returns_none_when_empty() {
        let id = IDS[0].clone().into();
        let mut kademlia = default_kademlia(id);

        let new_contact: NodeId = IDS[1].clone().into();

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
        let id = IDS[0].clone().into();
        let mut kademlia = default_kademlia(id);

        let new_contact: NodeId = IDS[1].clone().into();

        assert_eq!(0, kademlia.to_be_verified.len());

        assert!(kademlia.add_contact_to_be_verified(new_contact.clone()));
        assert_eq!(1, kademlia.to_be_verified.len());

        assert!(kademlia.pop_contact_to_be_verified().is_none());
        assert_eq!(0, kademlia.to_be_verified.len());
    }

    #[test]
    fn test_add_contact_adds_to_be_verified_when_bucket_is_full() {
        let id = IDS[0].clone().into();
        let mut kademlia = Kademlia::new(id, 1, T_REFRESH);

        let node4 = IDS[4].clone().into();
        let node5 = IDS[5].clone().into();

        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.touch_contact(node4);
        assert_eq!(0, kademlia.to_be_verified.len());

        kademlia.touch_contact(node5);
        assert_eq!(0, kademlia.to_be_verified.len());
    }

    #[test]
    fn handle_refresh_command_must_not_crash() {
        let node_id = SocketAddr::v4(127, 0, 0, 1, 8080).into();
        let mut kademlia = Kademlia::new(node_id, 8, 60_000);
        kademlia.handle_refresh_command();
    }
}
