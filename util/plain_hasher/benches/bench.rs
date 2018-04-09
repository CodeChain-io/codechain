#![feature(test)]

extern crate plain_hasher;
extern crate test;

use plain_hasher::PlainHasher;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use test::{black_box, Bencher};

#[bench]
fn write_plain_hasher(b: &mut Bencher) {
    b.iter(|| {
        let n: u8 = black_box(100);
        (0..n).fold(PlainHasher::default(), |mut old, new| {
            let bb = black_box([new; 32]);
            old.write(&bb as &[u8]);
            old
        });
    });
}

#[bench]
fn write_default_hasher(b: &mut Bencher) {
    b.iter(|| {
        let n: u8 = black_box(100);
        (0..n).fold(DefaultHasher::default(), |mut old, new| {
            let bb = black_box([new; 32]);
            old.write(&bb as &[u8]);
            old
        });
    });
}
