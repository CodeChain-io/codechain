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

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use time;

use super::super::json::Uuid;
use super::super::{json, Error, SafeAccount};
use super::KeyDirectory;

const IGNORED_FILES: &'static [&'static str] = &["thumbs.db"];

#[cfg(not(windows))]
fn restrict_permissions_to_owner(file_path: &Path) -> Result<(), i32> {
    use libc;
    use std::ffi;

    let cstr = ffi::CString::new(&*file_path.to_string_lossy()).map_err(|_| -1)?;
    match unsafe { libc::chmod(cstr.as_ptr(), libc::S_IWUSR | libc::S_IRUSR) } {
        0 => Ok(()),
        x => Err(x),
    }
}

#[cfg(windows)]
fn restrict_permissions_to_owner(_file_path: &Path) -> Result<(), i32> {
    Ok(())
}

/// Root keys directory implementation
pub type RootDiskDirectory = DiskDirectory<DiskKeyFileManager>;

/// Disk directory key file manager
pub trait KeyFileManager: Send + Sync {
    /// Read `SafeAccount` from given key file stream
    fn read<T>(&self, filename: Option<String>, reader: T) -> Result<SafeAccount, Error>
    where
        T: io::Read;
    /// Write `SafeAccount` to given key file stream
    fn write<T>(&self, account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where
        T: io::Write;
}

/// Disk-based keys directory implementation
pub struct DiskDirectory<T>
where
    T: KeyFileManager, {
    path: PathBuf,
    key_manager: T,
}

/// Keys file manager for root keys directory
pub struct DiskKeyFileManager;

impl RootDiskDirectory {
    pub fn create<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>, {
        fs::create_dir_all(&path)?;
        Ok(Self::at(path))
    }

    pub fn at<P>(path: P) -> Self
    where
        P: AsRef<Path>, {
        DiskDirectory::new(path, DiskKeyFileManager)
    }
}

impl<T> DiskDirectory<T>
where
    T: KeyFileManager,
{
    /// Create new disk directory instance
    pub fn new<P>(path: P, key_manager: T) -> Self
    where
        P: AsRef<Path>, {
        DiskDirectory {
            path: path.as_ref().to_path_buf(),
            key_manager,
        }
    }

    fn files(&self) -> Result<Vec<PathBuf>, Error> {
        Ok(fs::read_dir(&self.path)?
            .flat_map(Result::ok)
            .filter(|entry| {
                let metadata = entry.metadata().ok();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                // filter directories
                metadata.map_or(false, |m| !m.is_dir()) &&
					// hidden files
					!name.starts_with(".") &&
					// other ignored files
					!IGNORED_FILES.contains(&&*name)
            })
            .map(|entry| entry.path())
            .collect::<Vec<PathBuf>>())
    }

    pub fn files_hash(&self) -> Result<u64, Error> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        let files = self.files()?;
        for file in files {
            hasher.write(file.to_str().unwrap_or("").as_bytes())
        }

        Ok(hasher.finish())
    }

    fn last_modification_date(&self) -> Result<u64, Error> {
        use std::time::{Duration, UNIX_EPOCH};
        let duration = fs::metadata(&self.path)?.modified()?.duration_since(UNIX_EPOCH).unwrap_or(Duration::default());
        let timestamp = duration.as_secs() ^ (duration.subsec_nanos() as u64);
        Ok(timestamp)
    }

    /// all accounts found in keys directory
    fn files_content(&self) -> Result<HashMap<PathBuf, SafeAccount>, Error> {
        // it's not done using one iterator cause
        // there is an issue with rustc and it takes tooo much time to compile
        let paths = self.files()?;
        Ok(paths
            .into_iter()
            .filter_map(|path| {
                let filename = Some(
                    path.file_name().and_then(|n| n.to_str()).expect("Keys have valid UTF8 names only.").to_owned(),
                );
                fs::File::open(path.clone())
                    .map_err(Into::into)
                    .and_then(|file| self.key_manager.read(filename, file))
                    .map_err(|e| {
                        warn!("Invalid key file: {:?} ({})", path, e);
                        e
                    })
                    .map(|account| (path, account))
                    .ok()
            })
            .collect())
    }


    /// insert account with given filename. if the filename is a duplicate of any stored account and dedup is set to
    /// true, a random suffix is appended to the filename.
    pub fn insert_with_filename(
        &self,
        account: SafeAccount,
        mut filename: String,
        dedup: bool,
    ) -> Result<SafeAccount, Error> {
        // path to keyfile
        let mut keyfile_path = self.path.join(filename.as_str());

        // check for duplicate filename and append random suffix
        if dedup && keyfile_path.exists() {
            let suffix = ::random::random_string(4);
            filename.push_str(&format!("-{}", suffix));
            keyfile_path.set_file_name(&filename);
        }

        // update account filename
        let original_account = account.clone();
        let mut account = account;
        account.filename = Some(filename);

        {
            // save the file
            let mut file = fs::File::create(&keyfile_path)?;

            // write key content
            self.key_manager.write(original_account, &mut file).map_err(|e| Error::Custom(format!("{:?}", e)))?;

            file.flush()?;

            if let Err(_) = restrict_permissions_to_owner(keyfile_path.as_path()) {
                return Err(Error::Io(io::Error::last_os_error()))
            }

            file.sync_all()?;
        }

        Ok(account)
    }

