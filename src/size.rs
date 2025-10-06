pub fn human_readable_size(bytes: u64) -> String {
	const KB: u64 = 1024;
	const MB: u64 = KB * 1024;
	const GB: u64 = MB * 1024;
	const TB: u64 = GB * 1024;

	match bytes {
		b if b >= TB => format!("{:.2} TB", b as f64 / TB as f64),
		b if b >= GB => format!("{:.2} GB", b as f64 / GB as f64),
		b if b >= MB => format!("{:.2} MB", b as f64 / MB as f64),
		b if b >= KB => format!("{:.2} KB", b as f64 / KB as f64),
		b => format!("{} bytes", b),
	}
}
