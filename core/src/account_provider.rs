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
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ckey::{public_to_address, Address, Error as KeyError, Generator, KeyPair, Password, Private, Public, Random};
use ckeystore::accounts_dir::MemoryDirectory;
use ckeystore::{DecryptedAccount, Error as KeystoreError, KeyStore, SecretStore, SimpleSecretStore};
use parking_lot::RwLock;
use vrf::openssl::Error as VRFError;

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
struct UnlockedPassword {
    unlock: Unlock,
    password: Password,
}

/// Signing error
#[derive(Debug)]
pub enum Error {
    /// Account is not unlocked
    NotUnlocked,
    /// Account does not exist.
    NotFound,
    /// Key error.
    KeyError(KeyError),
    /// Keystore error.
    KeystoreError(KeystoreError),
    /// VRF error,
    VRFError(VRFError),
}

impl From<KeyError> for Error {
    fn from(e: KeyError) -> Self {
        Error::KeyError(e)
    }
}

impl From<KeystoreError> for Error {
    fn from(e: KeystoreError) -> Self {
        Error::KeystoreError(e)
    }
}

impl From<VRFError> for Error {
    fn from(e: VRFError) -> Self {
        Error::VRFError(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Error::NotUnlocked => write!(f, "Account is locked"),
            Error::NotFound => write!(f, "Account does not exist"),
            Error::KeyError(e) => write!(f, "{}", e),
            Error::KeystoreError(e) => write!(f, "{}", e),
            Error::VRFError(e) => write!(f, "{}", e),
        }
    }
}

pub struct AccountProvider {
    /// Unlocked account data.
    unlocked: RwLock<HashMap<Address, UnlockedPassword>>,
    keystore: KeyStore,
}

impl AccountProvider {
    pub fn new(keystore: KeyStore) -> Arc<Self> {
        Arc::new(Self {
            unlocked: RwLock::new(HashMap::new()),
            keystore,
        })
    }

    /// Creates not disk backed provider.
    pub fn transient_provider() -> Arc<Self> {
        Arc::new(Self {
            unlocked: RwLock::new(HashMap::new()),
            keystore: KeyStore::open(Box::new(MemoryDirectory::default())).unwrap(),
        })
    }

    pub fn new_account_and_public(&self, password: &Password) -> Result<(Address, Public), Error> {
        let acc = Random.generate().expect("secp context has generation capabilities; qed");
        self.insert_account_internal(&acc, password)
    }

    pub fn insert_account(&self, private: Private, password: &Password) -> Result<Address, Error> {
        let acc = KeyPair::from_private(private)?;
        self.insert_account_internal(&acc, password).map(|(addr, _)| addr)
    }

    fn insert_account_internal(&self, acc: &KeyPair, password: &Password) -> Result<(Address, Public), Error> {
        let private = *acc.private();
        let public = *acc.public();
        let address = public_to_address(&public);
        self.keystore.insert_account(*private, password)?;
        Ok((address, public))
    }

    pub fn remove_account(&self, address: Address) -> Result<(), Error> {
        self.keystore.remove_account(&address)?;
        Ok(())
    }

    pub fn has_account(&self, address: &Address) -> Result<bool, Error> {
        let has = self.keystore.has_account(address)?;
        Ok(has)
    }

    pub fn has_public(&self, public: &Public) -> Result<bool, Error> {
        let address = public_to_address(public);
        let has = self.keystore.has_account(&address)?;
        Ok(has)
    }

    pub fn get_list(&self) -> Result<Vec<Address>, Error> {
        let addresses = self.keystore.accounts()?;
        Ok(addresses)
    }

    pub fn import_wallet(&self, json: &[u8], password: &Password) -> Result<Address, Error> {
        Ok(self.keystore.import_wallet(json, password, false)?)
    }

    pub fn change_password(
        &self,
        address: Address,
        old_password: &Password,
        new_password: &Password,
    ) -> Result<(), Error> {
        self.keystore.change_password(&address, &old_password, &new_password)?;
        Ok(())
    }

