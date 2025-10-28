use crate::manifest::Package;
use serde::Serialize;
use std::fs;

#[derive(Serialize)]
pub struct PackageCredit {
	pub name: String,
	pub author: String,
}

fn parse_pkginfo(contents: &str) -> Option<(String, String)> {
	let mut pkgname: Option<String> = None;
	let mut packager: Option<String> = None;

	for line in contents.lines() {
		if line.starts_with("pkgname = ") {
			pkgname = Some(
				line["pkgname = ".len()..]
					.trim()
					.trim_matches('"')
					.to_string(),
			);
		}
		if line.starts_with("packager = ") {
			packager = Some(
				line["packager = ".len()..]
					.trim()
					.trim_matches('"')
					.to_string(),
			);
		}
	}

	match (pkgname, packager) {
		(Some(p), Some(a)) => Some((p, a)),
		_ => None,
	}
}

fn package_credit(pkg: &Package) -> Option<PackageCredit> {
	if let Some(author) = &pkg.author {
		return Some(PackageCredit {
			name: pkg.name.clone(),
			author: author.clone(),
		});
	}

	let pkginfo_path = pkg.get_out_unpacked_dir().join(".PKGINFO");
	if let Ok(contents) = fs::read_to_string(pkginfo_path) {
		if let Some((name, author)) = parse_pkginfo(&contents) {
			return Some(PackageCredit { name, author });
		}
	}

	Some(PackageCredit {
		name: pkg.name.clone(),
		author: "Unknown".into(),
	})
}

pub fn generate_credits(packages: &[Package]) -> Vec<PackageCredit> {
	packages.iter().filter_map(package_credit).collect()
}
