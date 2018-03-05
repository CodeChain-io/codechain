use kademlia::contact::Contact;
use std::collections::HashMap;
use std::collections::LinkedList;
use std::collections::linked_list::Iter;

pub struct Buckets {
    localhost: Contact,
    buckets: HashMap<usize, Bucket>,
    bucket_size: u8,
}

impl Buckets {
    pub fn new(port: u16, bucket_size: u8) -> Self {
        let capacity = 8;
        Buckets {
            localhost: Contact::localhost(port),
            buckets: HashMap::with_capacity(capacity),
            bucket_size,
        }
    }

    pub fn add_contact(&mut self, contact: Contact) {
        let index = self.localhost.log2_distance(&contact);
        // FIXME: Decide the maximum distance to contact.
        if index == 0 {
            return;
        }
        let ref mut bucket = self.buckets.entry(index).or_insert(Bucket::new(self.bucket_size));
        bucket.add_contact(contact);
    }

    pub fn get_contacts(&self, index: usize) -> Option<Iter<Contact>> {
        self.buckets.get(&index).map(|bucket| bucket.contacts.iter())
    }
}

struct Bucket {
	contacts: LinkedList<Contact>,
	bucket_size: u8,
}

impl Bucket {
	pub fn new(bucket_size: u8) -> Self {
		Bucket {
			contacts: LinkedList::new(),
			bucket_size,
		}
	}

	pub fn add_contact(&mut self, contact: Contact) {
		// FIXME: Check bucket_size
		self.contacts.push_back(contact);
	}
}
