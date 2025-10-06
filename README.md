# Hyprpacker

**Hyprpacker** is a command-line utility for building and packaging the Hyprside Linux system. It allows you to manage packages from multiple sources, compile them, and assemble final images for distribution in a modular and incremental way.

---

## Features

* Package management from:

  * Local PKGBUILD
  * Remote Git repository
  * Precompiled Arch Linux binary packages
* Incremental package build
* Final image assembly in SquashFS format
* TOML manifest for describing the system
* Simple CLI with subcommands:

  * `fetch`: downloads packages and validates the manifest
  * `build`: compiles packages (without creating the image)
  * `assemble`: compiles and assembles the final image
  * `push`: uploads the generated image to an update server (planned)

---

## Manifest Structure

The manifest is a TOML file that describes the system:

```toml
version = "0.1-alpha"

[[packages]]
name = "glibc"
version = "2.39"
source = { type = "binary", url = "https://archlinux.org/packages/core/x86_64/glibc/download" }

[[packages]]
name = "mesa"
version = "git"
source = { type = "git", repo = "https://gitlab.freedesktop.org/mesa/mesa.git", rev = "main" }

[[packages]]
name = "tibs"
version = "0.1"
source = { type = "pkgbuild", path = "./pkgs/tibs" }
```

---

## Installation

> Requirements: Rust >= 1.70, Cargo

Clone the repository and build:

```bash
git clone https://github.com/yourusername/hyprpacker.git
cd hyprpacker
cargo build --release
```

The binary will be available in `target/release/hyprpacker`.

---

## Usage

```bash
# Only fetch packages
hyprpacker fetch manifest.toml

# Compile packages without building the image (Requires docker)
hyprpacker build manifest.toml

# Compile and assemble the final SquashFS image (Requires docker)
hyprpacker assemble manifest.toml

# (Future) Upload the generated image to a server
hyprpacker push manifest.toml
```

All build artifacts will be put inside `build/` in the current working directory

---

## License

Hyprpacker is distributed under the MIT license.
