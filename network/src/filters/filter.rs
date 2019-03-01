// Copyright 2018-2019 Kodebox, Inc.
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

use std::collections::HashMap;
use std::net::IpAddr;

use cidr::{Cidr, IpCidr};

#[derive(Default)]
pub struct Filter {
    enabled: bool,
    list: HashMap<IpCidr, String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FilterEntry {
    pub cidr: IpCidr,
    pub tag: String,
}

impl Filter {
    pub fn new(input_vector: Vec<FilterEntry>) -> Self {
        Self {
            enabled: !input_vector.is_empty(),
            list: input_vector.into_iter().map(|x| (x.cidr, x.tag)).collect(),
        }
    }

    pub fn add(&mut self, addr: IpCidr, tag: Option<String>) {
        match tag {
            Some(tag) => {
                self.list.insert(addr, tag);
            }
            None => {
                self.list.entry(addr).or_insert_with(String::new);
            }
        };
    }

    pub fn remove(&mut self, addr: &IpCidr) {
        self.list.remove(&addr);
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn status(&self) -> (Vec<FilterEntry>, bool) {
        let mut list: Vec<_> = self
            .list
            .iter()
            .map(|(a, b)| FilterEntry {
                cidr: a.clone(),
                tag: b.clone(),
            })
            .collect();
        list.sort();
        (list, self.enabled)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn contains(&self, addr: &IpAddr) -> bool {
        debug_assert!(self.enabled);
        debug_assert!(!addr.is_unspecified(), "{:?}", addr);
        self.list.iter().any(|(filter, _)| is_filtered(addr, filter))
    }
}

pub fn is_filtered(target: &IpAddr, filter: &IpCidr) -> bool {
    debug_assert!(!target.is_unspecified(), "{:?}", target);
    if let IpAddr::V4(target_inner) = target {
        debug_assert!(!target_inner.is_broadcast(), "{:?}", target);
    }

    filter.contains(target)
}


#[cfg(test)]
mod tests_is_filtered {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn cidr_without_prefix_filter_single_address() {
        let ip0 = IpAddr::from_str("1.2.3.4").unwrap();
        let ip1 = IpAddr::from_str("1.2.3.3").unwrap();
        let ip2 = IpAddr::from_str("1.2.3.5").unwrap();
        let filter = IpCidr::from_str("1.2.3.4").unwrap();
        assert!(is_filtered(&ip0, &filter));
        assert!(!is_filtered(&ip1, &filter));
        assert!(!is_filtered(&ip2, &filter));
    }

    #[test]
    fn cidr_with_suffix_filters_the_same_prefix() {
        let ip0 = IpAddr::from_str("1.2.3.4").unwrap();
        let ip1 = IpAddr::from_str("1.2.4.4").unwrap();
        let ip2 = IpAddr::from_str("1.2.3.4").unwrap();
        let ip3 = IpAddr::from_str("1.2.7.4").unwrap();
        let ip4 = IpAddr::from_str("1.2.8.9").unwrap();
        let filter = IpCidr::from_str("1.2.0.0/16").unwrap();
        assert!(is_filtered(&ip0, &filter));
        assert!(is_filtered(&ip1, &filter));
        assert!(is_filtered(&ip2, &filter));
        assert!(is_filtered(&ip3, &filter));
        assert!(is_filtered(&ip4, &filter));
    }

    #[test]
    fn cidr_with_suffix_partial_cover() {
        let ip0 = IpAddr::from_str("1.2.3.3").unwrap();
        let ip1 = IpAddr::from_str("1.2.3.255").unwrap();
        let ip2 = IpAddr::from_str("1.2.4.7").unwrap();
        let ip3 = IpAddr::from_str("1.2.7.4").unwrap();
        let ip4 = IpAddr::from_str("1.2.8.9").unwrap();
        let filter = IpCidr::from_str("1.2.0.0/22").unwrap();
        assert!(is_filtered(&ip0, &filter));
        assert!(is_filtered(&ip1, &filter));
        assert!(!is_filtered(&ip2, &filter));
        assert!(!is_filtered(&ip3, &filter));
        assert!(!is_filtered(&ip4, &filter));
    }

    #[test]
    fn cidr_with_suffix_does_not_filter_the_different_prefix() {
        let ip0 = IpAddr::from_str("4.2.3.4").unwrap();
        let ip1 = IpAddr::from_str("1.6.4.4").unwrap();
        let ip2 = IpAddr::from_str("7.8.3.4").unwrap();
        let ip3 = IpAddr::from_str("100.2.7.4").unwrap();
        let ip4 = IpAddr::from_str("1.21.8.9").unwrap();
        let filter = IpCidr::from_str("1.2.0.0/16").unwrap();
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

        filter.add(IpCidr::from_str("100.2.7.4").unwrap(), None);

        assert!(filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));
        assert!(!filter.contains(&IpAddr::from_str("100.2.7.3").unwrap()));
    }

    #[test]
    fn remove() {
        let mut filter = Filter::default();
        assert!(!filter.is_enabled());
        filter.enable();
        assert!(filter.is_enabled());

        filter.add(IpCidr::from_str("100.2.7.4").unwrap(), None);
        filter.add(IpCidr::from_str("100.2.7.4").unwrap(), Some("ABC".to_string()));

        assert!(filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));

        filter.remove(&IpCidr::from_str("100.2.7.4").unwrap());
        assert!(!filter.contains(&IpAddr::from_str("100.2.7.4").unwrap()));
    }
}
