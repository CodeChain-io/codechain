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

use std::collections::HashSet;
use std::net::IpAddr;

pub struct Filter {
    enabled: bool,
    list: HashSet<IpAddr>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            enabled: false,
            list: Default::default(),
        }
    }
}

impl Filter {
    pub fn new(input_vector: Vec<IpAddr>) -> Self {
        Self {
            enabled: !input_vector.is_empty(),
            list: input_vector.into_iter().collect(),
        }
    }

    pub fn add(&mut self, addr: IpAddr) {
        self.list.insert(addr);
    }

    pub fn remove(&mut self, addr: &IpAddr) {
        self.list.remove(&addr);
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn status(&self) -> (Vec<IpAddr>, bool) {
        let mut list: Vec<_> = self.list.iter().map(|a| *a).collect();
        list.sort();
        (list, self.enabled)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn contains(&self, addr: &IpAddr) -> bool {
        debug_assert!(self.enabled);
        debug_assert!(!addr.is_unspecified(), "{:?}", addr);
        self.list.iter().any(|filter| is_filtered(addr, filter))
    }
}

fn is_filtered(target: &IpAddr, filter: &IpAddr) -> bool {
    match (target, filter) {
        (IpAddr::V4(target), IpAddr::V4(filter)) => {
            debug_assert!(!target.is_unspecified(), "{:?}", target);
            debug_assert!(!target.is_broadcast(), "{:?}", target);
            match (target.octets(), filter.octets()) {
                (_, [0, 0, 0, 0]) => true,
                ([a0, _, _, _], [f0, 0, 0, 0]) => a0 == f0,
                ([a0, a1, _, _], [f0, f1, 0, 0]) => a0 == f0 && a1 == f1,
                ([a0, a1, a2, _], [f0, f1, f2, 0]) => a0 == f0 && a1 == f1 && a2 == f2,
                ([a0, a1, a2, a3], [f0, f1, f2, f3]) => a0 == f0 && a1 == f1 && a2 == f2 && a3 == f3,
            }
        }
        (IpAddr::V6(_), _) => unreachable!(),
        (_, IpAddr::V6(_)) => unreachable!(),
    }
}

#[cfg(test)]
mod tests_is_filtered {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn same_ip_is_filtered() {
        let ip = IpAddr::from_str("1.2.3.4").unwrap();
        let filter = IpAddr::from_str("1.2.3.4").unwrap();
        assert!(is_filtered(&ip, &filter));
    }

    #[test]
    fn broadcast_filters_the_same_prefix() {
        let ip0 = IpAddr::from_str("1.2.3.4").unwrap();
        let ip1 = IpAddr::from_str("1.2.4.4").unwrap();
        let ip2 = IpAddr::from_str("1.2.3.4").unwrap();
        let ip3 = IpAddr::from_str("1.2.7.4").unwrap();
        let ip4 = IpAddr::from_str("1.2.8.9").unwrap();
        let filter = IpAddr::from_str("1.2.0.0").unwrap();
        assert!(is_filtered(&ip0, &filter));
        assert!(is_filtered(&ip1, &filter));
        assert!(is_filtered(&ip2, &filter));
        assert!(is_filtered(&ip3, &filter));
        assert!(is_filtered(&ip4, &filter));
    }

    #[test]
    fn broadcast_does_not_filter_the_different_prefix() {
        let ip0 = IpAddr::from_str("4.2.3.4").unwrap();
        let ip1 = IpAddr::from_str("1.6.4.4").unwrap();
        let ip2 = IpAddr::from_str("7.8.3.4").unwrap();
        let ip3 = IpAddr::from_str("100.2.7.4").unwrap();
        let ip4 = IpAddr::from_str("1.21.8.9").unwrap();
        let filter = IpAddr::from_str("1.2.0.0").unwrap();
        assert!(!is_filtered(&ip0, &filter));
        assert!(!is_filtered(&ip1, &filter));
        assert!(!is_filtered(&ip2, &filter));
        assert!(!is_filtered(&ip3, &filter));
        assert!(!is_filtered(&ip4, &filter));
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn filter() {
        let mut filter = Filter::default();
        assert!(!filter.is_enabled());
        filter.enable();
        assert!(filter.is_enabled());

        filter.add(IpAddr::from_str("100.2.7.4").unwrap());

        assert!(filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));
        assert!(!filter.contains(&IpAddr::from_str("100.2.7.3").unwrap()));
    }

    #[test]
    fn remove() {
        let mut filter = Filter::default();
        assert!(!filter.is_enabled());
        filter.enable();
        assert!(filter.is_enabled());

        filter.add(IpAddr::from_str("100.2.7.4").unwrap());

        assert!(filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));

        filter.remove(&IpAddr::from_str("100.2.7.4").unwrap());
        assert!(!filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));
    }
}
