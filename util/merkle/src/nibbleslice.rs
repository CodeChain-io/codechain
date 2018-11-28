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
use std::cmp::*;
use std::fmt;

use elastic_array::ElasticArray36;


#[derive(Eq, Ord)]
pub struct NibbleSlice<'a> {
    pub data: &'a [u8],
    pub offset: usize,
}

impl<'a, 'view> NibbleSlice<'a>
where
    'a: 'view,
{
    /// Create a new nibble slice with the given byte-slice.
    pub fn new(data: &'a [u8]) -> Self {
        NibbleSlice::new_offset(data, 0)
    }

    /// Create a new nibble slice with the given byte-slice with a nibble offset.
    pub fn new_offset(data: &'a [u8], offset: usize) -> Self {
        NibbleSlice {
            data,
            offset,
        }
    }

    /// Create a new nibble slice from the given HPE encoded data (e.g. output of `encoded()`).
    pub fn from_encoded(data: &'a [u8]) -> NibbleSlice {
        let offset = if data[0] & 16 == 16 {
            1
        } else {
            2
        };
        Self::new_offset(data, offset)
    }

    /// Is this an empty slice?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the length (in nibbles, naturally) of this slice.
    pub fn len(&self) -> usize {
        self.data.len() * 2 - self.offset
    }

    /// Get the nibble at position `i`.
    pub fn at(&self, i: usize) -> u8 {
        if (self.offset + i) & 1 == 1 {
            self.data[(self.offset + i) / 2] & 15u8
        } else {
            self.data[(self.offset + i) / 2] >> 4
        }
    }

    /// Return object which represents a view on to this slice (further) offset by `i` nibbles.
    pub fn mid(&'view self, i: usize) -> NibbleSlice<'a> {
        NibbleSlice {
            data: self.data,
            offset: self.offset + i,
        }
    }

    /// Do we start with the same nibbles as the whole of `them`?
    pub fn starts_with(&self, them: &Self) -> bool {
        self.common_prefix(them) == them.len()
    }

    /// How many of the same nibbles at the beginning do we match with `them`?
    pub fn common_prefix(&self, them: &Self) -> usize {
        let s = min(self.len(), them.len());
        let mut i = 0usize;
        while i < s {
            if self.at(i) != them.at(i) {
                break
            }
            i += 1;
        }
        i
    }

    /// Encode while nibble slice in prefixed hex notation, noting whether it `is_leaf`.
    pub fn encoded(&self) -> ElasticArray36<u8> {
        let l = self.len();
        let mut r = ElasticArray36::new();
        let mut i = l % 2;
        r.push(if i == 1 {
            0x10 + self.at(0)
        } else {
            0
        });
        while i < l {
            r.push(self.at(i) * 16 + self.at(i + 1));
            i += 2;
        }
        r
    }


    /// Encode only the leftmost `n` bytes of the nibble slice in prefixed hex notation,
    /// noting whether it `is_leaf`.
    pub fn encoded_leftmost(&self, n: usize) -> ElasticArray36<u8> {
        let l = min(self.len(), n);
        let mut r = ElasticArray36::new();
        let mut i = l % 2;
        r.push(if i == 1 {
            0x10 + self.at(0)
        } else {
            0
        });
        while i < l {
            r.push(self.at(i) * 16 + self.at(i + 1));
            i += 2;
        }
        r
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec: Vec<u8> = Vec::new();
        for i in 0..self.len() {
            vec.push(self.at(i));
        }
        vec
    }

    pub fn from_vec(v: &[u8]) -> (ElasticArray36<u8>, usize) {
        let mut r = ElasticArray36::new();
        let l = v.len();
        let mut i = l % 2;
        r.push(if i == 1 {
            0x10 + (v[0] & 15u8)
        } else {
            0
        });
        while i < l {
            r.push(((v[i] & 15u8) << 4) + (v[i + 1] & 15u8));
            i += 2;
        }
        (r, 2 - (l % 2))
    }
}

impl<'a> PartialEq for NibbleSlice<'a> {
    fn eq(&self, them: &Self) -> bool {
        self.len() == them.len() && self.starts_with(them)
    }
}

impl<'a> PartialOrd for NibbleSlice<'a> {
    fn partial_cmp(&self, them: &Self) -> Option<Ordering> {
        let s = min(self.len(), them.len());
        let mut i = 0usize;
        while i < s {
            match self.at(i).partial_cmp(&them.at(i)).unwrap() {
                Ordering::Less => return Some(Ordering::Less),
                Ordering::Greater => return Some(Ordering::Greater),
                _ => i += 1,
            }
        }
        self.len().partial_cmp(&them.len())
    }
}

impl<'a> fmt::Debug for NibbleSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in 0..self.len() {
            match i {
                0 => write!(f, "{:01x}", self.at(i))?,
                _ => write!(f, "'{:01x}", self.at(i))?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NibbleSlice;
    use elastic_array::ElasticArray36;

    static D: &'static [u8; 3] = &[0x01u8, 0x23, 0x45];

    #[test]
    fn basics() {
        let n = NibbleSlice::new(D);
        assert_eq!(n.len(), 6);
        assert!(!n.is_empty());

        let n = NibbleSlice::new_offset(D, 6);
        assert!(n.is_empty());

        let n = NibbleSlice::new_offset(D, 3);
        assert_eq!(n.len(), 3);
        for i in 0..3 {
            assert_eq!(n.at(i), i as u8 + 3);
        }
    }

    #[test]
    fn mid() {
        let n = NibbleSlice::new(D);
        let m = n.mid(2);
        for i in 0..4 {
            assert_eq!(m.at(i), i as u8 + 2);
        }
        let m = n.mid(3);
        for i in 0..3 {
            assert_eq!(m.at(i), i as u8 + 3);
        }
    }

    #[test]
    fn encoded() {
        let n = NibbleSlice::new(D);
        assert_eq!(n.encoded(), ElasticArray36::from_slice(&[0x00, 0x01, 0x23, 0x45]));
        assert_eq!(n.mid(1).encoded(), ElasticArray36::from_slice(&[0x11, 0x23, 0x45]));
    }

    #[test]
    fn from_encoded() {
        let n = NibbleSlice::new(D);
        assert_eq!(n, NibbleSlice::from_encoded(&[0x00, 0x01, 0x23, 0x45]));
        assert_eq!(n.mid(1), NibbleSlice::from_encoded(&[0x11, 0x23, 0x45]));
    }

    #[test]
    fn shared() {
        let n = NibbleSlice::new(D);

        let other = &[0x01u8, 0x23, 0x01, 0x23, 0x45, 0x67];
        let m = NibbleSlice::new(other);

        assert_eq!(n.common_prefix(&m), 4);
        assert_eq!(m.common_prefix(&n), 4);
        assert_eq!(n.mid(1).common_prefix(&m.mid(1)), 3);
        assert_eq!(n.mid(1).common_prefix(&m.mid(2)), 0);
        assert_eq!(n.common_prefix(&m.mid(4)), 6);
        assert!(!n.starts_with(&m.mid(4)));
        assert!(m.mid(4).starts_with(&n));
    }

    #[test]
    fn compare() {
        let other = &[0x01u8, 0x23, 0x01, 0x23, 0x45];
        let n = NibbleSlice::new(D);
        let m = NibbleSlice::new(other);

        assert!(n != m);
        assert!(n > m);
        assert!(m < n);

        assert!(n == m.mid(4));
        assert!(n >= m.mid(4));
        assert!(n <= m.mid(4));
    }
}
