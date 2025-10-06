mod build;
mod fetch;
mod gc;
mod hash;
mod manifest;
mod prefix_commands;
mod size;
mod sources;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{io::ErrorKind, path::PathBuf};

#[derive(Parser, Debug)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"), long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
	#[arg(default_value = "manifest.toml", short)]
	manifest: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Removes unused source tarballs from the sources directory
	#[command(alias = "gc")]
	GarbageCollect,
	/// Pre-downloads sources for packages
	Fetch,
	/// Builds all packages without building the image
	Build,
	/// Assembles the OS ROM image
	Assemble,
	/// UNIMPLEMENTED!!! Pushes the image to the update server
	Push,
	/// Cleans up the build directory
	Clean,
}

fn main() {
	let cli = Cli::parse();
	let manifest = match std::fs::read_to_string(&cli.manifest) {
		Ok(manifest) => manifest,
		Err(e) => {
			eprintln!(
				"{}: Failed to read manifest file at {}: {e}",
				"ERROR".red().bold(),
				cli.manifest.display()
			);
			std::process::exit(1);
		}
	};
	let manifest = match toml::from_str::<manifest::Manifest>(&manifest) {
		Ok(manifest) => manifest,
		Err(e) => {
			eprintln!(
				"{}: Failed to parse manifest file at {}: {e}",
				"ERROR".red().bold(),
				cli.manifest.display()
			);
			std::process::exit(1);
		}
	};
	match cli.command {
		Commands::GarbageCollect => gc::gc_command(manifest),
		Commands::Fetch => {
			gc::gc_command(manifest.clone());
			let result = fetch::fetch(manifest);
			result.print();
			result.exit_if_failure();
		}
		Commands::Build => {
			gc::gc_command(manifest.clone());
			let fetch_result = fetch::fetch(manifest.clone());
			fetch_result.print();
			fetch_result.exit_if_failure();
			let build_result = build::build(manifest);
			build_result.print();
			build_result.exit_if_failure();
		}
		Commands::Assemble => {
			// TODO: implementar
		}
		Commands::Push => {
			todo!("push command")
		}
		Commands::Clean => {
			std::fs::remove_dir_all("build").unwrap_or_else(|e| {
				if let ErrorKind::NotFound = e.kind() {
					println!("{}", "✔ Build directory already clean".green().bold());
					std::process::exit(0);
				} else {
					eprintln!(
						"{}: Failed to clean build directory: {e}",
						"ERROR".red().bold()
					);
				}
				std::process::exit(1);
			});
			println!("{}", "Build directory cleaned successfully".green());
			std::process::exit(0);
		}
	}
}
