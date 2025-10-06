use std::{
	hash::{DefaultHasher, Hash, Hasher},
	path::{Path, PathBuf},
	process::Command,
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use colored::Colorize;
use thiserror::Error;

fn has_file_newer_than(dir: &Path, timestamp: SystemTime) -> std::io::Result<bool> {
	if !dir.exists() {
		return Ok(false);
	}

	for entry in std::fs::read_dir(dir)? {
		let entry = entry?;
		let path = entry.path();
		let metadata = entry.metadata()?;

		if metadata.is_dir() {
			if has_file_newer_than(&path, timestamp)? {
				return Ok(true);
			}
		} else if let Ok(modified) = metadata.modified() {
			if modified > timestamp {
				return Ok(true);
			}
		}
	}

	Ok(false)
}
use crate::{
	manifest::{DockerSettings, InvalidSourceError, Manifest, Package, Source},
	prefix_commands,
};
pub struct BuildResult {
	total_packages: usize,
	built_packages: usize,
	errors: usize,
}

impl BuildResult {
	pub fn print(&self) {
		if self.errors == self.total_packages && self.total_packages > 0 {
			println!(
				"{}: {}{} package{} failed to build",
				"ERROR".red().bold(),
				if self.errors != 1 { "All of the " } else { "" },
				self.errors,
				if self.errors != 1 { "s" } else { "" }
			);
		} else if self.errors > 0 {
			eprintln!(
				"{}: {} of the {} package{} failed to build",
				"ERROR".red().bold(),
				self.errors.to_string().blue(),
				self.total_packages.to_string().blue(),
				if self.total_packages != 1 { "s" } else { "" }
			);
		} else if self.built_packages == 0 && self.errors == 0 {
			println!(
				"{}",
				"󱌢 No packages to build: already up-to-date"
					.green()
					.bold()
					.dimmed()
			);
		} else {
			println!(
				"{} {} {}{}{}",
				"󱌢 All".green(),
				self.built_packages.to_string().cyan(),
				if self.built_packages != 1 {
					"packages"
				} else {
					"package"
				}
				.green(),
				" were built successfully!".green(),
				if self.built_packages < self.total_packages {
					" (incremental build)"
				} else {
					""
				}
				.dimmed()
			);
		}
	}
	pub fn exit_if_failure(&self) {
		if self.errors > 0 {
			std::process::exit(1);
		}
	}
}

pub fn build(manifest: Manifest) -> BuildResult {
	println!();
	let packages = manifest
		.packages
		.iter()
		.filter(|p| p.needs_rebuild())
		.cloned()
		.collect::<Vec<Package>>();

	if packages.is_empty() {
		return BuildResult {
			total_packages: manifest.packages.len(),
			built_packages: 0,
			errors: 0,
		};
	}
	println!(
		"{} {} {}",
		"󱌢  Compiling".green().bold(),
		packages.len().to_string().cyan(),
		"packages...".green().bold()
	);

	let mut built_packages = 0;
	let mut errors = 0;
	for pkg in packages {
		println!(
			"    {} {} {}",
			"󱌢  Compiling".green().bold(),
			pkg.name,
			pkg.version.dimmed()
		);
		match pkg.build() {
			Ok(()) => built_packages += 1,
			Err(error) => {
				errors += 1;
				println!(
					"\n    {} {}: {}\n",
					"  Error building package".red().bold(),
					pkg.name.cyan().bold().italic(),
					error.to_string().dimmed()
				);
			}
		}
	}
	BuildResult {
		total_packages: manifest.packages.len(),
		built_packages: built_packages,
		errors,
	}
}

#[derive(Debug, Error)]
pub enum BuildError {
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("process exited with non-zero code: {0}")]
	Non0ExitCode(i32),
	#[error("invalid source: {0}")]
	InvalidSource(#[from] InvalidSourceError),
	#[error("failed to unpack binary: {0}")]
	UnpackBinaryError(std::io::Error),
	#[error("failed to build docker image: {0}")]
	DockerError(#[from] BuildDockerImageError),
	#[error("no package found in out directory")]
	NoPackageFound,
}
#[derive(Debug, Error)]
pub enum BuildDockerImageError {
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("process exited with non-zero code: {0}")]
	Non0ExitCode(i32),
	#[error("invalid dockerfile path")]
	InvalidDockerfilePath(PathBuf),
}
impl Package {
	pub fn get_out_dir(&self) -> PathBuf {
		// calculate hash of self using Hash trait
		let mut hasher = DefaultHasher::new();
		self.source.hash(&mut hasher);
		self.docker.hash(&mut hasher);
		let hash = hasher.finish();
		[
			"build",
			"out",
			format!("{}-{}-{}", self.name, self.version, hash).as_str(),
		]
		.iter()
		.collect()
	}
	pub fn create_out_dir(&self) -> Result<PathBuf, std::io::Error> {
		let build_dir = self.get_out_dir();
		std::fs::create_dir_all(&build_dir)?;
		Ok(build_dir)
	}
	pub fn get_out_unpacked_dir(&self) -> PathBuf {
		let mut build_dir = self.get_out_dir();
		build_dir.push("unpacked");
		build_dir
	}
	pub fn create_out_unpacked_dir(&self) -> Result<PathBuf, std::io::Error> {
		let build_dir = self.get_out_unpacked_dir();
		std::fs::create_dir_all(&build_dir)?;
		Ok(build_dir)
	}
	pub fn build(&self) -> Result<(), BuildError> {
		let build_dir = self.create_out_dir()?;
		let unpacked_dir = self.create_out_unpacked_dir()?;
		match &self.source {
			Source::Binary { .. } => {
				let archlinux_pkg_path = self.source_tarball_path()?;
				// extract arch linux .pkg.tar.zst into the build_dir (streaming)
				let zstd = zstd::Decoder::new(std::fs::File::open(&archlinux_pkg_path)?)
					.map_err(BuildError::UnpackBinaryError)?;
				let mut tar = tar::Archive::new(zstd);

				println!(
					"    {} {}",
					"  Unpacking".green().bold(),
					archlinux_pkg_path
						.file_name()
						.unwrap()
						.display()
						.to_string()
						.italic()
				);
				tar
					.unpack(unpacked_dir)
					.map_err(BuildError::UnpackBinaryError)?;

				println!(
					"  {}  {} {}",
					" ".green().bold(),
					archlinux_pkg_path
						.file_name()
						.unwrap()
						.display()
						.to_string()
						.italic(),
					"unpacked successfully".green().bold()
				);
			}
			Source::PkgBuildGit {
				pick_packages_from_group,
				..
			}
			| Source::PkgBuildLocal {
				pick_packages_from_group,
				..
			} => {
				let docker_image_name = self.build_docker_image_if_needed()?;
				let pkg_build_root = if let Source::PkgBuildLocal { path, .. } = &self.source {
					path.clone()
				} else {
					self.get_package_prepared_dir()
				};
				let mut command = Command::new("docker");
				let script = r#"
pacman -Sy --needed --noconfirm sudo # Install sudo
useradd builduser -m # Create the builduser
passwd -d builduser # Delete the buildusers password
printf 'builduser ALL=(ALL) ALL\nDefaults    env_keep += "PKGDEST"\nDefaults    env_keep += "BUILDDIR"\n' | tee -a /etc/sudoers # Allow the builduser passwordless sudo
cd /src
rm -rf /out/makepkg/pkg
rm -rf /out/makepkg/*.pkg.tar.zst
rm -rf /out/*.pkg.tar.zst
mkdir /out/makepkg -p
chown builduser:builduser /out/ -R
sudo -u builduser bash -c 'makepkg --noconfirm --noprogressbar -s -C -f'
"#;
				command
					.arg("run")
					.arg("--rm")
					.arg("-v")
					.arg(format!("{}:/src", pkg_build_root.canonicalize()?.display()))
					.arg("-v")
					.arg(format!("{}:/out", build_dir.canonicalize()?.display()))
					.arg("-e")
					.arg("PKGDEST=/out")
					.arg("-e")
					.arg("BUILDDIR=/out/makepkg")
					.arg(docker_image_name)
					.arg("bash")
					.arg("-c")
					.arg(script);
				let exit_status = prefix_commands::run_command_with_tag(
					command,
					format!(
						"{}{}{}{}{}",
						"[".dimmed(),
						self.name.bold(),
						"@".dimmed(),
						self.version.dimmed(),
						" | makepkg] ".dimmed()
					),
				)
				.map_err(BuildError::Io)?;
				if !exit_status.success() {
					return Err(BuildError::Non0ExitCode(exit_status.code().unwrap_or(-1)));
				}
				let files_listing = std::fs::read_dir(&build_dir)?
					.filter_map(|entry| entry.ok())
					.filter(|entry| {
						entry
							.file_name()
							.to_string_lossy()
							.ends_with(".pkg.tar.zst")
					});

				let files_to_unpack: Vec<_> = match &pick_packages_from_group {
					Some(pkg_names) => files_listing
						.filter(|entry| {
							let file_name = entry.file_name();
							let file_name = file_name.to_string_lossy();
							pkg_names.iter().any(|pkg| {
								let pattern_prefix = format!("{}-{}-", pkg, self.version);
								file_name.starts_with(&pattern_prefix)
							})
						})
						.collect(),
					None => files_listing
						.filter(|entry| {
							let file_name = entry.file_name();
							let file_name = file_name.to_string_lossy();
							let pattern_prefix = format!("{}-debug-{}-", self.name, self.version);
							!file_name.starts_with(&pattern_prefix)
						})
						.collect(),
				};

				if files_to_unpack.is_empty() {
					return Err(BuildError::NoPackageFound);
				}

				for pkg_path in files_to_unpack {
					let path = pkg_path.path();
					println!(
						"    {} {}",
						"  Unpacking".yellow().bold(),
						path.file_name().unwrap().display().to_string().italic()
					);

					let file = std::fs::File::open(&path).map_err(BuildError::UnpackBinaryError)?;
					let zstd = zstd::Decoder::new(file).map_err(BuildError::UnpackBinaryError)?;
					let mut tar = tar::Archive::new(zstd);
					tar
						.unpack(&unpacked_dir)
						.map_err(BuildError::UnpackBinaryError)?;

					println!(
						"  {}  {} {}",
						" ".green().bold(),
						path.file_name().unwrap().display().to_string().italic(),
						"unpacked successfully".green().bold()
					);
				}

				// save the current time in a "last_successful_build_time" file
				std::fs::write(
					build_dir.join("last_successful_build_time"),
					std::time::SystemTime::now()
						.duration_since(std::time::UNIX_EPOCH)
						.unwrap()
						.as_millis()
						.to_string(),
				)?;
			}
		}
		Ok(())
	}

	pub fn needs_rebuild(&self) -> bool {
		let build_dir = self.get_out_dir();
		let last_successful_build_time_path = build_dir.join("last_successful_build_time");

		if !last_successful_build_time_path.exists() {
			return true;
		}

		let Some(last_successful_build_time) =
			std::fs::read_to_string(&last_successful_build_time_path)
				.ok()
				.and_then(|s| s.parse::<u128>().ok())
		else {
			return true;
		};

		let timestamp =
			UNIX_EPOCH + std::time::Duration::from_millis(last_successful_build_time as u64);

		let source_path = match &self.source {
			Source::PkgBuildLocal { path, .. } => path.clone(),
			Source::PkgBuildGit { .. } | Source::Binary { .. } => self
				.source_tarball_path()
				.ok()
				.unwrap_or_else(|| self.get_package_prepared_dir()),
		};

		let needs_rebuild = has_file_newer_than(&source_path, timestamp).unwrap_or(true);

		needs_rebuild
	}
	pub fn get_docker_image_name(&self) -> Result<String, BuildDockerImageError> {
		Ok(match &self.docker {
			DockerSettings::DockerfilePath {
				path: dockerfile_path,
			} => {
				let mut hasher = DefaultHasher::new();
				dockerfile_path.canonicalize()?.hash(&mut hasher);
				let hash = hasher.finish();
				format!("hyprpacker-{}", hash)
			}
			DockerSettings::ImageName { name } => name.clone(),
		})
	}
	pub fn build_docker_image_if_needed(&self) -> Result<String, BuildDockerImageError> {
		match &self.docker {
			DockerSettings::DockerfilePath {
				path: dockerfile_path,
			} => {
				let docker_image_name = self.get_docker_image_name()?;
				let dockerfile_folder = dockerfile_path
					.parent()
					.ok_or_else(|| BuildDockerImageError::InvalidDockerfilePath(dockerfile_path.clone()))?;
				let mut command = Command::new("docker");
				command.args([
					"build",
					"-t",
					&docker_image_name,
					"-f",
					dockerfile_path
						.to_str()
						.ok_or_else(|| BuildDockerImageError::InvalidDockerfilePath(dockerfile_path.clone()))?,
					dockerfile_folder
						.to_str()
						.ok_or_else(|| BuildDockerImageError::InvalidDockerfilePath(dockerfile_path.clone()))?,
				]);
				let output = prefix_commands::run_command_with_tag(
					command,
					format!(
						"{}{}{}{}{}",
						"[".dimmed(),
						self.name.bold(),
						"@".dimmed(),
						self.version.dimmed(),
						" | Dockerfile] ".dimmed()
					),
				)
				.map_err(BuildDockerImageError::Io)?;
				if output.success() {
					Ok(docker_image_name)
				} else {
					Err(BuildDockerImageError::Non0ExitCode(
						output.code().unwrap_or(-1),
					))
				}
			}
			DockerSettings::ImageName { name } => Ok(name.clone()),
		}
	}
}
