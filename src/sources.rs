use std::path::{Path, PathBuf};

use colored::Colorize;
use serde::Deserialize;

use crate::{
	hash::{Sha256Hash, hash_file},
	manifest::{InvalidSourceError, Package, Source, SourceFetchError},
};

#[derive(Debug, Deserialize, Clone)]
pub enum SourceType {
	Tarball { url: String, sha256: Sha256Hash },
	LocalFolder { path: PathBuf },
}
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
	std::fs::create_dir_all(&dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let ty = entry.file_type()?;
		if ty.is_dir() {
			copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
		} else {
			std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
		}
	}
	Ok(())
}

impl Package {
	pub const fn sources_path() -> &'static str {
		"build/sources"
	}

	pub fn create_sources_dir() -> std::io::Result<()> {
		std::fs::create_dir_all(Self::sources_path())
	}

	pub fn prepared_sources_dir() -> PathBuf {
		let mut path = PathBuf::from(Self::sources_path());
		path.push("prepared");
		path
	}

	pub fn create_prepared_sources_dir() -> std::io::Result<()> {
		std::fs::create_dir_all(Self::prepared_sources_dir())
	}

	pub fn source_type(&self) -> Result<SourceType, InvalidSourceError> {
		match self.source.clone() {
			Source::Binary { url, sha256 } => Ok(SourceType::Tarball { url, sha256 }),
			Source::PkgBuildLocal { path, .. } => Ok(SourceType::LocalFolder { path }),
			Source::PkgBuildGit {
				repo_url,
				rev,
				sha256,
				..
			} => {
				let tarball_url = if repo_url.contains("github.com") {
					// GitHub tarball URL format: https://github.com/{owner}/{repo}/archive/{rev}.tar.gz
					let repo = repo_url.trim_end_matches(".git");
					Some(format!("{repo}/archive/{rev}.tar.gz"))
				} else if repo_url.contains("gitlab.com") || repo_url.contains('/') {
					// GitLab tarball URL format: {repo_url}/-/archive/{rev}/{repo_name}-{rev}.tar.gz
					let repo = repo_url.trim_end_matches(".git");
					let repo_name = repo
						.split('/')
						.last()
						.ok_or(InvalidSourceError::InvalidGitSourceUrl)?;
					Some(format!("{repo}/-/archive/{rev}/{repo_name}-{rev}.tar.gz"))
				} else {
					None
				};
				match tarball_url {
					Some(url) => Ok(SourceType::Tarball { url, sha256 }),
					None => Err(InvalidSourceError::InvalidGitSourceUrl),
				}
			}
		}
	}

	pub fn source_tarball_path(&self) -> Result<PathBuf, InvalidSourceError> {
		let mut path = PathBuf::from(Self::sources_path());
		let t = self.source_type()?;
		match t {
			SourceType::Tarball { .. } => {
				use std::collections::hash_map::DefaultHasher;
				use std::hash::{Hash, Hasher};

				let mut hasher = DefaultHasher::new();
				self.source.hash(&mut hasher);
				let hash = hasher.finish();
				path.push(format!("{:x}.tar.gz", hash));
				Ok(path)
			}
			SourceType::LocalFolder { .. } => Err(InvalidSourceError::UnsupportedSourceType),
		}
	}

	pub fn assert_source_tarball_matches_hash(&self) -> Result<(), SourceFetchError> {
		let path = self.source_tarball_path()?;
		let t = self.source_type()?;
		match t {
			SourceType::Tarball { sha256, .. } => {
				let hash = hash_file(&path).map_err(SourceFetchError::Io)?;
				if hash != sha256 {
					return Err(SourceFetchError::HashMismatch {
						expected: sha256,
						actual: hash,
					});
				}
				Ok(())
			}
			SourceType::LocalFolder { .. } => Err(SourceFetchError::InvalidSource(
				InvalidSourceError::UnsupportedSourceType,
			)),
		}
	}

	pub fn get_package_prepared_dir(&self) -> PathBuf {
		let mut d = Self::prepared_sources_dir();
		d.push(format!("{}-{}", self.name, self.version));
		d
	}

	pub fn prepare_sources(&self) -> Result<PathBuf, SourceFetchError> {
		match &self.source {
			Source::PkgBuildGit { repo_url, rev, .. } => {
				// Ensure prepared dir exists
				Self::create_prepared_sources_dir()?;
				let tarball_path = self.source_tarball_path()?;
				let prepared_dir = self.get_package_prepared_dir();

				// If already unpacked, skip
				if prepared_dir.exists() {
					std::fs::remove_dir_all(&prepared_dir)?;
				}

				// Unpack tarball
				let tar_gz = std::fs::File::open(&tarball_path)?;
				let decompressor = flate2::read::GzDecoder::new(tar_gz);
				let mut archive = tar::Archive::new(decompressor);
				let repo_name = repo_url.split('/').last().unwrap().trim_end_matches(".git");
				let folder_name = format!("{repo_name}-{rev}");
				let tmp = std::env::temp_dir();
				std::fs::create_dir_all(&tmp)?;
				archive.unpack(&tmp)?;
				let extracted_dir = tmp.join(folder_name);
				std::fs::rename(&extracted_dir, &prepared_dir).or_else(|_| {
					copy_dir_all(&extracted_dir, &prepared_dir)?;
					std::fs::remove_dir_all(&extracted_dir)
				})?;
				Ok(prepared_dir)
			}
			Source::PkgBuildLocal { path, .. } => Ok(PathBuf::from(path)),
			Source::Binary { .. } => self
				.source_tarball_path()
				.map_err(SourceFetchError::InvalidSource),
		}
	}
	pub fn fetch_sources(&self) -> Result<(), SourceFetchError> {
		let t = self.source_type()?;
		match t {
			SourceType::Tarball { url, .. } => {
				let tarball_path = self.source_tarball_path()?;
				let needs_download = self.assert_source_tarball_matches_hash().is_err();
				if needs_download {
					eprintln!(
						"    {} {} {}",
						"󰇚 Fetching".green().bold(),
						self.name,
						self.version
					);
					let resp = ureq::get(&url)
						.call()
						.map_err(SourceFetchError::FetchError)?;
					let mut reader = resp.into_body().into_reader();
					let mut file = std::fs::File::create(&tarball_path).map_err(SourceFetchError::Io)?;
					std::io::copy(&mut reader, &mut file).map_err(SourceFetchError::Io)?;
				}
				self.assert_source_tarball_matches_hash()?;
				Ok(())
			}
			SourceType::LocalFolder { .. } => Ok(()),
		}
	}
}
