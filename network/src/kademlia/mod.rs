mod routing_table;
mod contact;

use self::routing_table::RoutingTable;

const ALPHA: u8 = 3;
const B: usize = 64 * 8;
const K: u8 = 16;
const T_REFRESH: u32 = 60_000;


pub struct Kademlia {
    alpha: u8,
    k: u8,
    t_refresh: u32,
    buckets: RoutingTable,
}

impl Kademlia {
    pub fn new() -> Self {
        const DEFAULT_BUCKET_SIZE: u8 = 8;
        Kademlia {
            alpha: ALPHA,
            k: K,
            t_refresh: T_REFRESH,
            buckets: RoutingTable::new(DEFAULT_BUCKET_SIZE),
        }
    }

    // FIXME: Implement message handler.
}

#[cfg(test)]
mod tests {
    use super::Kademlia;

    #[test]
    fn test_default_alpha() {
        let kademlia = Kademlia::new();
        assert_eq!(3, kademlia.alpha);
    }

    #[test]
    fn test_default_k() {
        let kademlia = Kademlia::new();
        assert_eq!(16, kademlia.k);
    }

    #[test]
    fn test_default_t_refresh() {
        let kademlia = Kademlia::new();
        assert_eq!(60_000, kademlia.t_refresh);
    }
}
