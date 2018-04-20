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

use cnetwork::{DiscoveryApi, NodeToken, SocketAddr};
use parking_lot::RwLock;
use std::vec::Vec;

pub struct Simple {
    vec: RwLock<Vec<(NodeToken, SocketAddr)>>,
}

impl Simple {
    pub fn new() -> Self {
        Self {
            vec: RwLock::new(Vec::new()),
        }
    }
}

impl DiscoveryApi for Simple {
    fn get(&self, max: usize) -> Vec<SocketAddr> {
        debug_assert!(max <= ::std::u8::MAX as usize);
        self.vec.read().iter().take(max).map(|&(_, ref addr)| addr.clone()).collect()
    }

    fn add_connection(&self, node: NodeToken, address: SocketAddr) {
        self.remove_connection(&node);
        let mut vec = self.vec.write();
        vec.push((node, address));
    }

    fn remove_connection(&self, node: &NodeToken) {
        let mut vec = self.vec.write();
        *vec = vec.iter().filter(|item| &item.0 != node).map(Clone::clone).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_discovery_add() {
        let discovery = Simple::new();
        let addr = vec![
            (1, SocketAddr::v4(127, 0, 0, 1, 3481)),
            (2, SocketAddr::v4(127, 0, 0, 1, 3482)),
            (3, SocketAddr::v4(127, 0, 0, 1, 3483)),
            (4, SocketAddr::v4(127, 0, 0, 1, 3484)),
            (5, SocketAddr::v4(127, 0, 0, 1, 3485)),
            (6, SocketAddr::v4(127, 0, 0, 1, 3486)),
            (7, SocketAddr::v4(127, 0, 0, 1, 3487)),
            (8, SocketAddr::v4(127, 0, 0, 1, 3488)),
            (9, SocketAddr::v4(127, 0, 0, 1, 3489)),
        ];
        for (n, a) in addr {
            discovery.add_connection(n, a);
        }

        assert_eq!(
            vec![
                SocketAddr::v4(127, 0, 0, 1, 3481),
                SocketAddr::v4(127, 0, 0, 1, 3482),
                SocketAddr::v4(127, 0, 0, 1, 3483),
                SocketAddr::v4(127, 0, 0, 1, 3484),
                SocketAddr::v4(127, 0, 0, 1, 3485),
                SocketAddr::v4(127, 0, 0, 1, 3486),
                SocketAddr::v4(127, 0, 0, 1, 3487),
                SocketAddr::v4(127, 0, 0, 1, 3488),
                SocketAddr::v4(127, 0, 0, 1, 3489),
            ],
            discovery.get(10)
        );

        assert_eq!(
            vec![
                SocketAddr::v4(127, 0, 0, 1, 3481),
                SocketAddr::v4(127, 0, 0, 1, 3482),
                SocketAddr::v4(127, 0, 0, 1, 3483),
                SocketAddr::v4(127, 0, 0, 1, 3484),
            ],
            discovery.get(4)
        );
    }

    #[test]
    fn simple_discovery_remove() {
        let discovery = Simple::new();
        let addr = vec![
            (1, SocketAddr::v4(127, 0, 0, 1, 3481)),
            (2, SocketAddr::v4(127, 0, 0, 1, 3482)),
            (3, SocketAddr::v4(127, 0, 0, 1, 3483)),
            (4, SocketAddr::v4(127, 0, 0, 1, 3484)),
            (5, SocketAddr::v4(127, 0, 0, 1, 3485)),
            (6, SocketAddr::v4(127, 0, 0, 1, 3486)),
            (7, SocketAddr::v4(127, 0, 0, 1, 3487)),
            (8, SocketAddr::v4(127, 0, 0, 1, 3488)),
            (9, SocketAddr::v4(127, 0, 0, 1, 3489)),
        ];
        for (n, a) in addr {
            discovery.add_connection(n, a);
        }
        discovery.remove_connection(&3);
        discovery.remove_connection(&2);
        discovery.remove_connection(&5);
        discovery.remove_connection(&7);
        discovery.remove_connection(&2);
        discovery.remove_connection(&6);


        assert_eq!(
            vec![
                SocketAddr::v4(127, 0, 0, 1, 3481),
                SocketAddr::v4(127, 0, 0, 1, 3484),
                SocketAddr::v4(127, 0, 0, 1, 3488),
                SocketAddr::v4(127, 0, 0, 1, 3489),
            ],
            discovery.get(10)
        );

        assert_eq!(vec![SocketAddr::v4(127, 0, 0, 1, 3481), SocketAddr::v4(127, 0, 0, 1, 3484)], discovery.get(2));
    }
}
