// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::BTreeMap;
use std::mem;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ccrypto::KEY_ITERATIONS;
use ckey::{Address, KeyPair, Password, Secret};
use parking_lot::{Mutex, RwLock};

use crate::account::{DecryptedAccount, SafeAccount};
use crate::accounts_dir::KeyDirectory;
use crate::json::{self, OpaqueKeyFile, Uuid};
use crate::random::Random;
use crate::{Error, SecretStore, SimpleSecretStore};

/// Accounts store.
pub struct KeyStore {
    store: KeyMultiStore,
}

impl KeyStore {
    /// Open a new accounts store with given key directory backend.
    pub fn open(directory: Box<KeyDirectory>) -> Result<Self, Error> {
        Self::open_with_iterations(directory, KEY_ITERATIONS as u32)
    }

    /// Open a new account store with given key directory backend and custom number of iterations.
    pub fn open_with_iterations(directory: Box<KeyDirectory>, iterations: u32) -> Result<Self, Error> {
        Ok(KeyStore {
            store: KeyMultiStore::open_with_iterations(directory, iterations)?,
        })
    }

    /// Modify account refresh timeout - how often they are re-read from `KeyDirectory`.
    ///
    /// Setting this to low values (or 0) will cause new accounts to be picked up quickly,
    /// although it may induce heavy disk reads and is not recommended if you manage many keys (say over 10k).
    ///
    /// By default refreshing is disabled, so only accounts created using this instance of `KeyStore` are taken into account.
    pub fn set_refresh_time(&self, time: Duration) {
        self.store.set_refresh_time(time)
    }
}

impl SimpleSecretStore for KeyStore {
    fn insert_account(&self, secret: Secret, password: &Password) -> Result<Address, Error> {
        let keypair = KeyPair::from_private(secret.into()).map_err(|_| Error::CreationFailed)?;
        if self.has_account(&keypair.address())? {
            Err(Error::AlreadyExists)
        } else {
            self.store.insert_account(secret, password)
        }
    }

    fn accounts(&self) -> Result<Vec<Address>, Error> {
        self.store.accounts()
    }

    fn has_account(&self, account: &Address) -> Result<bool, Error> {
        self.store.has_account(account)
    }

    fn remove_account(&self, account: &Address) -> Result<(), Error> {
        self.store.remove_account(account)
    }

    fn change_password(
        &self,
        account: &Address,
        old_password: &Password,
        new_password: &Password,
    ) -> Result<(), Error> {
        self.store.change_password(account, old_password, new_password)
    }

    fn export_account(&self, account: &Address, password: &Password) -> Result<OpaqueKeyFile, Error> {
        self.store.export_account(account, password)
    }

    fn decrypt_account(&self, account: &Address, password: &Password) -> Result<DecryptedAccount, Error> {
        self.store.decrypt_account(account, password)
    }
}

impl SecretStore for KeyStore {
    fn import_wallet(&self, json: &[u8], password: &Password, gen_id: bool) -> Result<Address, Error> {
        let json_keyfile =
            json::KeyFile::load(json).map_err(|err| Error::InvalidKeyFile(format!("Invalid JSON format: {}", err)))?;
        let mut safe_account = SafeAccount::from_file(json_keyfile, None, Some(password))?;

        if gen_id {
            safe_account.id = Random::random();
        }

        let secret = safe_account.crypto.secret(password).map_err(|_| Error::InvalidPassword)?;
        safe_account.address = KeyPair::from_private(secret.into())?.address();
        self.store.import(safe_account)
    }

