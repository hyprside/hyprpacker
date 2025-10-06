use std::{
	io::{BufRead, BufReader},
	process::{Command, ExitStatus, Stdio},
};

use colored::Colorize;

/// Runs commands but adds a tag to each log line the process prints to the stdout/stderr
pub fn run_command_with_tag(
	mut command: Command,
	tag: String,
) -> Result<ExitStatus, std::io::Error> {
	command.stdout(Stdio::piped());
	command.stderr(Stdio::piped());
	command.stdin(Stdio::piped());
	let mut child = command.spawn()?;
	let stderr = child.stderr.take().unwrap();
	let stdout = child.stdout.take().unwrap();
	std::thread::scope(|s| {
		s.spawn(|| {
			let buf_reader = BufReader::new(stderr);
			for line in buf_reader.lines().filter_map(Result::ok) {
				eprintln!("{}", format!("{tag}{line}").dimmed());
			}
		});
		s.spawn(|| {
			let buf_reader = BufReader::new(stdout);
			for line in buf_reader.lines().filter_map(Result::ok) {
				println!("{}", format!("{tag}{line}").dimmed());
			}
		});
	});
	let status = child.wait()?;
	Ok(status)
}
