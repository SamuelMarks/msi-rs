# User Guide & Usage Manual: `msi-rs`

`msi-rs` is a complete, memory-safe, pure-Rust implementation of the Microsoft Windows Installer technology stack. It provides cross-platform readers, writers, compilers, execution runtimes, and desktop/terminal UI environments across Linux, macOS, FreeBSD, illumos/Solaris, and Windows.

Whether you need to:
1. Build Windows Installer (`.msi`) packages natively on Linux or macOS CI/CD runners without Windows or Wine,
2. Execute, repair, or uninstall `.msi` packages on POSIX systems with two-phase rollback transactions,
3. Replace the WiX Toolset (`candle`, `light`, `dark`, `heat`, `wix`, `torch`, `pyro`, `lit`, `smoke`),
4. Replace GNOME MSI tools (`msiinfo`, `msibuild`, `msidump`, `msidiff`, `msiextract`),
5. Embed MSI creation, manipulation, or SQL querying directly into a Rust application, or
6. Automate installer packaging and extraction via Python scripts,

this guide provides end-to-end instructions and concrete, copy-pasteable examples.

---

## Table of Contents

- [1. Installation and Setup](#1-installation-and-setup)
  - [Building from Source (Rust Workspace)](#building-from-source-rust-workspace)
  - [Installing Python Bindings](#installing-python-bindings)
  - [Binary Overview](#binary-overview)
- [2. Command-Line Interface (CLI) Guide](#2-command-line-interface-cli-guide)
  - [Primary Tool: `msi-cli`](#primary-tool-msi-cli)
  - [Direct `msiexec.exe` Command-Line Parity](#direct-msiexecexe-command-line-parity)
  - [WiX Toolset Compatible Binaries](#wix-toolset-compatible-binaries)
    - [`candle`: WiX Compiler](#candle-wix-compiler)
    - [`light`: WiX Linker and Binder](#light-wix-linker-and-binder)
    - [`wix`: Unified Modern WiX CLI](#wix-unified-modern-wix-cli)
    - [`dark`: MSI Decompiler](#dark-msi-decompiler)
    - [`heat`: Source Harvester](#heat-source-harvester)
    - [`torch`: Database Transform Generator (`.mst`)](#torch-database-transform-generator-mst)
    - [`pyro`: Patch Package Builder (`.msp`)](#pyro-patch-package-builder-msp)
    - [`lit`: WiX Library Builder (`.wixlib`)](#lit-wix-library-builder-wixlib)
    - [`smoke`: Package Validator (ICE Engine)](#smoke-package-validator-ice-engine)
  - [GNOME MSI Toolset Compatible Binaries](#gnome-msi-toolset-compatible-binaries)
    - [`msiinfo`: Metadata, Tables, and Stream Inspector](#msiinfo-metadata-tables-and-stream-inspector)
    - [`msibuild`: Stream and IDT Manipulator](#msibuild-stream-and-idt-manipulator)
    - [`msidump`: Package Archive Dumper](#msidump-package-archive-dumper)
    - [`msidiff`: Relational and Stream Diffing](#msidiff-relational-and-stream-diffing)
    - [`msiextract`: Cabinet Payload Extractor](#msiextract-cabinet-payload-extractor)
  - [Graphical (GUI) and Terminal (TUI) Installers](#graphical-gui-and-terminal-tui-installers)
    - [`msi-gui`: Interactive Desktop Wizard](#msi-gui-interactive-desktop-wizard)
    - [Terminal TUI Wizard (`msi-cli --tui`)](#terminal-tui-wizard-msi-cli---tui)
- [3. Rust Library Usage Guide (`msi`)](#3-rust-library-usage-guide-msi)
  - [Adding `msi` to `Cargo.toml`](#adding-msi-to-cargotoml)
  - [Creating Packages with `PackageBuilder`](#creating-packages-with-packagebuilder)
  - [Reading, Inspecting, and Modifying Packages](#reading-inspecting-and-modifying-packages)
  - [Executing Windows Installer SQL Dialect](#executing-windows-installer-sql-dialect)
  - [Cabinet Archive Compression and Extraction (`msi::cab`)](#cabinet-archive-compression-and-extraction-msicab)
  - [Compiling WiX XML Programmatically](#compiling-wix-xml-programmatically)
  - [Two-Phase Transactional Execution Runtime](#two-phase-transactional-execution-runtime)
- [4. Python Library Usage Guide (`msi`)](#4-python-library-usage-guide-msi)
  - [Quickstart: Directory-to-MSI One-Liner (`msi.build_msi`)](#quickstart-directory-to-msi-one-liner-msibuild_msi)
  - [Fluent Package Construction (`msi.PackageBuilder`)](#fluent-package-construction-msipackagebuilder)
  - [Inspecting Packages and Querying Tables (`msi.Package`)](#inspecting-packages-and-querying-tables-msipackage)
  - [Compiling WiX XML from Python (`msi.compile_wix`)](#compiling-wix-xml-from-python-msicompile_wix)
  - [Package Inspection Dictionary (`msi.inspect_msi`)](#package-inspection-dictionary-msiinspect_msi)
  - [Identifier Sanitization and ProductVersion Utilities](#identifier-sanitization-and-productversion-utilities)
  - [Error Handling](#error-handling)
- [5. Production CI/CD Recipes & Workflows](#5-production-cicd-recipes--workflows)
  - [Recipe 1: GitHub Actions Linux CI Building Windows MSI](#recipe-1-github-actions-linux-ci-building-windows-msi)
  - [Recipe 2: Automated Python Build Script for Release Artifacts](#recipe-2-automated-python-build-script-for-release-artifacts)
  - [Recipe 3: Headless Silent Deployment in Docker / Kubernetes](#recipe-3-headless-silent-deployment-in-docker--kubernetes)

---

## 1. Installation and Setup

### Building from Source (Rust Workspace)

Prerequisites:
- Rust 1.85+ (stable toolchain)
- `cargo`

```bash
# Clone the repository
git clone https://github.com/SamuelMarks/msi-rs.git
cd msi-rs

# Compile all workspace crates and CLI binaries with release optimizations
cargo build --release

# The compiled binaries will be located in target/release/
ls -la target/release/msi-cli
ls -la target/release/candle target/release/light target/release/wix
ls -la target/release/msiinfo target/release/msiextract
```

To install all executables to your Cargo bin directory (`~/.cargo/bin`):

```bash
cargo install --path crates/msi-cli
cargo install --path crates/msi-gui

# Optional convenience alias for standard shell environments
alias msi=msi-cli
```

### Installing Python Bindings

The Python package `msi` provides high-performance native Rust bindings via PyO3:

```bash
# In an active virtual environment (Python 3.9+)
pip install maturin
maturin develop --release -m crates/msi-python/Cargo.toml

# Or build a standalone wheel
maturin build --release -m crates/msi-python/Cargo.toml --out dist/
pip install dist/msi-*.whl
```

### Binary Overview

| Binary | Description | Compatible Upstream Tool |
| :--- | :--- | :--- |
| `msi-cli` | Main installer runtime, manager, and `msiexec` emulator (can be aliased as `msi`) | Microsoft `msiexec.exe` |
| `candle` | WiX v3 XML compiler (`.wxs` -> `.wixobj`) | WiX Toolset `candle.exe` |
| `light` | WiX v3 linker & binder (`.wixobj` -> `.msi`) | WiX Toolset `light.exe` |
| `wix` | Unified WiX v4/v5 multi-command toolchain | WiX Toolset `wix.exe` |
| `dark` | Decompiles `.msi` databases back to WiX XML | WiX Toolset `dark.exe` |
| `heat` | Harvester for directories, files, and registry | WiX Toolset `heat.exe` |
| `torch` | Database transform generator (`.mst`) | WiX Toolset `torch.exe` |
| `pyro` | Patch builder (`.msp`) | WiX Toolset `pyro.exe` |
| `lit` | WiX library builder (`.wixlib`) | WiX Toolset `lit.exe` |
| `smoke` | Standalone ICE validation tool | WiX Toolset `smoke.exe` |
| `msiinfo` | Summary information, table, and stream inspector | GNOME `msiinfo` |
| `msibuild` | Low-level stream and table manipulator | GNOME `msibuild` |
| `msidump` | Archive dumper (IDT tables and binary streams) | GNOME `msidump` |
| `msidiff` | Database diff tool | GNOME `msidiff` |
| `msiextract` | File payload extractor from cabinets | GNOME `msiextract` |
| `msi-gui` | Desktop native GUI installer wizard | Windows Installer UI |

---

## 2. Command-Line Interface (CLI) Guide

### Primary Tool: `msi-cli`

The `msi-cli` executable (also aliased as `msi` throughout this guide) provides a modern subcommand interface alongside direct `msiexec.exe` flag parity.

#### 1. Inspecting Package Information (`info`)

```bash
msi info ./MyApplication.msi
```

Output:
```text
Package: MyApplication
Version: 1.2.0
Manufacturer: Acme Corporation
ProductCode: {12345678-1234-1234-1234-1234567890AB}
```

#### 2. Installing Packages (`install`)

```bash
# Full interactive GUI installation
msi install ./MyApplication.msi --ui full

# Basic UI with progress bar
msi install ./MyApplication.msi --ui basic

# Quiet / silent installation with log file and property overrides
msi install ./MyApplication.msi 
  --ui quiet 
  --log ./install.log 
  INSTALLDIR="/opt/myapp" APP_ENV="production"
```

Available `--ui` options:
- `quiet`: Completely silent without UI (`/qn`).
- `basic`: Basic progress bar (`/qb`).
- `basic-no-cancel`: Progress bar without Cancel button (`/qb!`).
- `reduced`: Reduced UI (`/qr`).
- `full`: Full interactive modal dialogs (`/qf`).

#### 3. Uninstalling Packages (`uninstall`)

```bash
# Uninstall silently using package file
msi uninstall ./MyApplication.msi --ui quiet --log ./uninstall.log

# Uninstall with basic UI
msi uninstall ./MyApplication.msi --ui basic
```

#### 4. Repairing Installations (`repair`)

```bash
# Repair with standard flags (reinstall missing files, registry, shortcuts)
msi repair ./MyApplication.msi --flags omus --log ./repair.log

# Force reinstall all files regardless of checksum/version
msi repair ./MyApplication.msi --flags a
```

Standard repair flag characters (`--flags`):
- `p`: Reinstall only if file is missing.
- `o`: Reinstall if file is missing or older version.
- `e`: Reinstall if file is missing or equal/older version.
- `d`: Reinstall if file is missing or different version.
- `c`: Reinstall if file is missing or checksum does not match.
- `a`: Force all files to be reinstalled.
- `u`: Rewrite all required user-specific registry entries.
- `m`: Rewrite all required computer-specific registry entries.
- `s`: Reinstall all shortcuts and overwrite existing icons.
- `v`: Run from source and re-cache local package.

#### 5. Administrative Installation (`admin`)

Creates a source image on a network share or local directory for administrative deployment:

```bash
msi admin ./MyApplication.msi --ui basic --log ./admin.log
```

#### 6. Advertising Products (`advertise`)

Advertises product features to the system without copying local binaries:

```bash
# Machine-level advertisement
msi advertise ./MyApplication.msi

# Per-user advertisement
msi advertise ./MyApplication.msi --user
```

#### 7. Applying Patch Packages (`patch`)

```bash
msi patch ./MyApplication.msi ./Update1.msp --ui basic --log ./patch.log
```

#### 8. Harvesting Files into WiX XML (`harvest`)

```bash
# Harvest a directory tree into a WiX ComponentGroup
msi harvest ./dist 
  --mode dir 
  --group AppComponents 
  --dir-id INSTALLFOLDER 
  --output ./harvested.wxs

# Harvest a Windows .reg file into WiX RegistryValue elements
msi harvest ./settings.reg 
  --mode reg 
  --group RegSettings 
  --output ./registry.wxs
```

#### 9. Decompiling an MSI Database (`decompile`)

```bash
# Decompile database into WiX XML and extract embedded cabinets/streams
msi decompile ./MyApplication.msi 
  --output ./MyApplication.wxs 
  --extract-assets ./assets/
```

#### 10. Privileged Worker Daemon (`worker`)

Runs the background privileged transaction executor listening on a Unix domain socket or Windows named pipe:

```bash
msi worker --worker-socket /tmp/msi_worker_0123.sock
```

---

### Direct `msiexec.exe` Command-Line Parity

`msi-cli` detects when the first argument starts with `/` or `-` and switches directly to standard Win32 `msiexec.exe` syntax.

```bash
# Silent install with property override and verbose logging
msi /i ./Setup.msi /qn /l*vx ./install.log TARGETDIR=/opt/custom

# Silent uninstall
msi /x ./Setup.msi /qn

# Basic UI repair
msi /fomus ./Setup.msi /qb

# Administrative deployment
msi /a ./Setup.msi /qb TARGETDIR=/srv/share/setup

# Apply patch
msi /p ./Update.msp /qb

# Advertise per-user
msi /ju ./Setup.msi
```

#### MSI Logging Flags (`/l`)

Combine standard logging mode characters with `/l`:
- `i`: Status messages.
- `w`: Non-fatal warnings.
- `e`: All error messages.
- `a`: Start of actions.
- `r`: Action-specific records.
- `u`: User requests.
- `c`: Initial UI parameters.
- `m`: Out-of-memory or fatal exit information.
- `o`: Out-of-disk-space messages.
- `p`: Terminal properties.
- `v`: Verbose output.
- `x`: Extra debugging information.
- `+`: Append to existing log file.
- `!`: Flush each line immediately to disk.
- `*`: Log all information except `v` and `x` (use `/l*vx` for maximum detail).

#### Exit Codes

`msi-rs` strictly conforms to standard Windows Installer exit codes:
- `0`: `MSI_ERROR_SUCCESS` — Operation completed successfully.
- `1602`: `MSI_ERROR_USER_CANCEL` — User cancelled installation.
- `1603`: `MSI_ERROR_FATAL` — Fatal error occurred during execution.
- `1605`: `MSI_ERROR_NOT_INSTALLED` — Action valid only for currently installed products.
- `3010`: `MSI_ERROR_SUCCESS_REBOOT_REQUIRED` — Operation completed successfully, restart required.

---

### WiX Toolset Compatible Binaries

`msi-rs` ships drop-in binary replacements for the WiX toolchain.

#### `candle`: WiX Compiler

Compiles WiX source files (`.wxs`, `.wxi`) into intermediate object files (`.wixobj`):

```bash
# Basic compilation
candle Product.wxs -out Product.wixobj

# Target architecture, preprocessor defines, and include search directories
candle 
  -nologo 
  -arch x64 
  -dVersion=1.2.0 
  -dBuildType=Release 
  -I./shared_includes 
  -out obj/Product.wixobj 
  Product.wxs
```

Supported flags:
- `-arch <x86|x64|arm64>`: Target processor architecture.
- `-d<var>=<val>`: Define preprocessor variable `$(var.NAME)`.
- `-I<dir>`: Add directory to include resolution search path (`<?include?>`).
- `-ext <extension>`: Load WiX compiler extension.
- `-out <path>`: Destination path for `.wixobj`.
- `-nologo`: Suppress copyright banner.
- `-q` / `-quiet`: Suppress compiler informational messages.

#### `light`: WiX Linker and Binder

Links `.wixobj` and `.wixlib` files, resolves symbols, packs cabinets, runs ICE validation, and emits `.msi`:

```bash
# Link intermediate object to MSI
light Product.wixobj -out MyApplication.msi

# Advanced link: specify base file directory, cultures, and localization file
light 
  -nologo 
  -b ./payload_files 
  -cultures:en-US 
  -loc Strings.wxl 
  -sice:ICE18 
  -out bin/MyApplication.msi 
  obj/Product.wixobj obj/Harvested.wixobj
```

Supported flags:
- `-b <dir>`: Base path for resolving relative source file paths on disk.
- `-loc <file.wxl>`: Provide WiX localization catalog (`!(loc.StringId)`).
- `-cultures:<cultures>`: Target localization cultures (e.g. `en-US;fr-FR`).
- `-sice:<rule>`: Suppress specific ICE validation rule (e.g. `-sice:ICE03`).
- `-sval`: Suppress all ICE validation checks.
- `-out <path>`: Destination path for `.msi`.

#### `wix`: Unified Modern WiX CLI

Replicates WiX v4/v5 multi-command toolchain:

```bash
# 1. Build an MSI directly from source XML in one pass
wix build 
  -arch x64 
  -d Version=2.0.0 
  -b ./dist 
  -o bin/MyApplication.msi 
  src/Package.wxs

# 2. Harvest directory into WiX source XML
wix harvest dir ./dist 
  -cg AppFiles 
  -dr INSTALLDIR 
  -o src/Harvested.wxs

# 3. Clean intermediate build artifacts (*.wixobj, *.wixpdb, *.cab)
wix clean ./src

# 4. Decompile MSI back to source
wix msi decompile -o Decompiled.wxs bin/MyApplication.msi
```

#### `dark`: MSI Decompiler

Decompiles any `.msi` or `.msm` database back into conforming WiX source XML:

```bash
# Decompile into XML and extract embedded cabinet files to ./assets
dark -x ./assets -o Product.wxs MyApplication.msi

# Suppress UI dialog decompilation
dark -sui -o HeadlessProduct.wxs MyApplication.msi
```

#### `heat`: Source Harvester

Automatically crawls directories, files, or `.reg` files and produces WiX XML fragments:

```bash
# Harvest a directory tree with auto-generated GUIDs
heat dir ./build_output 
  -cg ProductComponents 
  -dr INSTALLFOLDER 
  -srd 
  -gg 
  -var var.SourceDir 
  -out Components.wxs

# Harvest a registry file (.reg)
heat reg settings.reg 
  -cg RegistryEntries 
  -dr INSTALLFOLDER 
  -out Registry.wxs
```

#### `torch`: Database Transform Generator (`.mst`)

Computes the binary and relational delta between two MSI databases to produce a transform:

```bash
torch -p -o Update.mst Baseline.msi Updated.msi
```

#### `pyro`: Patch Package Builder (`.msp`)

Assembles an update patch from a patch descriptor and transforms:

```bash
pyro Patch.wixobj -t SampleDiff Update.mst -out Update.msp
```

#### `lit`: WiX Library Builder (`.wixlib`)

Combines multiple `.wixobj` intermediate files into a shared library:

```bash
lit -bf -out SharedUI.wixlib Dialogs.wixobj Controls.wixobj
```

#### `smoke`: Package Validator (ICE Engine)

Validates databases against Internal Consistency Evaluators:

```bash
# Run all standard ICE validation rules
smoke MyApplication.msi

# Run specific rules and suppress others
smoke MyApplication.msi -ice:ICE01 -ice:ICE03 -sice:ICE18
```

---

### GNOME MSI Toolset Compatible Binaries

`msi-rs` provides full command and argument parity with the Linux `msitools` suite.

#### `msiinfo`: Metadata, Tables, and Stream Inspector

```bash
# 1. Print summary information properties
msiinfo MyApplication.msi

# 2. List all database tables
msiinfo tables MyApplication.msi

# 3. Display schema for a specific table
msiinfo schema MyApplication.msi File

# 4. List all binary streams and storages in the CFB container
msiinfo streams MyApplication.msi

# 5. Export a database table as tab-delimited IDT format
msiinfo export MyApplication.msi Property

# 6. Extract a raw binary stream to a local file
msiinfo extract MyApplication.msi "#cab1.cab" ./extracted_cab.cab
```

#### `msibuild`: Stream and IDT Manipulator

Low-level database manipulation:

```bash
# Add or replace a cabinet stream inside an MSI
msibuild MyApplication.msi -a "#cab1.cab" ./new_payload.cab

# Remove a stream from the MSI CFB container
msibuild MyApplication.msi -s "#old_stream"
```

#### `msidump`: Package Archive Dumper

Dumps all relational tables (`.idt` files) and binary streams to a directory:

```bash
msidump -d ./dump_output MyApplication.msi
```

The `./dump_output` directory will contain:
- Standard `.idt` table dumps (`Property.idt`, `File.idt`, `Directory.idt`, etc.).
- Binary stream dumps (`_StringPool`, `_StringData`, `#cab1.cab`, etc.).

#### `msidiff`: Relational and Stream Diffing

Compares two MSI databases and outputs differences in schemas, rows, streams, and summary properties:

```bash
msidiff App_v1.msi App_v2.msi
```

#### `msiextract`: Cabinet Payload Extractor

Extracts payload files from embedded cabinets:

```bash
# List all contained files without extracting
msiextract -l MyApplication.msi

# Extract all files into a target directory
msiextract -C ./extracted_payload MyApplication.msi

# Extract files belonging only to a specific Component
msiextract -C ./extracted_payload --component MainExecutable MyApplication.msi

# Extract files belonging only to a specific Feature
msiextract -C ./extracted_payload --feature CoreFeature MyApplication.msi
```

---

### Graphical (GUI) and Terminal (TUI) Installers

#### `msi-gui`: Interactive Desktop Wizard

Renders a desktop UI wizard matching WiX dialog layouts (`WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`):

```bash
# Launch interactive Mondo theme
msi-gui ./MyApplication.msi --theme mondo

# Launch Feature Tree theme with custom window size
msi-gui ./MyApplication.msi --theme feature-tree --width 640 --height 480

# Launch with pure software rasterizer (for systems without GPU acceleration)
msi-gui ./MyApplication.msi --backend softbuffer
```

Available CLI options:
- `--theme <mondo|installdir|feature-tree>`: Dialog layout flow.
- `--backend <wgpu|glow|softbuffer>`: Graphics hardware acceleration.
- `--width <u32>` / `--height <u32>`: Initial window dimensions.

#### Terminal TUI Wizard (`msi-cli --tui`)

For remote servers or headless CI environments, `msi-rs` includes an interactive curses terminal installer displaying ASCII/Unicode wizard dialogs, real-time action progress, and keyboard navigation:

```bash
# Launch interactive terminal TUI wizard
msi-cli install ./MyApplication.msi --tui

# Runs automatically if no display server is present (headless / SSH)
msi-cli install ./MyApplication.msi
```

---

## 3. Rust Library Usage Guide (`msi`)

The `msi` crate gives you pure-Rust primitives for reading, writing, building, and executing Windows Installer databases without foreign libraries.

### Adding `msi` to `Cargo.toml`

```toml
[dependencies]
msi = { version = "0.0.1", path = "../msi-rs/crates/msi" }
```

---

### Creating Packages with `PackageBuilder`

The following complete Rust example demonstrates creating an MSI package from scratch, configuring metadata, adding directory hierarchies, components, features, files, media records, and packing an embedded MSZIP cabinet:

```rust
use msi::cab::{CabinetWriter, CompressionType};
use msi::database::tables::core::{
    ComponentRow, DirectoryRow, FeatureRow, FileRow, MediaRow,
};
use msi::database::tables::record::Record;
use msi::database::tables::types::{
    ComponentGuid, ComponentId, DirectoryId, DiskId, FeatureId, FileId, FileName,
};
use msi::{Package, ProductVersion, Result};
use std::path::Path;

fn main() -> Result<()> {
    let output_path = Path::new("MyApplication.msi");

    // 1. Pack application payload into an MSZIP cabinet archive
    let mut cab_writer = CabinetWriter::new(CompressionType::MsZip);
    let app_binary_content = b"echo 'Hello from Rust MSI!'";
    cab_writer.add_file("app.sh", app_binary_content)?;
    let cabinet_bytes = cab_writer.build();

    // 2. Initialize PackageBuilder with Product metadata
    let version = ProductVersion::new(1, 0, 0);
    let mut builder = Package::builder()
        .product_name("My Custom Rust App")
        .manufacturer("Rust Builders Inc.")
        .version(version)
        .product_code("{A1B2C3D4-E5F6-7890-1234-567890ABCDEF}")
        .upgrade_code("{F1E2D3C4-B5A6-0987-6543-210FEDCBA987}")
        .add_property("INSTALLLEVEL", "1")
        .add_property("ARPCOMMENTS", "Built with pure Rust msi-rs");

    // 3. Define Directory layout
    // TARGETDIR -> ProgramFilesFolder -> INSTALLDIR
    builder = builder
        .add_directory(DirectoryRow {
            directory: DirectoryId::from("TARGETDIR"),
            directory_parent: None,
            default_dir: "SourceDir".to_string(),
        })
        .add_directory(DirectoryRow {
            directory: DirectoryId::from("ProgramFilesFolder"),
            directory_parent: Some(DirectoryId::from("TARGETDIR")),
            default_dir: "PFiles|Program Files".to_string(),
        })
        .add_directory(DirectoryRow {
            directory: DirectoryId::from("INSTALLDIR"),
            directory_parent: Some(DirectoryId::from("ProgramFilesFolder")),
            default_dir: "AppDir|MyCustomApp".to_string(),
        });

    // 4. Define Component
    builder = builder.add_component(ComponentRow {
        component: ComponentId::from("MainComponent"),
        component_id: Some(ComponentGuid::from("{11111111-2222-3333-4444-555555555555}")),
        directory: DirectoryId::from("INSTALLDIR"),
        attributes: 0,
        condition: None,
        key_path: Some("app_file".to_string()),
    });

    // 5. Define Feature
    builder = builder.add_feature(FeatureRow {
        feature: FeatureId::from("MainFeature"),
        feature_parent: None,
        title: Some("Core Executable".to_string()),
        description: Some("Installs application binaries".to_string()),
        display: Some(1),
        level: 1,
        directory: Some(DirectoryId::from("INSTALLDIR")),
        attributes: 0,
    });

    // 6. Define File record
    builder = builder.add_file(FileRow {
        file: FileId::from("app_file"),
        component: ComponentId::from("MainComponent"),
        file_name: FileName::parse("app.sh").map_err(|e| msi::Error::InvalidArgument {
            argument: "file_name".to_string(),
            reason: e.to_string(),
        })?,
        file_size: app_binary_content.len() as u32,
        version: None,
        language: None,
        attributes: Some(512), // msidbFileAttributesChecksum
        sequence: 1,
    });

    // 7. Define Media table entry pointing to embedded cabinet
    builder = builder.add_media(MediaRow {
        disk_id: DiskId::from(1),
        last_sequence: 1,
        disk_prompt: None,
        cabinet: Some("#cab1.cab".to_string()),
        volume_label: None,
        source: None,
    });

    // 8. Attach embedded cabinet stream
    builder = builder.add_embedded_cabinet("#cab1.cab", cabinet_bytes);

    // 9. Build and serialize package to disk
    let package = builder.build()?;
    package.save(output_path)?;

    println!("Successfully built '{}'", output_path.display());
    Ok(())
}
```

---

### Reading, Inspecting, and Modifying Packages

Inspect existing `.msi` files, read metadata, iterate tables, or extract embedded cabinets:

```rust
use msi::cab::CabinetReader;
use msi::database::tables::record::FieldValue;
use msi::{Package, Result};
use std::fs;
use std::path::Path;

fn inspect_package(path: &Path) -> Result<()> {
    // 1. Open package from filesystem
    let package = Package::open(path)?;

    // 2. Query high-level metadata
    let metadata = package.metadata();
    println!("Product Name: {}", metadata.product_name());
    println!("Version:      {}", metadata.version());
    println!("Manufacturer: {}", metadata.manufacturer());
    println!("ProductCode:  {}", metadata.product_code());

    // 3. Inspect Summary Information stream properties
    let summary = package.summary_info();
    if let Some(ref title) = summary.title {
        println!("Title:        {title}");
    }
    if let Some(page_count) = summary.page_count {
        println!("Minimum MSI:  v{page_count}");
    }

    // 4. Query Relational Database tables
    let db = package.database();
    let file_records = db.get_records("File");
    println!("Total files in package: {}", file_records.len());

    for rec in file_records {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::Long(size))) =
            (rec.get(0), rec.get(3))
        {
            println!("  File: {file_id:<20} Size: {size} bytes");
        }
    }

    // 5. Extract Embedded Cabinet archives
    for (cab_name, cab_data) in package.embedded_cabinets() {
        println!("Found embedded cabinet '{cab_name}' ({} bytes)", cab_data.len());
        let reader = CabinetReader::new(cab_data)?;

        for file in reader.files() {
            println!("  Extracting '{}' ({} bytes)", file.filename(), file.uncompressed_size());
            let extracted_bytes = reader.extract_file(file.filename())?;
            fs::write(file.filename(), extracted_bytes)?;
        }
    }

    Ok(())
}
```

---

### Executing Windows Installer SQL Dialect

`msi-rs` contains a native pure-Rust SQL executor supporting official Windows Installer SQL queries against [`LinkedDatabase`]:

```rust
use msi::database::sql::{execute_sql, QueryResult};
use msi::database::tables::record::FieldValue;
use msi::{Package, Result};

fn run_database_queries(package: &mut Package) -> Result<()> {
    let db = package.database_mut();

    // 1. Execute a SELECT query
    let select_sql = "SELECT `Property`, `Value` FROM `Property` WHERE `Property` = 'ProductName'";
    let result = execute_sql(db, select_sql, &[])?;

    if let QueryResult::Select(rows) = result {
        for row in rows {
            println!("Property: {:?} = {:?}", row.get(0), row.get(1));
        }
    }

    // 2. Execute an INSERT query with parameterized bindings
    let insert_sql = "INSERT INTO `Property` (`Property`, `Value`) VALUES (?, ?)";
    let params = vec![
        FieldValue::String("CustomBuildEnvironment".to_string()),
        FieldValue::String("Linux-CI".to_string()),
    ];
    execute_sql(db, insert_sql, &params)?;

    // 3. Execute an UPDATE query
    let update_sql = "UPDATE `Property` SET `Value` = 'Production' WHERE `Property` = 'CustomBuildEnvironment'";
    execute_sql(db, update_sql, &[])?;

    Ok(())
}
```

---

### Cabinet Archive Compression and Extraction (`msi::cab`)

Compress files into standalone Microsoft Cabinet archives or decompress existing archives using MSZIP, LZX, or Quantum compression:

```rust
use msi::cab::{CabinetReader, CabinetWriter, CompressionType};
use msi::Result;
use std::fs;

fn cabinet_roundtrip_example() -> Result<()> {
    // 1. Compress files into a CAB archive using MSZIP
    let mut writer = CabinetWriter::new(CompressionType::MsZip);
    writer.add_file("readme.txt", b"Cabinet archive generated by msi-rs.")?;
    writer.add_file("data.json", b"{\"name\": \"msi-rs\", \"pure_rust\": true}")?;

    let cab_bytes = writer.build();
    fs::write("archive.cab", &cab_bytes)?;

    // 2. Read and decompress files from the CAB archive
    let reader = CabinetReader::new(&cab_bytes)?;
    for file_entry in reader.files() {
        let name = file_entry.filename();
        let payload = reader.extract_file(name)?;
        println!("Decompressed '{}' ({} bytes)", name, payload.len());
    }

    Ok(())
}
```

---

### Compiling WiX XML Programmatically

You can invoke the entire WiX preprocessor, compiler, and linker pipeline directly from Rust:

```rust
use msi::wix::{compile_wix, Linker, PreprocessorContext};
use msi::{Package, Result};

fn compile_wxs_to_msi() -> Result<Package> {
    let wxs_source = r#"
    <Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
        <Product Id="{12345678-1234-1234-1234-1234567890AB}"
                 Name="$(var.ProductName)"
                 Version="$(var.ProductVersion)"
                 Manufacturer="Acme Corp">
            <Package Description="Testing programmatic WiX compiler" />
            <Directory Id="TARGETDIR" Name="SourceDir">
                <Directory Id="ProgramFilesFolder" Name="PFiles">
                    <Directory Id="INSTALLDIR" Name="MyApp">
                        <Component Id="C1" Guid="{22222222-2222-2222-2222-222222222222}">
                            <File Id="F1" Source="test.txt" />
                        </Component>
                    </Directory>
                </Directory>
            </Directory>
            <Feature Id="Main" Title="Main Feature" Level="1">
                <ComponentRef Id="C1" />
            </Feature>
        </Product>
    </Wix>
    "#;

    // 1. Setup preprocessor variables
    let mut ctx = PreprocessorContext::new();
    ctx.define_var("ProductName", "Compiled App");
    ctx.define_var("ProductVersion", "1.2.3");

    // 2. Compile into intermediate WixObject
    let wix_object = compile_wix(wxs_source, &mut ctx)?;

    // 3. Link into LinkedDatabase
    let mut linker = Linker::new();
    linker.add_object(wix_object);
    let linked_db = linker.link()?;

    // 4. Wrap into final Package
    let embedded_cabs = linker.take_embedded_cabinets();
    let package = Package::from_database(linked_db, embedded_cabs);

    Ok(package)
}
```

---

### Two-Phase Transactional Execution Runtime

`msi-rs` replaces `msiexec.exe` with a memory-safe, typestate-enforced two-phase transaction execution engine:

```rust
use msi::execution::{DiskCostEngine, EvaluationContext, Transaction, WorkerContext};
use msi::{Package, Result};

fn execute_installer_transaction(package: &Package) -> Result<()> {
    // 1. Setup execution context and evaluate properties
    let mut context = EvaluationContext::new();
    context.set_property("INSTALLLEVEL", "1");
    context.set_property("UILevel", "None");

    // 2. Cost disk space
    let cost_engine = DiskCostEngine::new();

    // 3. Phase 1: Immediate Execution (builds install and rollback scripts)
    let transaction = Transaction::new(package.database().clone(), context, cost_engine);
    let prepared_tx = transaction.prepare()?;

    // 4. Phase 2: Deferred Privileged Execution
    let mut worker = WorkerContext::new();
    let executed_tx = match prepared_tx.execute(&mut worker) {
        Ok(exec) => exec,
        Err(err) => {
            eprintln!("Execution failed: {err}; rolling back changes...");
            // Rollback script executes atomically and restores original state
            return Err(err);
        }
    };

    // 5. Commit transaction and clean up physical rollback quarantine (.rbf)
    let _committed_tx = executed_tx.commit(&mut worker)?;
    println!("Installation committed successfully!");

    Ok(())
}
```

---

## 4. Python Library Usage Guide (`msi`)

The `msi` Python library enables automated Windows Installer creation, inspection, and compilation on Linux, macOS, and Windows.

### Quickstart: Directory-to-MSI One-Liner (`msi.build_msi`)

Pack an entire local directory tree into a fully compliant `.msi` package with automated MD5 hash generation, standard sequences, and MSZIP cabinet compression:

```python
import msi
from pathlib import Path

def on_progress(phase: str, current: int, total: int) -> None:
    percent = (current / total * 100) if total > 0 else 0
    print(f"[{phase}] {current}/{total} ({percent:.1f}%)")

msi.build_msi(
    source_dir="dist/my_application",
    output_path="bin/MyApplication.msi",
    product_name="My Python Application",
    version="1.0.0",
    manufacturer="Acme Software LLC",
    product_code="{11111111-2222-3333-4444-555555555555}",
    upgrade_code="{88888888-9999-0000-1111-222222222222}",
    install_dir_name="MyApplication",
    compression=msi.CompressionType.MSZIP,
    progress_callback=on_progress,
)
```

---

### Fluent Package Construction (`msi.PackageBuilder`)

For granular control over GUIDs, directory hierarchies, features, components, and embedded cabinets, use `msi.PackageBuilder`:

```python
import msi
from pathlib import Path

with msi.PackageBuilder(
    product_name="Custom Enterprise App",
    manufacturer="Enterprise Corp",
    version="2.1.0",
    product_code="{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}",
    upgrade_code="{FFFFFFFF-0000-1111-2222-333333333333}",
) as builder:
    # 1. Add Custom Properties
    builder.add_property("ARPHELPLINK", "https://help.example.com")
    builder.add_property("ALLUSERS", "1")

    # 2. Define Directory Hierarchy
    # TARGETDIR -> ProgramFilesFolder -> INSTALLDIR
    builder.add_directory("TARGETDIR", None, "SourceDir")
    builder.add_directory("ProgramFilesFolder", "TARGETDIR", "PFiles|Program Files")
    builder.add_directory("INSTALLDIR", "ProgramFilesFolder", "AppDir|CustomApp")

    # 3. Add Component
    builder.add_component(
        comp_id="MainBinaryComp",
        dir_id="INSTALLDIR",
        comp_guid="{12345678-ABCD-1234-ABCD-12345678ABCD}",
    )

    # 4. Add Features and link Components
    builder.add_feature(
        feat_id="Complete",
        title="Complete Application",
        description="All binaries and documentation",
        level=1,
    )
    builder.add_feature_component("Complete", "MainBinaryComp")

    # 5. Add Files directly from disk
    # This automatically computes file size, generates keypaths, and adds to CAB
    builder.add_file_from_disk(
        source_path="build/app.exe",
        target_dir_id="INSTALLDIR",
        feature_id="Complete",
        component_id="MainBinaryComp",
    )

    # 6. Build directly to file or in-memory bytes
    builder.build_to_file("CustomApp.msi")
    raw_msi_bytes = builder.build_to_bytes()

print(f"Generated MSI size: {len(raw_msi_bytes)} bytes")
```

---

### Inspecting Packages and Querying Tables (`msi.Package`)

Inspect tables and metadata from any `.msi` file:

```python
import msi

with msi.Package.open("CustomApp.msi") as pkg:
    # 1. Query Metadata
    print("Product Name:   ", pkg.get_property("ProductName"))
    print("Product Version:", pkg.get_property("ProductVersion"))
    print("Manufacturer:   ", pkg.get_property("Manufacturer"))

    # 2. Inspect All Properties
    props = pkg.properties
    for key, val in props.items():
        print(f"  {key} = {val}")

    # 3. List All Database Tables
    tables = pkg.table_names
    print("Tables present in MSI:", ", ".join(tables))

    # 4. Read Table Records as Python Dictionaries
    if "File" in tables:
        files = pkg.get_table("File")
        for f in files:
            print(f"File ID: {f.get('File')}, Name: {f.get('FileName')}, Size: {f.get('FileSize')}")

    # 5. Extract Embedded Cabinet Archives
    extracted = pkg.extract_cabinet("#cab1.cab", "./extracted_files")
    print(f"Extracted {len(extracted)} files to ./extracted_files")
```

---

### Compiling WiX XML from Python (`msi.compile_wix`)

Compile WiX v3/v4 source code directly without installing WiX:

```python
import msi

wxs_content = """
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}"
             Name="$(var.AppName)"
             Version="1.0.0"
             Manufacturer="Acme Corp">
        <Package Description="WiX compiled from Python" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="C1">
                <File Id="F1" Source="main.py" />
            </Component>
        </Directory>
        <Feature Id="Main" Title="Main" Level="1">
            <ComponentRef Id="C1" />
        </Feature>
    </Product>
</Wix>
"""

# Compile from string with variable substitutions
msi.compile_wix(
    source=wxs_content,
    output_path="WixApp.msi",
    variables={"AppName": "My WiX App from Python"},
)

# Or compile from an existing file on disk
msi.compile_wix(source="src/Product.wxs", output_path="WixApp.msi")
```

---

### Package Inspection Dictionary (`msi.inspect_msi`)

`msi.inspect_msi` returns a dictionary representation of an entire package:

```python
import msi
import json

info = msi.inspect_msi("CustomApp.msi")

print("Metadata:")
print(json.dumps(info["metadata"], indent=2))

print("Tables in Package:")
print(info["table_names"])

print(f"Total Tables: {len(info['tables'])}")
```

---

### Identifier Sanitization and ProductVersion Utilities

Windows Installer identifiers must conform to strict alphanumeric naming constraints. The `msi` module provides utilities for version parsing and identifier sanitation:

```python
import msi

# 1. Sanitize string into compliant MSI identifier
raw_name = "my-awesome_file.v1.0!@#.txt"
safe_id = msi.sanitize_identifier(raw_name)
print(f"Sanitized ID: {safe_id}")  # e.g., 'my_awesome_file.v1.0____txt'

# 2. ProductVersion comparisons
v1 = msi.ProductVersion(1, 2, 0)
v2 = msi.ProductVersion.parse("1.2.5")

print(v1 < v2)      # True
print(v2.as_tuple()) # (1, 2, 5)
print(str(v2))      # "1.2.5"
```

---

### Error Handling

The Python package raises strongly-typed exceptions inheriting from `msi.MsiError`:

```python
import msi

try:
    pkg = msi.Package.open("nonexistent.msi")
except msi.IoError as e:
    print(f"I/O error occurred: {e}")
except msi.ValidationError as e:
    print(f"Package validation failed: {e}")
except msi.DatabaseError as e:
    print(f"Database table corruption: {e}")
except msi.MsiError as e:
    print(f"General MSI error: {e}")
```

---

## 5. Production CI/CD Recipes & Workflows

### Recipe 1: GitHub Actions Linux CI Building Windows MSI

Build your Windows installer natively on `ubuntu-latest` without Windows runners:

```yaml
name: Build Windows MSI on Linux

on:
  push:
    tags:
      - 'v*'

jobs:
  build-installer:
    runs-on: ubuntu-latest

    steps:
      - name: Check out repository
        uses: actions/checkout@v4

      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build Application Binaries
        run: |
          cargo build --release --target x86_64-pc-windows-msvc

      - name: Install msi-rs CLI tools
        run: |
          cargo install --git https://github.com/SamuelMarks/msi-rs.git msi-cli

      - name: Harvest Assets & Build MSI
        run: |
          # 1. Harvest application release directory
          msi harvest ./dist 
            --mode dir 
            --group AppFiles 
            --dir-id INSTALLFOLDER 
            --output ./AppFiles.wxs

          # 2. Compile WiX objects
          candle -arch x64 -dVersion=${{ github.ref_name }} -out App.wixobj Product.wxs
          candle -arch x64 -out AppFiles.wixobj AppFiles.wxs

          # 3. Link into final MSI
          light -b ./dist -out MyApplication.msi App.wixobj AppFiles.wixobj

      - name: Validate MSI with ICE rules
        run: |
          smoke MyApplication.msi

      - name: Upload Installer Artifact
        uses: actions/upload-artifact@v4
        with:
          name: MyApplication-Setup
          path: MyApplication.msi
```

---

### Recipe 2: Automated Python Build Script for Release Artifacts

```python
#!/usr/bin/env python3
"""
Packaging script: builds Python application and packages into an MSI installer.
"""

import sys
import shutil
import subprocess
from pathlib import Path
import msi

def main() -> None:
    version = "1.5.0"
    dist_dir = Path("build_dist")
    out_msi = Path(f"dist/EnterpriseApp-{version}-x64.msi")

    # Clean and prepare directory
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    dist_dir.mkdir(parents=True)

    # Populate dist directory with binaries and assets
    (dist_dir / "app.exe").write_bytes(b"Simulated application binary")
    (dist_dir / "config.json").write_text('{"env": "production"}')

    print(f"Building {out_msi}...")
    msi.build_msi(
        source_dir=dist_dir,
        output_path=out_msi,
        product_name="Enterprise Application",
        version=version,
        manufacturer="Enterprise Global Solutions",
        product_code="{12345678-ABCD-EF01-2345-6789ABCDEF01}",
        upgrade_code="{87654321-DCBA-10FE-5432-10FEDCBA9876}",
        compression=msi.CompressionType.MSZIP,
        progress_callback=lambda phase, cur, tot: print(f"[{phase}] {cur}/{tot}"),
    )

    # Verify package integrity
    pkg_info = msi.inspect_msi(out_msi)
    assert pkg_info["metadata"]["ProductName"] == "Enterprise Application"
    print(f"Successfully generated {out_msi} ({out_msi.stat().st_size} bytes)")

if __name__ == "__main__":
    main()
```

---

### Recipe 3: Headless Silent Deployment in Docker / Kubernetes

Deploy and extract MSI payloads in container environments:

```dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Install pre-built msi-rs binaries
COPY --from=builder /usr/local/bin/msi /usr/local/bin/msi
COPY --from=builder /usr/local/bin/msiextract /usr/local/bin/msiextract

WORKDIR /opt/deployment
COPY SetupPackage.msi .

# Method A: Extract file payloads directly without transaction execution
RUN msiextract -C /opt/myapp SetupPackage.msi

# Method B: Execute installation transaction silently with logging
RUN msi /i SetupPackage.msi /qn /l*vx /var/log/msi_install.log TARGETDIR=/opt/myapp

ENTRYPOINT ["/opt/myapp/app.exe"]
```
