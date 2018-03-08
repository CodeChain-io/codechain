use std::fmt;
use secp256k1::Error as SecpError;
use bech32::Error as Bech32Error;

#[derive(Debug, PartialEq)]
pub enum Error {
	InvalidPublic,
	InvalidSecret,
	InvalidMessage,
	InvalidSignature,
	InvalidNetwork,
	InvalidChecksum,
	InvalidPrivate,
	InvalidAddress,
	FailedKeyGeneration,
	Bech32MissingSeparator,
	Bech32InvalidChecksum,
	Bech32InvalidLength,
	Bech32InvalidChar(u8),
	Bech32InvalidData(u8),
	Bech32MixedCase,
	Bech32UnknownHRP,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = match *self {
			Error::InvalidPublic => "Invalid Public",
			Error::InvalidSecret => "Invalid Secret",
			Error::InvalidMessage => "Invalid Message",
			Error::InvalidSignature => "Invalid Signature",
			Error::InvalidNetwork => "Invalid Network",
			Error::InvalidChecksum => "Invalid Checksum",
			Error::InvalidPrivate => "Invalid Private",
			Error::InvalidAddress => "Invalid Address",
			Error::FailedKeyGeneration => "Key generation failed",
			Error::Bech32MissingSeparator => "Missing human-readable separator",
			Error::Bech32InvalidChecksum => "Invalid checksum",
			Error::Bech32InvalidLength => "Invalid Length",
			Error::Bech32InvalidChar(_) => "Invalid character",
			Error::Bech32InvalidData(_) => "Invalid data point",
			Error::Bech32MixedCase => "Mixed-case strings not allowed",
			Error::Bech32UnknownHRP => "Unknown human-readable part",
		};

		msg.fmt(f)
	}
}

impl From<SecpError> for Error {
	fn from(e: SecpError) -> Self {
		match e {
			SecpError::InvalidPublicKey	=> Error::InvalidPublic,
			SecpError::InvalidSecretKey => Error::InvalidSecret,
			SecpError::InvalidMessage => Error::InvalidMessage,
			_ => Error::InvalidSignature,
		}
	}
}

impl From<Bech32Error> for Error {
	fn from(e: Bech32Error) -> Self {
		match e {
			Bech32Error::MissingSeparator => Error::Bech32MissingSeparator,
			Bech32Error::InvalidChecksum => Error::Bech32InvalidChecksum,
			Bech32Error::InvalidLength => Error::Bech32InvalidLength,
			Bech32Error::InvalidChar(ch) => Error::Bech32InvalidChar(ch),
			Bech32Error::InvalidData(data) => Error::Bech32InvalidData(data),
			Bech32Error::MixedCase => Error::Bech32MixedCase,
		}
	}
}
