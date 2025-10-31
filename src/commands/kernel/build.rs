use std::{
	fs::{self, File},
	io::{self, Write},
	path::{Path, PathBuf},
	process::{Command, Stdio},
};

use colored::Colorize;
use thiserror::Error;

use crate::{
	manifest::{KernelOptionValue, Manifest},
	prefix_commands,
};

const KERNEL_IMAGE_NAME: &str = "hyprpacker-kernel-builder:latest";
const KERNEL_DOCKERFILE_CONTENT: &str = include_str!("../../../docker/kernel.Dockerfile");
const BUILD_SCRIPT: &str = r##"set -euo pipefail

DOWNLOADS="/kernel/downloads"
SRC="/kernel/src"
OUT="/kernel/out"
CONFIG="/kernel/config/options.config"

TARBALL="$(find "${DOWNLOADS}" -maxdepth 1 -type f | head -n1)"

if [[ -z "${TARBALL}" ]]; then
  echo "kernel tarball not found in ${DOWNLOADS}" >&2
  exit 1
fi

rm -rf "${SRC}/*"
tar -xf "${TARBALL}" -C "${SRC}"

rm -rf "${OUT}/*"
mkdir -p "${OUT}/modules"

pushd "${SRC}" >/dev/null

make x86_64_defconfig

