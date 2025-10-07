use std::os::unix::ffi::OsStrExt;
use std::{fs::DirEntry, path::Path, time::SystemTime};

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
	copy_dir_all_with_filter(src, dst, |_| true)
}
pub fn copy_dir_all_with_filter(
	src: impl AsRef<Path>,
	dst: impl AsRef<Path>,
	filter: impl Fn(&DirEntry) -> bool,
) -> std::io::Result<()> {
	std::fs::create_dir_all(&dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		if !filter(&entry) {
			continue;
		}
		let ty = entry.file_type()?;
		if ty.is_dir() {
			copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
		} else {
			std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
		}
	}
	Ok(())
}

pub fn has_file_newer_than(dir: &Path, timestamp: SystemTime) -> std::io::Result<bool> {
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
