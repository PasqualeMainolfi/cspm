# cspm: A Modern Package Manager for Csound

**NOTE: THIS PROJECT IS CURRENTLY A WORK IN PROGRESS. A REMOTE REGISTRY IS NOT YET AVAILABLE, BUT IT WILL BE IMPLEMENTED SOON!**      

**cspm** is a modern, fast, and deterministic package and project manager for the [Csound](https://csound.com/), written in Rust.

It brings modern dependency management (similar to `cargo` or `npm`) to the Csound ecosystem.  
With `cspm`, you can easily initialize projects, manage third-party modules (UDOs) or projects (CSD), share and download modules from the community, and create fully reproducible projects through manifests and lockfiles. 
This ensures that the exact same module versions are used across different systems and environments.

---

## Features

- **Simple Dependency Management**: Add, update, or remove Csound modules with a single command.
- **Deterministic Builds**: Uses a Cspm.lock file with cryptographic checksums (SHA-256) to ensure your project produces exactly the same results on any machine.
- **Recursive Dependency Resolution**: Automatically resolves and installs complete dependency trees.
- **Global Module Cache**: Modules are downloaded only once and stored in a global cache, allowing subsequent projects to reuse them instantly.
- **Local or Global Installation**: Modules can be installed either locally within a project or globally for system-wide use.
- **Module Discovery**: Search the registry to quickly find available modules and versions.
- **TOML-based Manifests**: Clean and human-readable Cspm.toml files define your project configuration and dependencies.
- **Built-in Runner**: Run your Csound projects directly with cspm run.
- **Plugin Installation via Risset**: Install and manage Csound plugins directly through Risset, integrated into cspm.

---

## Installation

*Note: Pre-compiled binaries will be available in the Releases tab soon.*

If you have [Rust and Cargo](https://rustup.rs/) installed, you can build and install `cspm` from source:

```bash
git clone <https://github.com/PasqualeMainolfi/cspm.git>
cd cspm
cargo install --path .
```

Verify the installation

```bash
cspm --version
```

Add a community module to your project:

```bash
cspm add [module]
```

```bash
cspm add [module1, module2, ...]
```

Run a `.csd` project

```bash
cspm run [cs_options]
```

Install plugins via risset

```bash
cspm risset install [plugin]
```

Or, you can download a shared `.csd` or `orc/sco` Csound project and build it from manifest or lock file

```bash
cspm take [project]
cd project
cspm build
```

## Cspm.toml file

```toml
[package]
name = "my_synth" # name of module/project
version = "0.1.0"
mode = "cs-module" # package mode: cs-module (.udo) or cs-project (.csd or .orc/.sco)
description = "An awesome FM synthesizer"
authors =["<you@example.com>"]
license = "MIT"
cs_version = "7.0" # csound version
include = ["src"] # files/folders to include in the package
plugins = []

[dependencies]
module1 = "1.0.0"
module2 = "2.1.3"

[main]
src = "src" # main folder
csd = "src/main.csd" # entry point
# orc = "src/main.orc"
# sco = "src/main.sco"
```

## Command reference

```text
Hello, from cspm!
A modern package manager for Csound

Usage: cspm <COMMAND>

Commands:
  init                    Create a new Csound project
  add                     Add dependencies to the project
  install                 Install modules globally (without manifest)
  uninstall               Uninstall modules globally (without manifest)
  upgrade                 Upgrade global modules (without manifest)
  reinstall               Reinstall dependencies to the project
  remove                  Remove dependencies from the project
  update                  Update the project dependencies
  cache                   Manage cspm cache
  sync                    Check the project environment status
  build                   Build project from manifest or lock file
  publish                 Validate module structure and metadata before creating a Pull Request
  run                     Run Csound project
  validate                Check Cspm.toml file and fixes issues automatically
  risset                  Install plugins using risset
  search                  Display module info
  take                    Download a shared Csound project
  version, -v, --version  Display cspm version
  help                    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```
