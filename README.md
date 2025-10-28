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
* Kernel configuration with per-option toggles (no `CONFIG_` prefix needed) applied to the resulting `.config`
* Containerized kernel build pipeline with cached sources
* x86_64-focused kernel build (other architectures currently unsupported)
* TOML manifest for describing the system
* Simple CLI with subcommands:

  * `image packages fetch`: downloads sources and validates the manifest
  * `image packages build`: compiles packages (without creating the image)
  * `image packages garbage-collect` (`image packages gc`): removes unused package sources
  * `image assemble`: compiles and assembles the final image
  * `kernel build`: builds the Linux kernel defined in the manifest
  * `image push`: uploads the generated image to an update server (planned)
  * `clean`: cleans build artifacts

---

## Manifest Structure

The manifest is a TOML file that describes the system:

```toml
version = "0.1-alpha"

[kernel]
url = "https://example.com/linux-kernel.tar.zst"

# Kernel config toggles (names without the CONFIG_ prefix)
[kernel.options]
EXAMPLE_FEATURE = true
ANOTHER_FEATURE = false

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
# Only fetch packages and sources
hyprpacker image packages fetch

# Compile packages without building the image (Requires docker)
hyprpacker image packages build

# Compile and assemble the final SquashFS image (Requires docker and squashfs-tools)
hyprpacker image assemble

# (Future) Upload the generated image to a server
hyprpacker image push

# Remove stale package sources
hyprpacker image packages gc

# Build the manifest's kernel (Requires docker)
hyprpacker kernel build

# Clean build artifacts
hyprpacker clean
```

All build artifacts will be put inside `build/` in the current working directory

---

## License

Hyprpacker is distributed under the MIT license.
