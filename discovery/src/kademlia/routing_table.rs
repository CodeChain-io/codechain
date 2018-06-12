use std::cmp;
use std::collections::{BTreeSet, HashMap, VecDeque};

use cnetwork::{IntoSocketAddr, SocketAddr};

use super::node_id::log2_distance_between_nodes;
use super::NodeId;

pub struct RoutingTable {
    local_id: NodeId,
    buckets: HashMap<usize, Bucket>,
    bucket_size: u8,
}

impl RoutingTable {
    pub fn new(local_id: NodeId, bucket_size: u8) -> Self {
        const CAPACITY: usize = 8;
        RoutingTable {
            local_id,
            buckets: HashMap::with_capacity(CAPACITY),
            bucket_size,
        }
    }

    pub fn local_id(&self) -> NodeId {
        self.local_id
    }

    pub fn touch_contact(&mut self, node_id: NodeId) -> Option<&NodeId> {
        let index = log2_distance_between_nodes(&node_id, &self.local_id);
        // FIXME: Decide the maximum distance to node_id.
        if index == 0 {
            return None
        }
        let bucket = self.add_bucket(index);
        bucket.touch_contact(node_id)
    }

    #[allow(dead_code)]
    pub fn remove_contact(&mut self, node_id: &NodeId) -> Option<&NodeId> {
        let index = log2_distance_between_nodes(&node_id, &self.local_id);
        if index == 0 {
            return None
        }

        let bucket = self.buckets.get_mut(&index);
        bucket.and_then(|bucket| bucket.remove_contact(node_id))
    }

    fn add_bucket(&mut self, index: usize) -> &mut Bucket {
        self.buckets.entry(index).or_insert(Bucket::new(self.bucket_size))
    }

    pub fn get_closest_nodes(&self, target: &NodeId, result_limit: u8) -> Vec<NodeId> {
        let nodes = self.get_nodes_in_distance_order(target);
        nodes
            .into_iter()
            .take(cmp::min(result_limit, self.bucket_size) as usize)
            .map(|item| {
                debug_assert_ne!(target, &item.node_id);
                debug_assert_ne!(self.local_id, item.node_id);
                item.node_id
            })
            .collect()
    }

    fn get_nodes_in_distance_order(&self, target: &NodeId) -> BTreeSet<ContactWithDistance> {
        let mut result = BTreeSet::new();
        let mut max_distance = 0;
        for (_, bucket) in self.buckets.iter() {
            for i in 0..self.bucket_size {
                let node_id = bucket.node_ids.get(i as usize);
                if node_id.is_none() {
                    break
                }

                let node_id = node_id.unwrap();

                if target == node_id {
                    continue
                }

                let item = ContactWithDistance::new(node_id, target);
                if max_distance < item.distance {
                    if (self.bucket_size as usize) <= result.len() {
                        // FIXME: Remove the last item to guarantee the maximum size of return value.
                        continue
                    }
                    max_distance = item.distance;
                }
                result.insert(item);
            }
        }
        result
    }

    pub fn contains(&self, node_id: &NodeId) -> bool {
        let index = log2_distance_between_nodes(node_id, &self.local_id);
        if index == 0 {
            return false
        }

        let bucket = self.buckets.get(&index);
        match bucket.map(|bucket| bucket.contains(node_id)) {
            None => false,
            Some(has) => has,
        }
    }

    pub fn cleanup(&mut self) {
        self.buckets.retain(|_, bucket| !bucket.is_empty());
    }

    pub fn distances(&self) -> Vec<usize> {
        self.buckets.keys().cloned().collect()
    }

    pub fn get_contacts_with_distance(&self, distance: usize) -> Vec<NodeId> {
        self.buckets.get(&distance).map(|bucket| Vec::from(bucket.node_ids.clone())).unwrap_or(vec![])
    }

    pub fn remove_address(&mut self, address: &SocketAddr) {
        for bucket in self.buckets.values_mut() {
            bucket.remove_address(&address);
        }
    }

    pub fn len(&self) -> usize {
        self.buckets.values().map(|bucket| bucket.node_ids.len()).sum()
    }
}


struct Bucket {
    node_ids: VecDeque<NodeId>,
    bucket_size: u8,
}

impl Bucket {
    pub fn new(bucket_size: u8) -> Self {
        Bucket {
            node_ids: VecDeque::new(),
            bucket_size,
        }
    }

    pub fn touch_contact(&mut self, node_id: NodeId) -> Option<&NodeId> {
        self.remove_contact(&node_id);
        self.node_ids.push_back(node_id);
        self.head_if_full()
    }


    pub fn remove_contact(&mut self, node_id: &NodeId) -> Option<&NodeId> {
        self.node_ids.retain(|old_contact| old_contact != node_id);
        self.head_if_full()
    }

    fn head_if_full(&self) -> Option<&NodeId> {
        if self.node_ids.len() > self.bucket_size as usize {
            self.node_ids.front()
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.node_ids.is_empty()
    }

    fn contains(&self, node_id: &NodeId) -> bool {
        self.node_ids.contains(node_id)
    }

    fn remove_address(&mut self, address: &SocketAddr) {
        self.node_ids.retain(|node_id| &node_id.into_addr() != address);
    }
}


#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContactWithDistance {
    distance: usize,
    node_id: NodeId,
}

