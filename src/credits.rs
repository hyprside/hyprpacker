use std::fs;
use serde::Serialize;
use serde_json::json;
use crate::manifest::Package;
#[derive(Serialize)]
pub struct PackageCredit {
    pub name: String,
    pub author: String,
}

fn parse_pkginfo(contents: &str) -> Option<PackageCredit> {
    let mut pkgname: Option<String> = None;
    let mut author: Option<String> = None;

    for line in contents.lines() {
        if line.starts_with("pkgname = ") {
            pkgname = Some(line["pkgname = ".len()..].trim().trim_matches('"').to_string());
        }
        if line.starts_with("packager = ") {
            author = Some(line["packager = ".len()..].trim().trim_matches('"').to_string());
        }
    }

    match (pkgname, author) {
        (Some(p), Some(a)) => Some(PackageCredit { name: p, author: a }),
        _ => None,
    }
}

fn package_credit(pkg: &Package) -> Option<PackageCredit> {
    let pkginfo_path = pkg.get_out_unpacked_dir().join(".PKGINFO");
    let contents = fs::read_to_string(pkginfo_path).ok()?;
    let (name, author) = parse_pkginfo(&contents)?;
    Some(PackageCredit { name, author })
}

pub fn generate_credits(packages: &[Package]) -> Vec<PackageCredit> {
    packages.iter()
        .filter_map(package_credit)
        .collect()
}
