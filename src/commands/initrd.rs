use std::{
	collections::HashMap,
	fs,
	io::{BufRead, BufReader},
	path::{Path, PathBuf},
	process::{Command, ExitStatus, Stdio},
	time::UNIX_EPOCH,
};

use colored::Colorize;
use thiserror::Error;

use crate::manifest::Manifest;

// ===============================
//        Error definitions
// ===============================

#[derive(Debug, Error)]
pub enum InitrdError {
	#[error("failed to spawn initrd build script: {0}")]
	Spawn(std::io::Error),

	#[error("initrd build script failed (exit status {0:?})")]
	NonZeroExit(ExitStatus),

	#[error("failed to serialize initrd metadata: {0}")]
	Serialize(#[from] serde_json::Error),

	#[error("filesystem error: {0}")]
	Fs(#[from] std::io::Error),

	#[error("{0}")]
	Other(String),
}

// ===============================
//        Command runner
// ===============================

pub fn run_command_with_tag_and_collect_dependencies(
	mut command: Command,
	tag: String,
) -> Result<(ExitStatus, Vec<String>), std::io::Error> {
	command.stdout(Stdio::piped());
	command.stderr(Stdio::piped());
	command.stdin(Stdio::piped());

	let mut child = command.spawn()?;
	let stderr = child.stderr.take().unwrap();
	let stdout = child.stdout.take().unwrap();

	let mut dependencies: Vec<String> = Vec::new();

	std::thread::scope(|s| {
		// STDERR thread (just prints)
		s.spawn(|| {
			let buf_reader = BufReader::new(stderr);
			for line in buf_reader.lines().filter_map(Result::ok) {
				eprintln!("{}", format!("{tag}{}", tag_line(&line, &tag)).dimmed());
			}
		});

		// STDOUT thread (prints + dependency collection)
		s.spawn(|| {
			let buf_reader = BufReader::new(stdout);
			for line in buf_reader.lines().filter_map(Result::ok) {
				let trimmed_line = line.trim();
				if trimmed_line.starts_with("DEPENDENCY ") {
					dependencies.push(trimmed_line.trim_start_matches("DEPENDENCY ").to_string());
				}
				println!("{}", format!("{tag}{}", tag_line(&line, &tag)).dimmed());
			}
		});
	});

	let status = child.wait()?;
	Ok((status, dependencies))
}

fn tag_line(line: &str, tag: &str) -> String {
	line
		.replace("\r\n", "\n")
		.replace("\r", &format!("\r{tag}"))
		.replace("\n", &format!("\n{tag}"))
}

// ===============================
//        Build initrd
// ===============================

pub fn build_initrd(manifest: &Manifest) -> Result<PathBuf, InitrdError> {
	let script_path = manifest.initrd.build_script.clone(); // PathBuf
	let output_path = PathBuf::from("build/initrd/initrd.img");
	let metadata_path = PathBuf::from("build/initrd_metadata.json");

	// --- check for rebuild necessity ---
	let need_rebuild = match read_metadata_map(&metadata_path) {
		Ok(old_map) => !metadata_up_to_date(&old_map, &script_path),
		Err(_) => true,
	};

	if !need_rebuild && output_path.exists() {
		println!(
			"{}",
			" No changes detected in initrd dependencies — using cached initrd"
				.green()
				.bold()
				.dimmed()
		);
		return Ok(output_path);
	}

	fs::create_dir_all("build")?;

	// --- execute script respecting shebang (kernel handles interpreter) ---
	let mut command = Command::new(&script_path);
	command.arg(&output_path);

	let tag = format!("{}", "[ initrd ] ".blue());
	let (status, mut deps) =
		run_command_with_tag_and_collect_dependencies(command, tag).map_err(InitrdError::Spawn)?;

	if !status.success() {
		return Err(InitrdError::NonZeroExit(status));
	}

	// --- ensure script is included in deps ---
	ensure_script_in_deps(&mut deps, &script_path);

	// --- rebuild metadata ---
	let metadata_map = build_metadata_map(&deps);
	let json = serde_json::to_string_pretty(&metadata_map)?;
	fs::write(&metadata_path, json)?;

	println!(
		"{} → {}",
		" initrd built and metadata saved".green(),
		output_path.display()
	);

	Ok(output_path)
}

// ===============================
//        Helper functions
// ===============================

fn read_metadata_map(path: &Path) -> Result<HashMap<String, u128>, InitrdError> {
	if !path.exists() {
		return Err(InitrdError::Other("metadata missing".into()));
	}
	let contents = fs::read_to_string(path)?;
	let map = serde_json::from_str::<HashMap<String, u128>>(&contents)?;
	Ok(map)
}

fn metadata_up_to_date(old_map: &HashMap<String, u128>, script_path: &Path) -> bool {
	// check all dependencies
	let deps_ok = old_map.iter().all(|(path, old_ts)| {
		fs::metadata(path)
			.and_then(|m| m.modified())
			.is_ok_and(|mtime| {
				mtime
					.duration_since(UNIX_EPOCH)
					.map(|dur| dur.as_millis() <= *old_ts)
					.unwrap_or(false)
			})
	});

	if !deps_ok {
		return false;
	}

	// check script itself
	let script_key = script_path.to_string_lossy().to_string();
	let old_script_ts = old_map.get(&script_key).copied().unwrap_or(0);

	fs::metadata(script_path)
		.and_then(|m| m.modified())
		.is_ok_and(|mtime| {
			mtime
				.duration_since(UNIX_EPOCH)
				.map(|dur| dur.as_millis() <= old_script_ts)
				.unwrap_or(false)
		})
}

fn ensure_script_in_deps(deps: &mut Vec<String>, script_path: &Path) {
	let script_str = script_path.to_string_lossy().to_string();
	if !deps.iter().any(|d| d == &script_str) {
		deps.push(script_str);
	}
}

fn build_metadata_map(deps: &[String]) -> HashMap<String, u128> {
	let mut map = HashMap::new();
	for d in deps {
		let pb = Path::new(d);
		let ts = fs::metadata(pb)
			.and_then(|m| m.modified())
			.ok()
			.and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
			.map(|dur| dur.as_millis())
			.unwrap_or(0);
		map.insert(d.clone(), ts);
	}
	map
}