    /// Unlocks account permanently.
    pub fn unlock_account_permanently(&self, account: Address, password: Password) -> Result<(), KeystoreError> {
        self.unlock_account(account, password, Unlock::Perm)
    }

    /// Unlocks account temporarily (for one signing).
    pub fn unlock_account_temporarily(&self, account: Address, password: Password) -> Result<(), KeystoreError> {
        self.unlock_account(account, password, Unlock::OneTime)
    }

    /// Unlocks account temporarily with a timeout.
    pub fn unlock_account_timed(
        &self,
        account: Address,
        password: Password,
        duration: Duration,
    ) -> Result<(), KeystoreError> {
        self.unlock_account(account, password, Unlock::Timed(Instant::now() + duration))
    }

    /// Helper method used for unlocking accounts.
    fn unlock_account(&self, address: Address, password: Password, unlock: Unlock) -> Result<(), KeystoreError> {
        // check if account is already unlocked permanently, if it is, do nothing
        let mut unlocked = self.unlocked.write();
        if let Some(data) = unlocked.get(&address) {
            if let Unlock::Perm = data.unlock {
                return Ok(())
            }
        }

        if !self.keystore.test_password(&address, &password)? {
            return Err(KeystoreError::InvalidPassword)
        }

        let unlocked_account = UnlockedPassword {
            unlock,
            password,
        };

        unlocked.insert(address, unlocked_account);
        Ok(())
    }

    pub fn get_unlocked_account(&self, address: &Address) -> Result<ScopedAccount, Error> {
        let mut unlocked = self.unlocked.write();
        let data = unlocked.get(address).ok_or(Error::NotUnlocked)?.clone();
        if let Unlock::OneTime = data.unlock {
            unlocked.remove(address).expect("data exists: so key must exist: qed");
        }
        if let Unlock::Timed(ref end) = data.unlock {
            if Instant::now() > *end {
                unlocked.remove(address).expect("data exists: so key must exist: qed");
                return Err(Error::NotUnlocked)
            }
        }

        let decrypted = self.decrypt_account(address, &data.password)?;
        Ok(ScopedAccount::from(decrypted))
    }

    fn decrypt_account(&self, address: &Address, password: &Password) -> Result<DecryptedAccount, KeystoreError> {
        self.keystore.decrypt_account(address, password)
    }

    pub fn get_account(&self, address: &Address, password: Option<&Password>) -> Result<ScopedAccount, Error> {
        match password {
            Some(password) => Ok(ScopedAccount::from(self.decrypt_account(address, password)?)),
            None => self.get_unlocked_account(address),
        }
    }
}

// UnlockedAccount should have limited lifetime
pub struct ScopedAccount<'a> {
    decrypted: DecryptedAccount,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Deref for ScopedAccount<'a> {
    type Target = DecryptedAccount;

    fn deref(&self) -> &DecryptedAccount {
        &self.decrypted
    }
}

impl<'a> ScopedAccount<'a> {
    fn from(decrypted: DecryptedAccount) -> ScopedAccount<'a> {
        ScopedAccount {
            decrypted,
            phantom: PhantomData::default(),
        }
    }

    pub fn disclose(self) -> DecryptedAccount {
        self.decrypted
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
        assert!(ap.insert_account(*kp.private(), &"test".into()).is_ok());
        assert!(ap.unlock_account_temporarily(kp.address(), "test1".into()).is_err());
        assert!(ap.unlock_account_temporarily(kp.address(), "test".into()).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_err());
    }

    #[test]
    fn unlock_account_perm() {
        let kp = Random.generate().unwrap();
        let ap = AccountProvider::transient_provider();
        assert!(ap.insert_account(*kp.private(), &"test".into()).is_ok());
        assert!(ap.unlock_account_permanently(kp.address(), "test1".into()).is_err());
        assert!(ap.unlock_account_permanently(kp.address(), "test".into()).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_ok());
        assert!(ap.unlock_account_temporarily(kp.address(), "test".into()).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_ok());
        assert!(ap.get_account(&kp.address(), None).is_ok());
    }
}
