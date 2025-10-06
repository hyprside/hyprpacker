use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::{fs, io};

pub fn hash_file(path: impl Into<PathBuf>) -> io::Result<Sha256Hash> {
	let path = path.into();

	let mut hasher = Sha256::new();
	let mut file = fs::File::open(path)?;

	io::copy(&mut file, &mut hasher)?;
	let hash_bytes = hasher.finalize();
	Ok(format!("{:X}", hash_bytes).into())
}
pub fn default_hash<T: From<String>>() -> T {
	"A".repeat(64).into()
}

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, Hash, Eq, Serialize)]
pub struct Sha256Hash(String);

impl PartialEq for Sha256Hash {
	fn eq(&self, other: &Self) -> bool {
		self.0.to_uppercase() == other.0.to_uppercase()
	}
}

impl Sha256Hash {
	pub fn as_str(&self) -> &str {
		&self.0
	}
	pub fn into_string(self) -> String {
		self.0
	}
	pub fn from_str(s: &str) -> Result<Self, String> {
		if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
			Ok(Sha256Hash(s.to_uppercase()))
		} else {
			Err(format!("Invalid SHA256 hash: {}", s))
		}
	}
}

impl<'de> Deserialize<'de> for Sha256Hash {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Sha256Hash::from_str(&s).map_err(serde::de::Error::custom)
	}
}

impl fmt::Display for Sha256Hash {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<String> for Sha256Hash {
	fn from(s: String) -> Self {
		Sha256Hash::from_str(&s).unwrap()
	}
}

impl From<Sha256Hash> for String {
	fn from(hash: Sha256Hash) -> Self {
		hash.0
	}
}
