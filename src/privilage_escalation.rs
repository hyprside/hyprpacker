use std::{
	convert::Infallible,
	env, io,
	process::{Command, exit},
};

/// Re-executes the current binary with the same args using an elevation tool.
/// Tries `sudo`, then `doas`, then `su`.
///
/// Returns `Ok(())` only if this function does not error before exiting the process.
/// Use carefully: on success the current process will `exit(0)`.
pub fn reexec_with_elevation() -> io::Result<Infallible> {
	let exe = env::current_exe()?;
	let args: Vec<String> = env::args().skip(1).collect();

	// candidates in order of preference
	let candidates = ["sudo", "doas", "su"];

	for &cmd in &candidates {
		match try_run_with(cmd, &exe, &args) {
			Ok(status_ok) => {
				if status_ok {
					// successful elevated run — exit the current process.
					exit(0);
				} else {
					// command ran but returned non-zero -> treat as error
					return Err(io::Error::new(
						io::ErrorKind::Other,
						format!("{} returned non-zero exit code", cmd),
					));
				}
			}
			Err(e) => {
				// If the error is NotFound, try next candidate.
				// Otherwise return the error immediately.
				if e.kind() == io::ErrorKind::NotFound {
					// try next candidate
					continue;
				} else {
					return Err(e);
				}
			}
		}
	}

	Err(io::Error::new(
		io::ErrorKind::NotFound,
		"no privilege escalation tool found (sudo/doas/su)",
	))
}

/// Try to run the exe+args with `cmd`.
/// Returns:
///  - Ok(true)  -> child ran and exited with status 0
///  - Ok(false) -> child ran and exited with non-zero status
///  - Err(e)    -> spawn or wait error (including NotFound when the command binary doesn't exist)
fn try_run_with(cmd: &str, exe: &std::path::Path, args: &[String]) -> io::Result<bool> {
	// Special handling for `su`: we must pass a single string to `su -c`.
	// Build a single shell-escaped command string.
	// We quote each arg safely using single quotes and escape existing single quotes.
	fn shell_escape(arg: &str) -> String {
		if arg.is_empty() {
			"''".to_string()
		} else if !arg.contains('\'') {
			format!("'{}'", arg)
		} else {
			// replace ' with '\'' (POSIX shell trick)
			let replaced = arg.replace('\'', r#"'\'"'"#);
			format!("'{}'", replaced)
		}
	}

	let mut parts: Vec<String> = Vec::with_capacity(1 + args.len());
	parts.push(shell_escape(&exe.to_string_lossy()));
	for a in args {
		parts.push(shell_escape(a));
	}
	let command_str = parts.join(" ");

	// spawn su -c '<command_str>'
	let child = if cmd == "su" {
		Command::new(cmd).arg("-c").arg(command_str).spawn()
	} else {
		Command::new(cmd)
			.arg("su")
			.arg("-c")
			.arg(command_str)
			.spawn()
	};

	let mut child = match child {
		Ok(c) => c,
		Err(e) => return Err(e),
	};

	let status = child.wait()?;
	return Ok(status.success());
}

/// Ensure the current process runs as root. If already root, returns normally.
/// Otherwise attempts to re-exec the binary with elevated privileges (via
/// `reexec_with_elevation`). If escalation is unsuccessful, this function
/// will print an error and terminate the process with exit code 1.
///
/// Note: `reexec_with_elevation()` is expected to either `exit(0)` on success
/// (after launching the elevated child) or return an `Err(io::Error)` on failure.
pub fn ensure_root() {
	// libc::geteuid is used to check effective UID without external crates.
	let euid = unsafe { libc::geteuid() };
	if euid == 0 {
		// Already root — continue normal execution.
		return;
	}

	// Not root -> try to escalate. If escalation succeeds, `reexec_with_elevation`
	// will spawn the elevated child and exit the current process (so we never return).
	// If it returns Err, escalation failed and we must abort.
	match reexec_with_elevation() {
		Ok(_) => {
			// In practice this branch is unreachable because reexec_with_elevation()
			// exits the current process on success. But handle defensively:
			eprintln!("Privilege escalation returned unexpectedly; aborting.");
			std::process::exit(1);
		}
		Err(err) => {
			eprintln!(
				"Failed to obtain root privileges (sudo/doas/su). Error: {}",
				err
			);
			std::process::exit(1);
		}
	}
}