if [[ -f "${CONFIG}" ]]; then
  while IFS='=' read -r key value; do
    [[ -z "${key}" ]] && continue
    [[ "${key}" =~ ^# ]] && continue
    key="${key//[[:space:]]/}"
    key=${key#CONFIG_}
    key=${key#config_}
    key=${key#Config_}
    key="${key//-/_}"
    key="${key//./_}"
    value="${value//[[:space:]]/}"
    symbol="CONFIG_${key^^}"

  # Determine replacement form
  if [[ "${value}" == "y" ]]; then
    replacement="${symbol}=y"
  elif [[ "${value}" == "n" ]]; then
    replacement="# ${symbol} is not set"
  elif [[ "${value}" =~ ^[0-9]+$ ]]; then
    # numeric value
    replacement="${symbol}=${value}"
  else
    # treat as string: if already quoted, keep; otherwise quote and escape internal quotes
    if [[ "${value}" =~ ^\".*\"$ || "${value}" =~ ^\'.*\'$ ]]; then
      strval="${value}"
    else
      esc=$(printf '%s' "${value}" | sed 's/"/\\"/g')
      strval="\"${esc}\""
    fi
    replacement="${symbol}=${strval}"
  fi

  # Escape replacement for sed (escape &, | and /)
  escaped_replacement=$(printf '%s' "${replacement}" | sed -e 's/[&/|]/\\&/g')

  # Try to replace existing setting (either CONFIG=... or commented "is not set")
  sed -i -e "s|^${symbol}=.*|${escaped_replacement}|" -e "s|^# ${symbol} is not set|${escaped_replacement}|" "${KCONFIG_FILE}" || true

  # If nothing matched above, append the setting
  if ! grep -q -E "^(# ${symbol} is not set|${symbol}=)" "${KCONFIG_FILE}" 2>/dev/null; then
    echo "${replacement}" >> "${KCONFIG_FILE}"
  fi
  done < "${CONFIG}"
fi

make olddefconfig
make -j"$(nproc)"
make modules_install INSTALL_MOD_PATH="${OUT}/modules"

if [[ -f arch/x86/boot/bzImage ]]; then
  cp arch/x86/boot/bzImage "${OUT}/kernel"
else
  echo "Kernel not found after build" >&2
  exit 1
fi

popd >/dev/null
"##;

#[derive(Debug, Error)]
pub enum KernelBuildError {
	#[error("io error: {0}")]
	Io(#[from] io::Error),
	#[error("failed to download kernel sources: {0}")]
	Download(#[from] ureq::Error),
	#[error("docker build failed with status code {0:?}")]
	DockerBuildFailed(Option<i32>),
	#[error("docker run failed with status code {0:?}")]
	DockerRunFailed(Option<i32>),
	#[error("kernel artifact not produced at {0}")]
	MissingArtifact(PathBuf),
}

pub struct KernelBuildResult {
	pub artifact_path: PathBuf,
}

impl KernelBuildResult {
	pub fn print(&self) {
		let artifact = self.artifact_path.display().to_string().green().bold();
		println!(
			"{} {}",
			"✔🐧 Kernel image available at".green().bold(),
			artifact
		);
	}
}

pub fn build(manifest: &Manifest) -> Result<KernelBuildResult, KernelBuildError> {
	let kernel = &manifest.kernel;

	let kernel_root = PathBuf::from("build/kernel");
	let downloads_dir = kernel_root.join("downloads");
	let src_dir = kernel_root.join("src");
	let out_dir = kernel_root.join("out");
	let config_dir = kernel_root.join("config");

	fs::create_dir_all(&downloads_dir)?;
	fs::create_dir_all(&src_dir)?;
	fs::create_dir_all(&out_dir)?;
	fs::create_dir_all(&config_dir)?;

	let tarball_name = extract_filename(&kernel.url).unwrap_or_else(|| "kernel.tar".to_string());
	let tarball_path = downloads_dir.join(&tarball_name);

	let needs_download =
		!tarball_path.exists() || tarball_path.metadata().map(|m| m.len()).unwrap_or(0) == 0;
	if needs_download {
		println!(
			"{} {}",
			"󰇚 Downloading kernel sources".green().bold(),
			kernel.url.cyan()
		);
		let response = ureq::get(&kernel.url)
			.call()
			.map_err(KernelBuildError::Download)?;
		let mut reader = response.into_body().into_reader();
		let mut file = File::create(&tarball_path)?;
		std::io::copy(&mut reader, &mut file)?;
	} else {
		println!(
			"{} {}",
			"󰇚 Reusing cached kernel sources".green().bold(),
			tarball_path.display()
		);
	}

	let options_path = config_dir.join("options.config");
	write_options_file(&options_path, &kernel.options)?;

	let dockerfile_path = kernel_root.join("kernel.Dockerfile");
	fs::write(&dockerfile_path, KERNEL_DOCKERFILE_CONTENT)?;

	ensure_kernel_builder_image(&dockerfile_path)?;

	let downloads_dir = canonicalize(&downloads_dir)?;
	let src_dir = canonicalize(&src_dir)?;
	let out_dir = canonicalize(&out_dir)?;
	let config_dir = canonicalize(&config_dir)?;

	println!("{}", "🐧 Building kernel inside container".blue().bold());
	let mut command = Command::new("docker");
	command
		.arg("run")
		.arg("--rm")
		.arg("-v")
		.arg(format!("{}:/kernel/downloads:ro", downloads_dir.display()))
		.arg("-v")
		.arg(format!("{}:/kernel/src", src_dir.display()))
		.arg("-v")
		.arg(format!("{}:/kernel/out", out_dir.display()))
		.arg("-v")
		.arg(format!("{}:/kernel/config:ro", config_dir.display()))
		.arg(KERNEL_IMAGE_NAME)
		.arg("bash")
		.arg("-c")
		.arg(BUILD_SCRIPT);

	let status = prefix_commands::run_command_with_tag(
		command,
		"       [ 🐧 kernel-build ] ".blue().to_string(),
	)?;
	if !status.success() {
		return Err(KernelBuildError::DockerRunFailed(status.code()));
	}

	let artifact_path = locate_artifact(&out_dir)?;
	Ok(KernelBuildResult { artifact_path })
}

fn write_options_file(
	path: &Path,
	options: &crate::manifest::KernelOptions,
) -> Result<(), io::Error> {
	let mut file = File::create(path)?;
	for (key, value) in options {
		let normalized_key = normalize_option_key(key);
		let val = match value {
			KernelOptionValue::Number(n) => n.to_string(),
			KernelOptionValue::String(s) => match s.as_str() {
				"m" | "y" | "n" => s.clone(),
				s => format!("\"{s}\""),
			},
		};
		writeln!(file, "{normalized_key}={val}")?;
	}
	Ok(())
}

fn normalize_option_key(raw: &str) -> String {
	let trimmed = raw.trim().trim_start_matches("CONFIG_");
	trimmed
		.trim_start_matches("config_")
		.trim_start_matches("Config_")
		.replace('-', "_")
		.replace('.', "_")
		.to_uppercase()
}

fn extract_filename(url: &str) -> Option<String> {
	let without_query = url.split('?').next().unwrap_or(url);
	without_query
		.trim_end_matches('/')
		.rsplit('/')
		.next()
		.filter(|s| !s.is_empty())
		.map(|s| s.to_string())
}

fn ensure_kernel_builder_image(dockerfile_path: &Path) -> Result<(), KernelBuildError> {
	let inspect_status = Command::new("docker")
		.args(["image", "inspect", KERNEL_IMAGE_NAME])
		.stdout(Stdio::null())
		.status()?;
	if inspect_status.success() {
		return Ok(());
	}

	let dockerfile_path = dockerfile_path.canonicalize()?;
	let build_context = dockerfile_path
		.parent()
		.map(Path::to_path_buf)
		.unwrap_or_else(|| PathBuf::from("."));

	println!("{}", "󱌢 Building kernel builder Docker image".blue().bold());
	let mut command = Command::new("docker");
	command
		.arg("build")
		.arg("-f")
		.arg(&dockerfile_path)
		.arg("-t")
		.arg(KERNEL_IMAGE_NAME)
		.arg(&build_context);
	let status = prefix_commands::run_command_with_tag(
		command,
		"       [ 🐧 kernel-image ] ".blue().to_string(),
	)?;
	if !status.success() {
		return Err(KernelBuildError::DockerBuildFailed(status.code()));
	}
	Ok(())
}

fn canonicalize(path: &Path) -> Result<PathBuf, io::Error> {
	std::fs::canonicalize(path)
}

fn locate_artifact(out_dir: &Path) -> Result<PathBuf, KernelBuildError> {
	let kernel_path = out_dir.join("kernel");
	if kernel_path.exists() {
		return Ok(kernel_path);
	}
	Err(KernelBuildError::MissingArtifact(kernel_path))
}
