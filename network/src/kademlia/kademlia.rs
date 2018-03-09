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

use super::contact::Contact;
use super::routing_table::RoutingTable;

use super::ALPHA;
use super::K;
use super::T_REFRESH;

pub struct Kademlia {
    alpha: u8,
    k: u8,
    t_refresh: u32,
    table: RoutingTable,
}

impl Kademlia {
    pub fn new(localhost: Contact) -> Self {
        const DEFAULT_BUCKET_SIZE: u8 = 8;
        Kademlia {
            alpha: ALPHA,
            k: K,
            t_refresh: T_REFRESH,
            table: RoutingTable::new(localhost, DEFAULT_BUCKET_SIZE),
        }
    }

    // FIXME: Implement message handler.
}


#[cfg(test)]
mod tests {
    use super::Kademlia;
    use super::super::contact::Contact;

    const ID: &str = "0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
    #[test]
    fn test_default_alpha() {
        let kademlia = Kademlia::new(Contact::from_hash(ID));
        assert_eq!(3, kademlia.alpha);
    }

    #[test]
    fn test_default_k() {
        let kademlia = Kademlia::new(Contact::from_hash(ID));
        assert_eq!(16, kademlia.k);
    }

    #[test]
    fn test_default_t_refresh() {
        let kademlia = Kademlia::new(Contact::from_hash(ID));
        assert_eq!(60_000, kademlia.t_refresh);
    }
}