    /// Get key file manager referece
    pub fn key_manager(&self) -> &T {
        &self.key_manager
    }
}

impl<T> KeyDirectory for DiskDirectory<T>
where
    T: KeyFileManager,
{
    fn load(&self) -> Result<Vec<SafeAccount>, Error> {
        let accounts = self.files_content()?.into_iter().map(|(_, account)| account).collect();
        Ok(accounts)
    }

    fn update(&self, account: SafeAccount) -> Result<SafeAccount, Error> {
        // Disk store handles updates correctly iff filename is the same
        let filename = account_filename(&account);
        self.insert_with_filename(account, filename, false)
    }

    fn insert(&self, account: SafeAccount) -> Result<SafeAccount, Error> {
        let filename = account_filename(&account);
        self.insert_with_filename(account, filename, true)
    }

    fn remove(&self, account: &SafeAccount) -> Result<(), Error> {
        // enumerate all entries in keystore
        // and find entry with given address
        let to_remove = self
            .files_content()?
            .into_iter()
            .find(|&(_, ref acc)| acc.id == account.id && acc.address == account.address);

        // remove it
        match to_remove {
            None => Err(Error::InvalidAccount),
            Some((path, _)) => fs::remove_file(path).map_err(From::from),
        }
    }

    fn path(&self) -> Option<&PathBuf> {
        Some(&self.path)
    }

    fn unique_repr(&self) -> Result<u64, Error> {
        self.last_modification_date()
    }
}

impl KeyFileManager for DiskKeyFileManager {
    fn read<T>(&self, filename: Option<String>, reader: T) -> Result<SafeAccount, Error>
    where
        T: io::Read, {
        let key_file = json::KeyFile::load(reader).map_err(|e| Error::Custom(format!("{:?}", e)))?;
        Ok(SafeAccount::from_file(key_file, filename))
    }

    fn write<T>(&self, account: SafeAccount, writer: &mut T) -> Result<(), Error>
    where
        T: io::Write, {
        let key_file: json::KeyFile = account.into();
        key_file.write(writer).map_err(|e| Error::Custom(format!("{:?}", e)))
    }
}

fn account_filename(account: &SafeAccount) -> String {
    // build file path
    account.filename.clone().unwrap_or_else(|| {
        let timestamp = time::strftime("%Y-%m-%dT%H-%M-%S", &time::now_utc()).expect("Time-format string is valid.");
        format!("UTC--{}Z--{}", timestamp, Uuid::from(account.id))
    })
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::{env, fs};

    use ckey::{Generator, Random};

    use self::tempdir::TempDir;
    use super::*;

    #[test]
    fn create_new_account() {
        // given
        let mut dir = env::temp_dir();
        dir.push("keystore_should_create_new_account");
        let _ = fs::remove_dir_all(&dir);
        let keypair = Random.generate().unwrap();
        let password = &"hello world".into();
        let directory = RootDiskDirectory::create(dir.clone()).unwrap();

        // when
        let account = SafeAccount::create(&keypair, [0u8; 16], password, 1024, "Test".to_string(), "{}".to_string());
        let res = directory.insert(account.unwrap());

        // then
        assert!(res.is_ok(), "Should save account succesfuly.");
        assert!(res.unwrap().filename.is_some(), "Filename has been assigned.");

        // cleanup
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn handle_duplicate_filenames() {
        // given
        let mut dir = env::temp_dir();
        dir.push("keystore_should_handle_duplicate_filenames");
        let _ = fs::remove_dir_all(&dir);
        let keypair = Random.generate().unwrap();
        let password = &"hello world".into();
        let directory = RootDiskDirectory::create(dir.clone()).unwrap();

        // when
        let account =
            SafeAccount::create(&keypair, [0u8; 16], password, 1024, "Test".to_string(), "{}".to_string()).unwrap();
        let filename = "test".to_string();
        let dedup = true;

        directory.insert_with_filename(account.clone(), "foo".to_string(), dedup).unwrap();
        let file1 = directory.insert_with_filename(account.clone(), filename.clone(), dedup).unwrap().filename.unwrap();
        let file2 = directory.insert_with_filename(account.clone(), filename.clone(), dedup).unwrap().filename.unwrap();
        let file3 = directory.insert_with_filename(account.clone(), filename.clone(), dedup).unwrap().filename.unwrap();

        // then
        // the first file should have the original names
        assert_eq!(file1, filename);

        // the following duplicate files should have a suffix appended
        assert!(file2 != file3);
        assert_eq!(file2.len(), filename.len() + 5);
        assert_eq!(file3.len(), filename.len() + 5);

        // cleanup
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hash_of_files() {
        let temp_path = TempDir::new("").unwrap();
        let directory = RootDiskDirectory::create(&temp_path).unwrap();

        let hash = directory.files_hash().expect("Files hash should be calculated ok");
        assert_eq!(hash, 15130871412783076140);

        let keypair = Random.generate().unwrap();
        let password = &"test pass".into();
        let account = SafeAccount::create(&keypair, [0u8; 16], password, 1024, "Test".to_string(), "{}".to_string());
        directory.insert(account.unwrap()).expect("Account should be inserted ok");

        let new_hash = directory.files_hash().expect("New files hash should be calculated ok");

        assert!(new_hash != hash, "hash of the file list should change once directory content changed");
    }
}
