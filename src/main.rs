mod commands;
mod credits;
mod fs_utils;
mod hash;
mod manifest;
mod prefix_commands;
mod privilage_escalation;
mod size;
mod sources;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{io::ErrorKind, path::PathBuf};

use crate::{
	commands::{
		image::{self, packages},
		kernel,
	},
	privilage_escalation::ensure_root,
};

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
	/// Image-related operations
	Image {
		#[command(subcommand)]
		command: ImageCommands,
	},
	/// Kernel related helpers
	Kernel {
		#[command(subcommand)]
		command: KernelCommands,
	},
	/// Cleans up the build directory
	Clean,
}

#[derive(Subcommand, Debug)]
enum KernelCommands {
	/// Builds the Linux kernel defined in the manifest
	Build,
}

#[derive(Subcommand, Debug)]
enum ImageCommands {
	/// Assembles the OS ROM image
	Assemble,
	/// Package management helpers for image builds
	Packages {
		#[command(subcommand)]
		command: PackageCommands,
	},
	/// UNIMPLEMENTED!!! Pushes the image to the update server
	Push,
}

#[derive(Subcommand, Debug)]
enum PackageCommands {
	/// Removes unused source tarballs from the sources directory
	#[command(alias = "gc")]
	GarbageCollect,
	/// Pre-downloads sources for packages
	Fetch,
	/// Builds all packages without building the image
	Build,
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
		Commands::Image { command } => match command {
			ImageCommands::Assemble => {
				packages::gc_command(&manifest);
				let fetch_result = packages::fetch(&manifest);
				fetch_result.print();
				fetch_result.exit_if_failure();
				let build_result = packages::build(&manifest);
				build_result.print();
				build_result.exit_if_failure();
				println!("{}", "  Assembling image".blue().bold());
				let assemble_result = image::assemble(&manifest);
				match assemble_result {
					Ok(image_path) => {
						println!(
							"{} {}",
							"✔ Assembled image".green().bold(),
							image_path.display().to_string().green().bold()
						);
					}
					Err(image::AssembleError::CopyError {
						package: pkg,
						error,
					}) => {
						eprintln!(
							"  {} {} {} {}: {}",
							" 󱁥  Failed copying ".bold().red(),
							pkg.name.red().bold(),
							pkg.version.dimmed(),
							"to sysroot".red(),
							error.to_string().red()
						);
					}
					Err(image::AssembleError::SquashfsError(e)) => {
						eprintln!();
						eprintln!(
							"    {}: {}",
							" 󱁥  Failed to create image".bold().red(),
							e.to_string().red()
						);
						eprintln!();
						fn hyperlink(link: impl core::fmt::Display, text: impl core::fmt::Display) -> String {
							format!("\x1b]8;;{link}\x1b\\{text}\x1b]8;;\x1b\\")
						}
						if let image::SquashFsError::CommandError(e) = e {
							if let ErrorKind::NotFound = e.kind() {
								eprintln!(
									"    {}: This is likely due to {} not being installed. {}",
									"help".bold().cyan(),
									hyperlink(
										"https://github.com/plougher/squashfs-tools",
										"squashfs-tools".bold().underline()
									),
									"Make sure it is installed and try again.".bold()
								);
								eprintln!();
							}
						}
					}
					Err(image::AssembleError::Io(e)) => {
						eprintln!();
						eprintln!(
							"    {}: {}",
							" 󱁥  Failed to create image due to an IO error"
								.bold()
								.red(),
							e.to_string().red().dimmed()
						);
						eprintln!();
					}
				}
			}
			ImageCommands::Packages { command } => match command {
				PackageCommands::GarbageCollect => packages::gc_command(&manifest),
				PackageCommands::Fetch => {
					packages::gc_command(&manifest);
					let result = packages::fetch(&manifest);
					result.print();
					result.exit_if_failure();
				}
				PackageCommands::Build => {
					packages::gc_command(&manifest);
					let fetch_result = packages::fetch(&manifest);
					fetch_result.print();
					fetch_result.exit_if_failure();
					let build_result = packages::build(&manifest);
					build_result.print();
					build_result.exit_if_failure();
				}
			},
			ImageCommands::Push => {
				todo!("push command")
			}
		},
		Commands::Kernel { command } => match command {
			KernelCommands::Build => match kernel::build(&manifest) {
				Ok(result) => result.print(),
				Err(e) => {
					eprintln!("{}: Failed to build kernel: {}", "ERROR".red().bold(), e);
					std::process::exit(1);
				}
			},
		},
		Commands::Clean => {
			std::fs::remove_dir_all("build").unwrap_or_else(|e| {
				if let ErrorKind::NotFound = e.kind() {
					println!("{}", "✔ Build directory already clean".green().bold());
					std::process::exit(0);
				} else {
					ensure_root();
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
