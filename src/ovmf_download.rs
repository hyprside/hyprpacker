// https://archlinux.org/packages/extra/any/edk2-ovmf/download/
// -> /usr/share/edk2/x64/OVMF_CODE.4m.fd
// -> /usr/share/edk2/x64/OVMF_VARS.4m.fd
use colored::*;
use std::path::PathBuf;

const OVMF_DOWNLOAD_URL: &str = "https://archlinux.org/packages/extra/any/edk2-ovmf/download/";
const OVMF_TARBALL_HASH: &str = "1D7FA267BF90BE35D5A792B14769226E5D371AADA87619B4F4DBDB621A552F3E";
const OVMF_TARBALL_PATH: &str = "build/ovmf/edk2-ovmf.tar.zst";
const OVMF_UNPACKED_DIR: &str = "build/ovmf/unpacked/";
const OVMF_CODE_FILE_HASH: &str =
	"92972B8AE68E808E33DD2E06C09CFD0766D654450C64C8979260B6C90FEE2991";
const OVMF_VARS_FILE_HASH: &str =
	"5D2AC383371B408398ACCEE7EC27C8C09EA5B74A0DE0CEEA6513388B15BE5D1E";

#[derive(Debug, thiserror::Error)]
pub enum OvfmDownloadError {
	#[error("an io error ocurred: {0}")]
	IOError(#[from] std::io::Error),
	#[error("failed to download: {0}")]
	DownloadError(#[from] ureq::Error),
	#[error("hash mismatch: expected {expected}, got {actual}")]
	HashMismatch { expected: String, actual: String },
}

/// Print a pretty result for the OVMF download operation.
/// This is intentionally separate from download_ovmf() — the
/// download function will print progress only; the caller can choose
/// whether and when to call this to print the final result.
pub fn print_ovmf_download_result(res: &Result<(PathBuf, PathBuf), OvfmDownloadError>) {
	match res {
		Ok((code, vars)) => {
			println!(
				"{} {} {}",
				"󰇚".green().bold(),
				"OVMF".cyan().bold(),
				format!("fetched to {} and {}", code.display(), vars.display()).green()
			);
		}
		Err(OvfmDownloadError::HashMismatch { expected, actual }) => {
			eprintln!(
    "{}:\n\n      {}: {}\n      {}:   {}\n\n      {}",
    "     Hash mismatch".red().bold(),
    "Expected".white(),
    expected.as_str().blue(),
    "Actual".white(),
    actual.as_str().white(),
    "(The file on the remote server may be corrupted, tampered with, or the URL may be incorrect.)".red()
   );
		}
		Err(e) => {
			eprintln!(
				"{} {}: {}",
				"    ".red().bold(),
				"Error fetching OVMF".red().bold(),
				format!("{}", e).red()
			);
		}
	}
}

/// Download edk2-ovmf (Arch package) and extract the BIOS files:
///  - usr/share/edk2/x64/OVMF_CODE.4m.fd
///  - usr/share/edk2/x64/OVMF_VARS.4m.fd
///
/// Both files are hash-checked. Returns (code_path, vars_path).
pub fn download_ovmf() -> Result<(PathBuf, PathBuf), OvfmDownloadError> {
	let tarball_path = PathBuf::from(OVMF_TARBALL_PATH);
	let unpack_dir = PathBuf::from(OVMF_UNPACKED_DIR);
	std::fs::create_dir_all(OVMF_UNPACKED_DIR)?;
	let code_rel = std::path::Path::new("usr")
		.join("share")
		.join("edk2")
		.join("x64")
		.join("OVMF_CODE.4m.fd");
	let vars_rel = std::path::Path::new("usr")
		.join("share")
		.join("edk2")
		.join("x64")
		.join("OVMF_VARS.4m.fd");
	let code_path = unpack_dir.join(&code_rel);
	let vars_path = unpack_dir.join(&vars_rel);

	// Start progress output
	println!(
		"{} {} {}",
		"󰇚".green().bold(),
		"Fetching OVMF (edk2-ovmf)".green().bold(),
		"...".green().bold()
	);

	// If we've already unpacked and hashes match, return early.
	if code_path.exists()
		&& crate::hash::hash_file(&code_path)?.as_str() == OVMF_CODE_FILE_HASH
		&& vars_path.exists()
		&& crate::hash::hash_file(&vars_path)?.as_str() == OVMF_VARS_FILE_HASH
	{
		println!(
			"    {} {} {}",
			"󰇚".green().bold(),
			"Using cached OVMF at".green(),
			format!("{} and {}", code_path.display(), vars_path.display()).cyan()
		);
		return Ok((code_path, vars_path));
	}

	// Ensure unpack dir is clean for a fresh attempt.
	if std::path::Path::new(OVMF_UNPACKED_DIR).exists() {
		println!(
			"    {} {}",
			"󰇚".green().bold(),
			"Cleaning previous unpacked directory".green()
		);
		std::fs::remove_dir_all(OVMF_UNPACKED_DIR)?;
	}

	// Check whether we need to download the tarball.
	let mut need_download = true;
	if tarball_path.exists() {
		println!(
			"    {} {}",
			"󰇚".green().bold(),
			"Found existing tarball, verifying hash...".green()
		);
		let hash = crate::hash::hash_file(tarball_path.clone())?;
		let actual = hash.to_string();
		if actual == OVMF_TARBALL_HASH {
			need_download = false;
			println!(
				"    {} {}",
				"󰇚".green().bold(),
				"Tarball hash matches; using cached tarball".green()
			);
		} else {
			// Remove corrupt/mismatched tarball so we re-download.
			println!(
				"    {} {}",
				"󰇚".yellow().bold(),
				"Tarball hash mismatch; removing and re-downloading".yellow()
			);
			std::fs::remove_file(&tarball_path)?;
		}
	}

	if need_download {
		// Ensure parent directory exists.
		if let Some(parent) = tarball_path.parent() {
			std::fs::create_dir_all(parent)?;
		}

		// Download with ureq.
		println!(
			"    {} {}",
			"󰇚".green().bold(),
			"Downloading OVMF package...".green()
		);
		let resp = ureq::get(OVMF_DOWNLOAD_URL).call()?;
		let mut reader = resp.into_body().into_reader();

		let mut out = std::fs::File::create(&tarball_path)?;
		std::io::copy(&mut reader, &mut out)?;
		println!(
			"    {} {} {}",
			"󰇚".green().bold(),
			"Downloaded package to".green(),
			format!("{}", tarball_path.display()).cyan()
		);
	}

	// Verify downloaded tarball hash.
	println!(
		"    {} {}",
		"󰇚".green().bold(),
		"Verifying downloaded package hash...".green()
	);
	let hash = crate::hash::hash_file(tarball_path.clone())?;
	let actual = hash.to_string();
	if actual != OVMF_TARBALL_HASH {
		return Err(OvfmDownloadError::HashMismatch {
			expected: OVMF_TARBALL_HASH.to_string(),
			actual,
		});
	}

	// Unpack the tarball if the unpacked files don't already exist.
	if !code_path.exists() || !vars_path.exists() {
		println!(
			"    {} {}",
			"󰇚".green().bold(),
			"Unpacking OVMF package...".green()
		);
		std::fs::create_dir_all(&unpack_dir)?;
		let tar_f = std::fs::File::open(&tarball_path)?;
		let dec = zstd::stream::read::Decoder::new(tar_f)?;
		let mut archive = tar::Archive::new(dec);
		archive.unpack(&unpack_dir)?;
		println!(
			"    {} {}",
			"󰇚".green().bold(),
			"Unpacked OVMF package".green()
		);
	}

	// Verify extracted files' hashes.
	if !code_path.exists() {
		return Err(OvfmDownloadError::IOError(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"OVMF_CODE file not found after unpacking",
		)));
	}
	if !vars_path.exists() {
		return Err(OvfmDownloadError::IOError(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"OVMF_VARS file not found after unpacking",
		)));
	}

	let code_hash = crate::hash::hash_file(code_path.clone())?;
	let vars_hash = crate::hash::hash_file(vars_path.clone())?;
	let code_actual = code_hash.to_string();
	let vars_actual = vars_hash.to_string();

	if code_actual != OVMF_CODE_FILE_HASH {
		return Err(OvfmDownloadError::HashMismatch {
			expected: OVMF_CODE_FILE_HASH.to_string(),
			actual: code_actual,
		});
	}
	if vars_actual != OVMF_VARS_FILE_HASH {
		return Err(OvfmDownloadError::HashMismatch {
			expected: OVMF_VARS_FILE_HASH.to_string(),
			actual: vars_actual,
		});
	}

	Ok((code_path, vars_path))
}
