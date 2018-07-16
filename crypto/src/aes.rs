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

use error::SymmError;
use primitives::{H128, H256};
use rcrypto::aes::KeySize::KeySize256;
use rcrypto::aes::{cbc_decryptor, cbc_encryptor};
use rcrypto::aessafe::AesSafe128Encryptor;
use rcrypto::blockmodes::{CtrMode, PkcsPadding};
use rcrypto::buffer::{BufferResult, ReadBuffer, RefReadBuffer, RefWriteBuffer, WriteBuffer};
pub use rcrypto::symmetriccipher::SymmetricCipherError;
use rcrypto::symmetriccipher::{Decryptor, Encryptor};

fn is_underflow(result: BufferResult) -> bool {
    match result {
        BufferResult::BufferUnderflow => true,
        BufferResult::BufferOverflow => false,
    }
}

// AES-256/CBC/Pkcs encryption.
pub fn encrypt(data: &[u8], key: &H256, iv: &H128) -> Result<Vec<u8>, SymmetricCipherError> {
    let mut encryptor = cbc_encryptor(KeySize256, key, iv, PkcsPadding);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = RefReadBuffer::new(data);
    let mut buffer = [0; 4096];
    let mut write_buffer = RefWriteBuffer::new(&mut buffer);


    let mut finish = false;
    while !finish {
        finish = is_underflow(encryptor.encrypt(&mut read_buffer, &mut write_buffer, true)?);
        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));
    }

    Ok(final_result)
}

// AES-256/CBC/Pkcs decryption.
pub fn decrypt(encrypted_data: &[u8], key: &H256, iv: &H128) -> Result<Vec<u8>, SymmetricCipherError> {
    let mut decryptor = cbc_decryptor(KeySize256, key, iv, PkcsPadding);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = RefReadBuffer::new(encrypted_data);
    let mut buffer = [0; 4096];
    let mut write_buffer = RefWriteBuffer::new(&mut buffer);

    let mut finish = false;
    while !finish {
        finish = is_underflow(decryptor.decrypt(&mut read_buffer, &mut write_buffer, true)?);
        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));
    }

    Ok(final_result)
}

/// Encrypt a message (CTR mode).
///
/// Key (`k`) length and initialisation vector (`iv`) length have to be 16 bytes each.
/// An error is returned if the input lengths are invalid.
pub fn encrypt_128_ctr(k: &[u8], iv: &[u8], plain: &[u8], dest: &mut [u8]) -> Result<(), SymmError> {
    let mut encryptor = CtrMode::new(AesSafe128Encryptor::new(k), iv.to_vec());
    encryptor.encrypt(&mut RefReadBuffer::new(plain), &mut RefWriteBuffer::new(dest), true)?;
    Ok(())
}

/// Decrypt a message (CTR mode).
///
/// Key (`k`) length and initialisation vector (`iv`) length have to be 16 bytes each.
/// An error is returned if the input lengths are invalid.
pub fn decrypt_128_ctr(k: &[u8], iv: &[u8], encrypted: &[u8], dest: &mut [u8]) -> Result<(), SymmError> {
    let mut encryptor = CtrMode::new(AesSafe128Encryptor::new(k), iv.to_vec());
    encryptor.decrypt(&mut RefReadBuffer::new(encrypted), &mut RefWriteBuffer::new(dest), true)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use primitives::{H128, H256};

    use self::rand::{OsRng, RngCore};
    use super::{decrypt, encrypt};

    #[test]
    fn test_aes256_with_random_key_and_iv() {
        let message = "0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
                       0123456789abcdefghijklmnopqrstubewxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

        let mut key = H256([0; 32]);
        let mut iv = H128([0; 16]);

        // In a real program, the key and iv may be determined
        // using some other mechanism. If a password is to be used
        // as a key, an algorithm like PBKDF2, Bcrypt, or Scrypt (all
        // supported by Rust-Crypto!) would be a good choice to derive
        // a password. For the purposes of this example, the key and
        // iv are just random values.
        let mut rng = OsRng::new().ok().unwrap();
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut iv);

        let encrypted_data = encrypt(message.as_bytes(), &key, &iv).ok().unwrap();
        let decrypted_data = decrypt(&encrypted_data[..], &key, &iv).ok().unwrap();

        assert_eq!(message.as_bytes(), &decrypted_data[..]);
    }

    #[test]
    fn test_short_input() {
        let input = vec![130, 39, 16];

        let mut key = H256([0; 32]);
        let mut iv = H128([0; 16]);

        let mut rng = OsRng::new().unwrap();
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut iv);

        let encrypted = encrypt(&input, &key, &iv).unwrap();
        let decrypted = decrypt(&encrypted, &key, &iv).unwrap();
        assert_eq!(input, decrypted);
    }
}
