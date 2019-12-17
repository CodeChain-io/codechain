// Copyright 2019 Kodebox, Inc.
// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of CodeChain.
//
// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::{fmt, str};

/// Journal database operating strategy.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Algorithm {
    /// Keep all keys forever.
    Archive,
}

impl Default for Algorithm {
    fn default() -> Algorithm {
        Algorithm::Archive
    }
}

impl str::FromStr for Algorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "archive" => Ok(Algorithm::Archive),
            e => Err(format!("Invalid algorithm: {}", e)),
        }
    }
}

impl Algorithm {
    /// Returns true if pruning strategy is stable
    pub fn is_stable(self) -> bool {
        match self {
            Algorithm::Archive => true,
        }
    }

    /// Returns all algorithm types.
    pub fn all_types() -> Vec<Algorithm> {
        vec![Algorithm::Archive]
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Algorithm::Archive => write!(f, "archive"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Algorithm;

    #[test]
    fn journal_algorithm_parsing() {
        assert_eq!(Algorithm::Archive, "archive".parse().unwrap());
    }

    #[test]
    fn journal_algorithm_printing() {
        assert_eq!(Algorithm::Archive.to_string(), "archive".to_string());
    }

    #[test]
    fn journal_algorithm_is_stable() {
        assert!(Algorithm::Archive.is_stable());
    }

    #[test]
    fn journal_algorithm_default() {
        assert_eq!(Algorithm::default(), Algorithm::Archive);
    }

    #[test]
    fn journal_algorithm_all_types() {
        // compiling should fail if some cases are not covered
        let mut archive = 0;

        for a in &Algorithm::all_types() {
            match *a {
                Algorithm::Archive => archive += 1,
            }
        }

        assert_eq!(archive, 1);
    }
}
