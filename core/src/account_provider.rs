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

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ckey::{
    public_to_address, Address, Error as KeyError, Generator, KeyPair, Message, Password, Private, Public, Random,
    Signature,
};
use ckeystore::accounts_dir::MemoryDirectory;
use ckeystore::{Error as KeystoreError, KeyStore, SecretStore, SimpleSecretStore};
use parking_lot::RwLock;

/// Type of unlock.
#[derive(Clone, PartialEq)]
enum Unlock {
    /// If account is unlocked temporarily, it should be locked after first usage.
    OneTime,
    /// Account unlocked permanently can always sign message.
    /// Use with caution.
    Perm,
    /// Account unlocked with a timeout
    Timed(Instant),
}

/// Data associated with account.
#[derive(Clone)]
struct AccountData {
    unlock: Unlock,
    password: Password,
}

/// Signing error
#[derive(Debug)]
pub enum SignError {
    /// Account is not unlocked
    NotUnlocked,
    /// Account does not exist.
    NotFound,
    /// Key error.
    KeyError(KeyError),
    /// Keystore error.
    KeystoreError(KeystoreError),
}

impl From<KeyError> for SignError {
    fn from(e: KeyError) -> Self {
        SignError::KeyError(e)
    }
}

impl From<KeystoreError> for SignError {
    fn from(e: KeystoreError) -> Self {
        SignError::KeystoreError(e)
    }
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SignError::NotUnlocked => write!(f, "Account is locked"),
            SignError::NotFound => write!(f, "Account does not exist"),
            SignError::KeyError(e) => write!(f, "{}", e),
            SignError::KeystoreError(e) => write!(f, "{}", e),
        }
    }
}

pub type Error = KeystoreError;

pub struct AccountProvider {
    /// Unlocked account data.
    unlocked: RwLock<HashMap<Address, AccountData>>,
    keystore: RwLock<KeyStore>,
}

impl AccountProvider {
    pub fn new(keystore: KeyStore) -> Arc<Self> {
        Arc::new(Self {
            unlocked: RwLock::new(HashMap::new()),
            keystore: RwLock::new(keystore),
        })
    }

    /// Creates not disk backed provider.
    pub fn transient_provider() -> Arc<Self> {
        Arc::new(Self {
            unlocked: RwLock::new(HashMap::new()),
            keystore: RwLock::new(KeyStore::open(Box::new(MemoryDirectory::default())).unwrap()),
        })
    }

    pub fn new_account_and_public(&self, password: &Password) -> Result<(Address, Public), SignError> {
        let acc = Random.generate().expect("secp context has generation capabilities; qed");
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.keystore.write().insert_account(*private, password)?;
        Ok((address, public))
    }

    pub fn insert_account(&self, private: Private, password: &Password) -> Result<Address, SignError> {
        let acc = KeyPair::from_private(private)?;
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.keystore.write().insert_account(*private, password)?;
        Ok(address)
    }

    pub fn remove_account(&self, address: Address, password: &Password) -> Result<(), SignError> {
        Ok(self.keystore.write().remove_account(&address, password)?)
    }

    pub fn sign(&self, address: Address, password: Option<Password>, message: Message) -> Result<Signature, SignError> {
        let password = password.map(Ok).unwrap_or_else(|| self.password(&address))?;
        Ok(self.keystore.read().sign(&address, &password, &message)?)
    }

    pub fn has_account(&self, address: &Address) -> Result<bool, SignError> {
        let has = self.keystore.read().has_account(address)?;
        Ok(has)
    }

    pub fn has_public(&self, public: &Public) -> Result<bool, SignError> {
        let address = public_to_address(public);
        let has = self.keystore.read().has_account(&address)?;
        Ok(has)
    }

    pub fn get_list(&self) -> Result<Vec<Address>, SignError> {
        let addresses = self.keystore.read().accounts()?;
        Ok(addresses)
    }

    pub fn import_wallet(&self, json: &[u8], password: &Password) -> Result<Address, SignError> {
        Ok(self.keystore.write().import_wallet(json, password, false)?)
    }

    pub fn change_password(
        &self,
        address: Address,
        old_password: &Password,
        new_password: &Password,
    ) -> Result<(), SignError> {
        Ok(self.keystore.read().change_password(&address, &old_password, &new_password)?)
    }

    /// Unlocks account permanently.
    pub fn unlock_account_permanently(&self, account: Address, password: Password) -> Result<(), Error> {
        self.unlock_account(account, password, Unlock::Perm)
    }

    /// Unlocks account temporarily (for one signing).
    pub fn unlock_account_temporarily(&self, account: Address, password: Password) -> Result<(), Error> {
        self.unlock_account(account, password, Unlock::OneTime)
    }

    /// Unlocks account temporarily with a timeout.
    pub fn unlock_account_timed(&self, account: Address, password: Password, duration: Duration) -> Result<(), Error> {
        self.unlock_account(account, password, Unlock::Timed(Instant::now() + duration))
    }

    /// Helper method used for unlocking accounts.
    fn unlock_account(&self, address: Address, password: Password, unlock: Unlock) -> Result<(), Error> {
        // check if account is already unlocked permanently, if it is, do nothing
        let mut unlocked = self.unlocked.write();
        if let Some(data) = unlocked.get(&address) {
            if let Unlock::Perm = data.unlock {
                return Ok(())
            }
        }

        // verify password by signing dump message
        // result may be discarded
        let _ = self.keystore.read().sign(&address, &password, &Default::default())?;

        let data = AccountData {
            unlock,
            password,
        };

        unlocked.insert(address, data);
        Ok(())
    }

    fn password(&self, address: &Address) -> Result<Password, SignError> {
        let mut unlocked = self.unlocked.write();
        let data = unlocked.get(address).ok_or(SignError::NotUnlocked)?.clone();
        if let Unlock::OneTime = data.unlock {
            unlocked.remove(address).expect("data exists: so key must exist: qed");
        }
        if let Unlock::Timed(ref end) = data.unlock {
            if Instant::now() > *end {
                unlocked.remove(address).expect("data exists: so key must exist: qed");
                return Err(SignError::NotUnlocked)
            }
        }
        Ok(data.password)
    }
}

#[cfg(test)]
mod tests {
    use ckey::{Generator, Random};

    use super::AccountProvider;

    #[test]
    fn unlock_account_temp() {
        let kp = Random.generate().unwrap();
        let ap = AccountProvider::transient_provider();
        assert!(ap.insert_account(kp.private().clone(), &"test".into()).is_ok());
        assert!(ap.unlock_account_temporarily(kp.address(), "test1".into()).is_err());
        assert!(ap.unlock_account_temporarily(kp.address(), "test".into()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_err());
    }

    #[test]
    fn unlock_account_perm() {
        let kp = Random.generate().unwrap();
        let ap = AccountProvider::transient_provider();
        assert!(ap.insert_account(kp.private().clone(), &"test".into()).is_ok());
        assert!(ap.unlock_account_permanently(kp.address(), "test1".into()).is_err());
        assert!(ap.unlock_account_permanently(kp.address(), "test".into()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.unlock_account_temporarily(kp.address(), "test".into()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
        assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    }
}