impl ContactWithDistance {
    pub fn new(node_id: &NodeId, target: &NodeId) -> Self {
        ContactWithDistance {
            distance: log2_distance_between_nodes(node_id, target),
            node_id: node_id.clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use cnetwork::SocketAddr;

    use super::*;

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

    fn get_node(distance_from_zero: usize) -> NodeId {
        IDS[distance_from_zero].clone().into()
    }

    fn init_routing_table(bucket_size: u8, local_index: usize) -> RoutingTable {
        let local_id = get_node(local_index);
        let mut routing_table = RoutingTable::new(local_id, bucket_size);

        for i in 0..IDS.len() {
            if i == local_index {
                continue
            }
            routing_table.touch_contact(get_node(i));
        }
        routing_table
    }

    #[test]
    fn test_size_of_closest_contacts_is_not_larger_than_bucket_size() {
        const BUCKET_SIZE: u8 = 5;
        let routing_table = init_routing_table(BUCKET_SIZE, 0);

        let closest_contacts = routing_table.get_closest_nodes(&get_node(4), BUCKET_SIZE);
        assert!(closest_contacts.len() <= (BUCKET_SIZE as usize));
    }

    #[test]
    fn test_closest_contacts_1() {
        const BUCKET_SIZE: u8 = 5;
        let routing_table = init_routing_table(BUCKET_SIZE, 0);

        let closest_contacts = routing_table.get_closest_nodes(&get_node(4), BUCKET_SIZE);
        assert_eq!(BUCKET_SIZE as usize, closest_contacts.len());
        assert_eq!(get_node(1), closest_contacts[0]);
        assert_eq!(get_node(6), closest_contacts[1]);
        assert_eq!(get_node(2), closest_contacts[2]);
        assert_eq!(get_node(3), closest_contacts[3]);
        assert_eq!(get_node(5), closest_contacts[4]);
    }

    #[test]
    fn test_closest_contacts_2() {
        const BUCKET_SIZE: u8 = 5;
        let routing_table = init_routing_table(BUCKET_SIZE, 0);

        let closest_contacts = routing_table.get_closest_nodes(&get_node(3), BUCKET_SIZE);
        assert_eq!(BUCKET_SIZE as usize, closest_contacts.len());
        assert_eq!(get_node(2), closest_contacts[0]);
        assert_eq!(get_node(1), closest_contacts[1]);
        assert_eq!(get_node(4), closest_contacts[2]);
        assert_eq!(get_node(6), closest_contacts[3]);
        assert_eq!(get_node(5), closest_contacts[4]);
    }

    #[test]
    fn test_closest_contacts_must_not_contain_target() {
        use std::u8;
        debug_assert!(IDS.len() <= (u8::MAX as usize));
        let bucket_size = IDS.len() as u8;
        let routing_table = init_routing_table(bucket_size, 0);

        const TARGET_INDEX: usize = 3;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), bucket_size);
        assert!(!closest_contacts.contains(&get_node(TARGET_INDEX)));
        assert!(2 <= IDS.len());
        let number_of_contacts_except_local = IDS.len() - 1;
        let number_of_contacts_except_local_and_target = number_of_contacts_except_local - 1;
        assert_eq!(number_of_contacts_except_local_and_target, closest_contacts.len());
    }

    #[test]
    fn test_closest_contacts_must_not_contain_removed() {
        use std::u8;
        debug_assert!(IDS.len() <= (u8::MAX as usize));
        let bucket_size = IDS.len() as u8;
        let mut routing_table = init_routing_table(bucket_size, 0);

        const KILLED_INDEX: usize = 4;
        routing_table.remove_contact(&get_node(KILLED_INDEX));

        const TARGET_INDEX: usize = 5;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), bucket_size);
        assert!(!closest_contacts.contains(&get_node(KILLED_INDEX)));
    }

    #[test]
    fn test_closest_contacts_takes_the_limit() {
        use std::u8;
        debug_assert!(IDS.len() <= (u8::MAX as usize));
        let bucket_size = IDS.len() as u8;
        let routing_table = init_routing_table(bucket_size, 0);

        const TARGET_INDEX: usize = 5;

        const RESULT_LIMIT3: u8 = 3;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), RESULT_LIMIT3);
        assert_eq!(RESULT_LIMIT3 as usize, closest_contacts.len());

        const RESULT_LIMIT2: u8 = 2;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), RESULT_LIMIT2);
        assert_eq!(RESULT_LIMIT2 as usize, closest_contacts.len());

        const RESULT_LIMIT7: u8 = 7;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), RESULT_LIMIT7);
        assert_eq!(RESULT_LIMIT7 as usize, closest_contacts.len());

        const RESULT_LIMIT5: u8 = 5;
        let closest_contacts = routing_table.get_closest_nodes(&get_node(TARGET_INDEX), RESULT_LIMIT5);
        assert_eq!(RESULT_LIMIT5 as usize, closest_contacts.len());
    }

    #[test]
    fn test_get_contacts_with_distance() {
        const BUCKET_SIZE: u8 = 5;
        let routing_table = init_routing_table(BUCKET_SIZE, 0);

        assert_eq!(0, routing_table.get_contacts_with_distance(1).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(2).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(3).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(4).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(5).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(6).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(7).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(8).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(9).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(10).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(11).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(12).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(13).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(14).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(15).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(16).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(17).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(18).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(19).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(20).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(21).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(22).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(23).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(24).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(25).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(26).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(27).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(28).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(29).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(30).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(31).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(32).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(33).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(34).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(35).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(36).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(37).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(38).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(39).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(40).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(41).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(42).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(43).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(44).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(45).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(46).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(47).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(48).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(49).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(50).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(51).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(52).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(53).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(54).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(55).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(56).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(57).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(58).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(59).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(60).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(61).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(62).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(63).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(64).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(65).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(66).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(67).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(68).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(69).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(70).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(71).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(72).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(73).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(74).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(75).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(76).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(77).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(78).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(79).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(80).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(81).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(82).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(83).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(84).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(85).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(86).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(87).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(88).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(89).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(90).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(91).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(92).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(93).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(94).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(95).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(96).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(97).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(98).len());
        assert_eq!(0, routing_table.get_contacts_with_distance(99).len());
    }
}
