mod buckets;
mod contact;

use self::buckets::Buckets;

const ALPHA: u8 = 3;
const B: usize = 32 * 8;
const K: u8 = 16;
const T_REFRESH: u32 = 60_000;


pub struct Kademlia {
	alpha: u8,
	k: u8,
	t_refresh: u32,
	buckets: Buckets,
}

impl Kademlia {
    pub fn new() -> Self {
        const DEFAULT_BUCKET_SIZE: u8 = 8;
        const DEFAULT_PORT: u16 = 3485;
        Kademlia {
            alpha: ALPHA,
            k: K,
            t_refresh: T_REFRESH,
            buckets: Buckets::new(DEFAULT_PORT, DEFAULT_BUCKET_SIZE),
        }
    }

    // FIXME: Implement message handler.
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;
    use super::B;
    use super::Kademlia;
    use super::contact;

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

    #[test]
    fn test_size_of_address_is_b() {
        assert_eq!(B, size_of::<contact::NodeId>() * 8);
    }
}
