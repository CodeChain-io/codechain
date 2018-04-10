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

use std::ops::{Drop, FnMut};

pub struct Finally<F>
where
    F: FnMut(), {
    f: F,
}

impl<F> Drop for Finally<F>
where
    F: FnMut(),
{
    fn drop(&mut self) {
        (self.f)();
    }
}

pub fn finally<F>(f: F) -> Finally<F>
where
    F: FnMut(), {
    Finally {
        f,
    }
}

#[cfg(test)]
mod tests {
    use super::finally;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn test_finally() {
        let a = AtomicUsize::new(0);
        {
            let _f = finally(|| {
                a.fetch_add(1, Ordering::SeqCst);
            });
            assert_eq!(0, a.load(Ordering::SeqCst));
        }
        assert_eq!(1, a.load(Ordering::SeqCst));
    }
}