    fn test_password(&self, account: &Address, password: &Password) -> Result<bool, Error> {
        match self.store.get_verified_account(account, password) {
            Ok(_) => Ok(true),
            Err(Error::InvalidPassword) => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn copy_account(
        &self,
        new_store: &SimpleSecretStore,
        account: &Address,
        password: &Password,
        new_password: &Password,
    ) -> Result<(), Error> {
        let secret = self.store.get_verified_account(account, password)?.secret;
        new_store.insert_account(secret, new_password)?;
        Ok(())
    }

    fn uuid(&self, account: &Address) -> Result<Uuid, Error> {
        let account = self.store.get_safe_account(account)?;
        Ok(account.id.into())
    }

    fn meta(&self, account: &Address) -> Result<String, Error> {
        let account = self.store.get_safe_account(account)?;
        Ok(account.meta.clone())
    }

    fn set_meta(&self, account_ref: &Address, meta: String) -> Result<(), Error> {
        let old = self.store.get_safe_account(account_ref)?;
        let mut safe_account = old.clone();
        safe_account.meta = meta;

        // save to file
        self.store.update(account_ref, &old, safe_account)
    }

    fn local_path(&self) -> PathBuf {
        self.store.dir.path().cloned().unwrap_or_else(PathBuf::new)
    }
}

/// Similar to `KeyStore` but may store many accounts (with different passwords) for the same `Address`
pub struct KeyMultiStore {
    dir: Box<KeyDirectory>,
    iterations: u32,
    // order lock: cache
    cache: RwLock<BTreeMap<Address, Vec<SafeAccount>>>,
    timestamp: Mutex<Timestamp>,
}

struct Timestamp {
    dir_hash: Option<u64>,
    last_checked: Instant,
    refresh_time: Duration,
}

impl KeyMultiStore {
    /// Open new multi-accounts store with given key directory backend.
    pub fn open(directory: Box<KeyDirectory>) -> Result<Self, Error> {
        Self::open_with_iterations(directory, KEY_ITERATIONS as u32)
    }

    /// Open new multi-accounts store with given key directory backend and custom number of iterations for new keys.
    pub fn open_with_iterations(directory: Box<KeyDirectory>, iterations: u32) -> Result<Self, Error> {
        let store = KeyMultiStore {
            dir: directory,
            iterations,
            cache: Default::default(),
            timestamp: Mutex::new(Timestamp {
                dir_hash: None,
                last_checked: Instant::now(),
                // by default we never refresh accounts
                refresh_time: Duration::from_secs(u64::max_value()),
            }),
        };
        store.reload_accounts()?;
        Ok(store)
    }

    /// Modify account refresh timeout - how often they are re-read from `KeyDirectory`.
    ///
    /// Setting this to low values (or 0) will cause new accounts to be picked up quickly,
    /// although it may induce heavy disk reads and is not recommended if you manage many keys (say over 10k).
    ///
    /// By default refreshing is disabled, so only accounts created using this instance of `KeyStore` are taken into account.
    pub fn set_refresh_time(&self, time: Duration) {
        self.timestamp.lock().refresh_time = time;
    }

    fn reload_if_changed(&self) -> Result<(), Error> {
        let mut last_timestamp = self.timestamp.lock();
        let now = Instant::now();
        if now - last_timestamp.last_checked > last_timestamp.refresh_time {
            let dir_hash = Some(self.dir.unique_repr()?);
            last_timestamp.last_checked = now;
            if last_timestamp.dir_hash == dir_hash {
                return Ok(())
            }
            self.reload_accounts()?;
            last_timestamp.dir_hash = dir_hash;
        }
        Ok(())
    }

    fn reload_accounts(&self) -> Result<(), Error> {
        let mut cache = self.cache.write();

        let mut new_accounts = BTreeMap::new();
        for account in self.dir.load()? {
            let account_ref = account.address;
            new_accounts.entry(account_ref).or_insert_with(Vec::new).push(account);
        }

        mem::replace(&mut *cache, new_accounts);
        Ok(())
    }

    fn get_safe_accounts(&self, account: &Address) -> Result<Vec<SafeAccount>, Error> {
        let from_cache = |account| {
            let cache = self.cache.read();

            match cache.get(account) {
                Some(accounts) if !accounts.is_empty() => Some(accounts.clone()),
                _ => None,
            }
        };

        let result = match from_cache(account) {
            Some(accounts) => Ok(accounts),
            None => {
                self.reload_if_changed()?;
                from_cache(account).ok_or(Error::InvalidAccount)
            }
        };

        assert!(if let Ok(ref result) = result {
            !result.is_empty()
        } else {
            true
        });
        result
    }

    fn get_safe_account(&self, account: &Address) -> Result<SafeAccount, Error> {
        let accounts = self.get_safe_accounts(account)?;
        Ok(accounts[0].clone())
    }

    fn get_verified_account(&self, account: &Address, password: &Password) -> Result<VerifiedAccount, Error> {
        for account in self.get_safe_accounts(account)?.into_iter() {
            match account.crypto.secret(password) {
                Ok(secret) => {
                    return Ok(VerifiedAccount {
                        account,
                        secret,
                    })
                }
                Err(Error::InvalidPassword) => continue,
                Err(err) => return Err(err),
            }
        }
        Err(Error::InvalidPassword)
    }

    fn import(&self, account: SafeAccount) -> Result<Address, Error> {
        // save to file
        let account = self.dir.insert(account)?;

        // update cache
        let account_ref = account.address;
        let mut cache = self.cache.write();
        cache.entry(account_ref).or_insert_with(Vec::new).push(account);

        Ok(account_ref)
    }

    fn update(&self, account_ref: &Address, old: &SafeAccount, new: SafeAccount) -> Result<(), Error> {
        // save to file
        let account = self.dir.update(new)?;

        // update cache
        let mut cache = self.cache.write();
        let accounts = cache.entry(*account_ref).or_insert_with(Vec::new);
        // Remove old account
        accounts.retain(|acc| acc != old);
        // And push updated to the end
        accounts.push(account);
        Ok(())
    }

    fn remove_safe_account(&self, account_ref: &Address, account: &SafeAccount) -> Result<(), Error> {
        // Remove from dir
        self.dir.remove(&account)?;

        // Remove from cache
        let mut cache = self.cache.write();
        let is_empty = {
            if let Some(accounts) = cache.get_mut(account_ref) {
                if let Some(position) = accounts.iter().position(|acc| acc == account) {
                    accounts.remove(position);
                }
                accounts.is_empty()
            } else {
                false
            }
        };

        if is_empty {
            cache.remove(account_ref);
        }

        Ok(())
    }
}

impl SimpleSecretStore for KeyMultiStore {
    fn insert_account(&self, secret: Secret, password: &Password) -> Result<Address, Error> {
        let keypair = KeyPair::from_private(secret.into()).map_err(|_| Error::CreationFailed)?;
        let id: [u8; 16] = Random::random();
        let account = SafeAccount::create(&keypair, id, password, self.iterations, "{}".to_string())?;
        self.import(account)
    }

    fn accounts(&self) -> Result<Vec<Address>, Error> {
        self.reload_if_changed()?;
        Ok(self.cache.read().keys().cloned().collect())
    }

    fn has_account(&self, account: &Address) -> Result<bool, Error> {
        match self.get_safe_accounts(account) {
            Ok(_) => Ok(true),
            Err(Error::InvalidAccount) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn remove_account(&self, account_ref: &Address) -> Result<(), Error> {
        let accounts = self.get_safe_accounts(account_ref)?;

        for account in accounts {
            self.remove_safe_account(account_ref, &account)?;
        }

        Ok(())
    }

    fn change_password(
        &self,
        account_ref: &Address,
        old_password: &Password,
        new_password: &Password,
    ) -> Result<(), Error> {
        let mut changed_any = false;
        for account in self.get_safe_accounts(account_ref)? {
            let new_account = match account.change_password(old_password, new_password, self.iterations) {
                Ok(new_account) => new_account,
                Err(Error::InvalidPassword) => continue,
                Err(err) => return Err(err),
            };
            self.update(account_ref, &account, new_account)?;
            changed_any = true
        }

        if changed_any {
            Ok(())
        } else {
            Err(Error::InvalidPassword)
        }
    }

    fn export_account(&self, account_ref: &Address, password: &Password) -> Result<OpaqueKeyFile, Error> {
        Ok(self.get_verified_account(account_ref, password)?.account.into())
    }

    fn decrypt_account(&self, account: &Address, password: &Password) -> Result<DecryptedAccount, Error> {
        Ok(DecryptedAccount::new(self.get_verified_account(account, password)?.secret))
    }
}

struct VerifiedAccount {
    account: SafeAccount,
    secret: Secret,
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use ckey::{Generator, Random};

    use super::*;
    use crate::accounts_dir::MemoryDirectory;

    fn keypair() -> KeyPair {
        Random.generate().unwrap()
    }

    fn store() -> KeyStore {
        KeyStore::open(Box::new(MemoryDirectory::default())).expect("MemoryDirectory always load successfuly; qed")
    }

    fn multi_store() -> KeyMultiStore {
        KeyMultiStore::open(Box::new(MemoryDirectory::default())).expect("MemoryDirectory always load successfuly; qed")
    }

    #[test]
    fn insert_account_successfully() {
        // given
        let store = store();
        let keypair = keypair();

        // when
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();

        // then
        assert_eq!(address, keypair.address());
        assert_eq!(Some(true), store.has_account(&address).ok(), "Should contain account.");
        assert_eq!(store.accounts().unwrap().len(), 1, "Should have one account.");
    }

    #[test]
    fn update_meta() {
        // given
        let store = store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();
        assert_eq!(&store.meta(&address).unwrap(), "{}");

        // when
        store.set_meta(&address, "meta".into()).unwrap();

        // then
        assert_eq!(&store.meta(&address).unwrap(), "meta");
        assert_eq!(store.accounts().unwrap().len(), 1);
    }

    #[test]
    fn remove_account() {
        // given
        let store = store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();

        // when
        store.remove_account(&address).unwrap();

        // then
        assert_eq!(store.accounts().unwrap().len(), 0, "Should remove account.");
    }

    #[test]
    fn return_true_if_password_is_correct() {
        // given
        let store = store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();

        // when
        let res1 = store.test_password(&address, &"x".into()).unwrap();
        let res2 = store.test_password(&address, &"test".into()).unwrap();

        assert!(!res1, "First password should be invalid.");
        assert!(res2, "Second password should be correct.");
    }

    #[test]
    fn multistore_can_have_the_same_account_twice() {
        // given
        let store = multi_store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();
        let address2 = store.insert_account(**private_key, &"xyz".into()).unwrap();
        assert_eq!(address, address2);

        // when
        assert_eq!(store.accounts().unwrap().len(), 1);
        assert!(store.remove_account(&address).is_ok());
        assert_eq!(store.accounts().unwrap().len(), 0);
    }

    #[test]
    fn copy_account() {
        // given
        let store = store();
        let multi_store = multi_store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();
        assert_eq!(multi_store.accounts().unwrap().len(), 0);

        // when
        store.copy_account(&multi_store, &address, &"test".into(), &"xyz".into()).unwrap();

        // then
        assert!(store.test_password(&address, &"test".into()).unwrap(), "First password should work for store.");
        assert!(
            multi_store
                .decrypt_account(&address, &"xyz".into())
                .map(|account| account.sign(&Default::default()))
                .is_ok(),
            "Second password should work for second store."
        );
        assert_eq!(multi_store.accounts().unwrap().len(), 1);
    }

    #[test]
    fn export_account() {
        // given
        let store = store();
        let keypair = keypair();
        let private_key = keypair.private();
        let address = store.insert_account(**private_key, &"test".into()).unwrap();

        // when
        let exported = store.export_account(&address, &"test".into());

        // then
        assert!(exported.is_ok(), "Should export single account: {:?}", exported);
    }
}
