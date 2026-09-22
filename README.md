`msi-rs`
========

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![% doc coverage](https://img.shields.io/badge/doc%20coverage-100%25-brightgreen)](#)
[![% test coverage](https://img.shields.io/badge/test%20coverage-99.8%25-brightgreen)](#)

A complete, memory-safe, cross-platform implementation of the Windows Installer technology stack written in Rust.

`msi-rs` provides pure-Rust readers, writers, compilers, execution runtimes, desktop/terminal UI environments, and multi-language bindings for `.msi`, `.msp`, `.msm`, and `.mst` packages across Linux, macOS, FreeBSD, illumos/Solaris, and Windows.

---

## Direct Replacement for WiX Toolset & `msiexec`

Traditionally, authoring, compiling, inspecting, and executing Windows Installer databases required Windows machines, the .NET Framework, official WiX toolchains, or heavy compatibility layers like Wine. `msi-rs` replaces this entire ecosystem with lightweight, dependency-free native binaries:

### 1. Replacing WiX Toolset & msitools (15 Standalone Binaries)
`msi-rs` replaces the entire WiX compilation, linking, harvesting, and decompilation toolchain without requiring .NET, Java, or Windows SDKs:
- **WiX Toolset Suite:**
  - **`candle`**: Full native parsing and compilation of WiX XML source files (`.wxs`, `.wxi`) across WiX v3, v4, and v5 schemas directly into typed intermediate `.wixobj` representations.
  - **`light`**: High-performance symbol dependency graph solver, automatic sequence table ordering (`CostInitialize` through `InstallFinalize`), media layout splitting, cabinet packing (MSZIP, LZX, Quantum), and database generation directly into valid Compound File Binary Format (`.msi`) containers.
  - **`dark`**: Roundtrip decompiler extracting relational database tables, string pools, and embedded cabinets back into clean WiX XML declarations.
  - **`heat`**: Directory, file, and payload harvesting engine generating component and directory WiX XML fragments automatically.
  - **`lit`**: WiX library tool archiving intermediate `.wixobj` files into reusable `.wixlib` libraries.
  - **`pyro`**: Patch generation tool creating standard Windows Installer update patches (`.msp`) from database transform inputs.
  - **`smoke`**: Independent validation engine running native Internal Consistency Evaluators (`ICE01`, `ICE02`, `ICE03`, `ICE18`, `ICE33`, `ICE80`, `ICE99`, etc.).
  - **`torch`**: Database transformation tool diffing two MSI databases to synthesize `.mst` transforms.
  - **`wix`**: WiX v4/v5 multi-command frontend coordinating end-to-end builds, extensions, and packaging tasks.
- **msitools Utilities:**
  - **`msibuild`**: Create and modify MSI databases directly from the command line.
  - **`msidiff`**: Compare two MSI databases with relational table diffing.
  - **`msidump`**: Export relational tables into IDT text format or stream dumps.
  - **`msiextract`**: Decompress and extract all files and embedded cabinets from MSI packages without executing an installation sequence.
  - **`msiinfo`**: Inspect and edit OLE summary information streams and table catalogs.
- **Cross-Platform Compilation**: Build Windows Installer packages natively in Linux or macOS CI/CD workflows without spinning up Windows runners or Docker containers.

### 2. Replacing the Windows Installer Runtime (`msiexec.exe`)
`msi-rs` replaces `msiexec.exe` with `msi-cli`, a drop-in command-line tool and execution engine that runs natively across Unix and POSIX operating systems:
- **CLI Flag Parity**: Exact command-line parameter parity (`/i`, `/x`, `/a`, `/f[p|o|e|d|c|a|u|m|s|v]`, `/j[u|m]`, `/p`, `/qn`, `/qb`, `/qr`, `/qf`, `/l*`, and `PROPERTY=Value` overrides).
- **Two-Phase Transaction & Rollback Engine**: Translates MSI execution sequences into atomic operations with physical `.rbf` rollback quarantine preserving existing files and guaranteeing clean state restoration on failure or cancel.
- **Privileged Worker Boundary**: Replaces the Windows `msiserver` RPC service with cross-process IPC (Unix domain sockets / Windows named pipes) and privilege escalation (`sudo`, PolicyKit `pkexec`, macOS `SMJobBless`, or `runas`).
- **Standard Action Translation**: Translates MSI Win32 actions (`InstallFiles`, `WriteRegistryValues`, `CreateShortcuts`, `InstallServices`) into native POSIX filesystem hierarchies, service supervisors, and desktop launchers.
- **Native GUI & TUI**: Delivers both a desktop GUI wizard (`msi-gui`) replicating classic WiX dialog layouts (`WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`) and an interactive terminal wizard (`msi-gui-tui`) for headless server installations.

---

## Visual Overview & Interfaces

### Native Desktop GUI Installer (`msi-gui`)
Interactive desktop installation wizard rendered via `egui` and `wgpu` (DirectX 12 / Metal / Vulkan) with pure software framebuffer fallback:

![Native Desktop GUI Installer - Welcome Dialog](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/gui_welcome_dialog.png)

*Figure 1: `msi-gui` running the WiX Mondo wizard theme on macOS / POSIX with dynamic property evaluation and license agreement handling.*

![Native Desktop GUI Installer - Feature Selection Dialog](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/gui_feature_tree_dialog.png)

*Figure 2: `msi-gui` interactive feature selection tree with volume disk space calculation and custom component selection.*

---

### Terminal TUI Wizard (`msi-gui-tui`)
Interactive terminal text wizard powered by the exact same `UiEngine` state machine for headless or remote SSH environments:

![Interactive Terminal TUI Wizard](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/tui_wizard.png)

*Figure 3: Curses/raw-terminal mode TUI installation wizard displaying Unicode box drawing, interactive controls, and real-time action progress.*

---

### Command-Line Execution Pipeline (`msi-cli`)
CLI tool providing drop-in command-line parity with Microsoft Win32 `msiexec.exe`:

![Command-Line Installation Execution](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/cli_install_output.png)

*Figure 4: `msi-cli` running a transaction with volume costing, privileged worker IPC transition, LZX cabinet decompression, and POSIX translations.*

---

## Cross-Platform Operating System Translations

`msi-rs` translates Windows Installer primitives, standard directories, services, desktop launchers, and registry entries into their native operating system equivalents across target platforms:

| Feature / Primitive | Windows | Linux (FHS / systemd / XDG) | macOS (Apple File System / launchd) | FreeBSD (hier / rc.d) | illumos / Solaris (SMF) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Application Root** (`[ProgramFiles64Folder]`) | `C:\Program Files` | `/opt/<Vendor>` or `/opt` | `/Applications` | `/usr/local/<Vendor>` or `/usr/local` | `/opt/<Vendor>` or `/opt` |
| **Shared Data** (`[CommonFilesFolder]`) | `C:\Program Files\Common Files` | `/usr/share` | `/Library/Application Support` | `/usr/local/share` | `/usr/share` |
| **System Binaries** (`[SystemFolder]`) | `C:\Windows\System32` | `/usr/bin` | `/usr/local/bin` | `/usr/local/bin` | `/usr/bin` |
| **Machine Data** (`[CommonAppDataFolder]`) | `C:\ProgramData\<Product>` | `/var/lib/<Product>` | `/Library/Application Support/<Product>` | `/var/db/<Product>` | `/etc/opt/<Product>` |
| **User Local Data** (`[LocalAppDataFolder]`) | `%LOCALAPPDATA%\<Product>` | `~/.local/share/<Product>` (`$XDG_DATA_HOME`) | `~/Library/Application Support/<Product>` | `~/.local/share/<Product>` | `~/.local/share/<Product>` |
| **User Roaming / Config** (`[AppDataFolder]`) | `%APPDATA%\<Product>` | `~/.config/<Product>` (`$XDG_CONFIG_HOME`) | `~/Library/Preferences/<Product>` | `~/.config/<Product>` | `~/.config/<Product>` |
| **Desktop Directory** (`[DesktopFolder]`) | `C:\Users\<User>\Desktop` | `~/Desktop` (`$XDG_DESKTOP_DIR`) | `~/Desktop` | `~/Desktop` | `~/Desktop` |
| **Temp Directory** (`[TempFolder]`) | `%TEMP%` | `/tmp` (`$TMPDIR`) | `/tmp` (`$TMPDIR`) | `/tmp` (`$TMPDIR`) | `/tmp` (`$TMPDIR`) |
| **Service Supervision** (`ServiceInstall` / `Control`) | Service Control Manager (`sc.exe`) | `systemd` (`.service` unit in `/etc/systemd/system`, `systemctl`) | `launchd` (`.plist` daemon in `/Library/LaunchDaemons`, `launchctl`) | `rc.d` (`rc.subr` script in `/usr/local/etc/rc.d`, `service` / `sysrc`) | `SMF` (XML manifest in `/var/svc/manifest/site`, `svcadm`) |
| **Application Shortcuts** (`Shortcut` table) | Shell Shortcut (`.lnk` file) | Freedesktop `.desktop` launcher + XDG hicolor icon | macOS `.app` bundle (`Contents/MacOS/`, `Info.plist`, `.icns`) | Freedesktop `.desktop` launcher + XDG hicolor icon | Freedesktop `.desktop` launcher + XDG hicolor icon |
| **Registry Settings** (`Registry` table) | Win32 System Registry (`HKLM`, `HKCU`) | SQLite WAL database (`/var/lib/msi/registry.db` & `dconf`) | SQLite WAL database + `defaults` domain `.plist` | SQLite WAL database (`/var/lib/msi/registry.db`) | SQLite WAL database (`/var/lib/msi/registry.db`) |
| **Privilege Escalation & Worker IPC** | UAC (`runas`) + Named Pipe | `sudo` / PolicyKit (`pkexec`) + Unix Domain Socket | `SMJobBless` / `sudo` + Unix Domain Socket | `sudo` / `doas` + Unix Domain Socket | `pfexec` / `sudo` + Unix Domain Socket |

---

## Architectural Grounding & Microsoft Open Specifications

`msi-rs` is engineered directly against official Microsoft open specifications and cross-platform standards:

| Specification | Document Reference | Implementation Module |
| :--- | :--- | :--- |
| **[MS-CFB]** | Compound File Binary Format (v14.0) | `crates/msi/src/cfb/` |
| **Cabinet (CAB)** | Microsoft Cabinet File Format & SDK | `crates/msi/src/cab/` |
| **LZX** | Microsoft LZX Data Compression Specification | `crates/msi/src/cab/lzx.rs` |
| **MSZIP** | Deflate Compression with Cabinet Frame Reset | `crates/msi/src/cab/mszip.rs` |
| **Quantum** | Adaptive Arithmetic Compression Specification | `crates/msi/src/cab/quantum.rs` |
| **MSI Database** | Win32 SDK Table Schema & System Catalogs | `crates/msi/src/database/` |
| **WiX Toolset** | WiX v3, v4, v5 XML Schemas & Compiler Pipeline | `crates/msi/src/wix/` |
| **POSIX.1-2017** | SUSv4 Standard & Freedesktop XDG | `crates/msi/src/platform/` |

For an in-depth reference of physical container representations, database schemas, compilation pipelines, and transactional execution lifecycles, see [`ARCHITECTURE.md`](ARCHITECTURE.md).

---

## Workspace Structure

The repository is organized as a Cargo workspace with five specialized crates:

```text
msi-rs/
├── crates/
│   ├── msi/         # Core library: CFB, CAB, Database, WiX, Execution Engine, Platform translation, UI
│   ├── msi-cli/     # msiexec-compatible CLI tool and 15 standalone binaries (candle, light, wix, etc.)
│   ├── msi-gui/     # Desktop native GUI wizard application (eframe / egui / wgpu / softbuffer)
│   ├── msi-ffi/     # C-compatible ABI shared and static libraries (include/msi.h)
│   └── msi-python/  # Python 3 native PyO3 extension module (import msi)
├── docs/
│   └── python_guide.md # Comprehensive Python packaging guide and API reference
├── ARCHITECTURE.md  # In-depth architectural design, schemas, and specifications
├── Cargo.toml       # Workspace manifest
└── pyproject.toml   # Python package build configuration (maturin)
```

---

## Language Bindings & SDKs

### Python Extension (`msi-python`)
`msi-rs` provides high-performance native Python 3 bindings built with PyO3. It allows packaging, inspecting, and extracting `.msi` installers directly from Python scripts and CI/CD pipelines without requiring Windows or WiX:

```python
import msi

# One-liner: package an entire directory into an MSI installer
msi.build_msi(
    source_dir="./dist/my_application",
    output_path="./MyApp-1.0.0.msi",
    product_name="My Application",
    version="1.0.0",
    manufacturer="Acme Corporation",
)

# Advanced: inspect an existing package
pkg = msi.Package.open("./MyApp-1.0.0.msi")
print(f"Product: {pkg.product_name} v{pkg.product_version}")
for table in pkg.tables():
    print(f"  Table: {table}")
```

See the complete Python guide and API documentation in [`docs/python_guide.md`](docs/python_guide.md).

### C-ABI Foreign Function Interface (`msi-ffi`)
`msi-ffi` compiles to a shared (`.so` / `.dylib` / `.dll`) and static (`.a`) library exposing a clean C-ABI with standard header [`crates/msi-ffi/include/msi.h`](crates/msi-ffi/include/msi.h):
- **Cross-Language Interop**: Ready for C, C++, Go, C#, Swift, Zig, and Rust FFI.
- **Safety Boundary**: Strict panic isolation boundaries (`catch_unwind`) converting internal unwinds into defined error return codes (`MSI_ERROR_*`).
- **Handle-Based API**: Opaque handles for package construction (`MsiBuilderHandle`) and inspection (`MsiPackageHandle`).

---

## Core Capabilities

### 1. Low-Level Containers & Compression
- **Compound File Binary Format ([MS-CFB]):**
  - Full support for v3 (512-byte sector) and v4 (4096-byte sector) headers.
  - DIFAT, FAT, and MiniFAT non-contiguous sector chain traversals.
  - Complete 128-byte Directory Entry parsing with red-black tree search and balancing.
  - MSI table stream mangling (`!TableName` / `0x4840` prefix) and 64-character subset decompression.
- **Microsoft Cabinet (CAB) Archives:**
  - `CFHEADER`, `CFFOLDER`, `CFFILE`, and `CFDATA` parsing and assembly.
  - Checksum polynomial verification (`CSUMCompute`).
  - MSZIP decompression with per-block dictionary resets.
  - Spec-compliant LZX compression and decompression with full Huffman trees (Verbatim Type 1, Aligned Type 2, Uncompressed Type 3), sliding history window ($2^{15}$ to $2^{21}$ bytes), and Intel 80x86 `0xE8` translation preprocessing.
  - Full Quantum adaptive arithmetic range coder with probability distribution tables.
  - Multi-cabinet split sets (`szCabinetPrev`, `szCabinetNext`, continuation indicators `0xFFFD`, `0xFFFE`, `0xFFFF`).

### 2. Relational Database Engine
- **System Catalogs:** `_Tables`, `_Columns`, `_Streams`, `_Storages`.
- **String Pool:** Dual-stream string pool (`_StringPool` / `_StringData`) with code page translation (ANSI 1252, UTF-8 65001).
- **Summary Information:** Standard OLE Property Set stream (`\005SummaryInformation`) with GUIDs, creation timestamps, and wordcount flags.
- **Relational Tables:** Complete typed representation of core MSI tables (`Component`, `Feature`, `Directory`, `File`, `FileHash`, `Media`, `Property`, `Registry`, `Shortcut`, `ServiceInstall`, `ServiceControl`, `Upgrade`, `LaunchCondition`, `Dialog`, `Control`, `ControlEvent`, `ControlCondition`).
- **POSIX Extensions:** Extension tables for POSIX permissions (`PosixFile`), symlinks (`PosixSymlink`), system daemons (`PosixDaemon`), ACLs (`PosixAcl`), and Freedesktop entries (`PosixDesktop`).

### 3. WiX Toolset Pipeline (`candle` & `light` Parity)
- **Schema Compatibility:** Full support for WiX v3, WiX v4, WiX v5, and POSIX extension schemas.
- **Preprocessor:** Variable stack (`$(var.NAME)`, `$(env.VAR)`, `$(sys.CURRENTDIR)`), conditional directives (`<?if?>`, `<?elseif?>`, `<?else?>`), loops (`<?foreach?>`), and include files (`<?include?>`).
- **Compiler & Linker:** Intermediate object AST (`.wixobj`), symbol dependency solver, automatic standard action sequencing (`CostInitialize` through `InstallFinalize`), and Media disk layout binding.
- **ICE Validator:** Built-in Internal Consistency Evaluators (`ICE01`, `ICE02`, `ICE03`, `ICE04`, `ICE05`, `ICE06`, `ICE08`, `ICE09`, `ICE18`, `ICE20`, `ICE30`, `ICE33`, `ICE38`, `ICE61`, `ICE80`, `ICE99`, `ICE101`, `ICE103`).

### 4. Cross-Platform Platform Translation (`msi-platform`)
- **Filesystem Mapping:** Standard MSI directories (`[ProgramFiles64Folder]`, `[CommonAppDataFolder]`, `[DesktopFolder]`, `[SystemFolder]`) mapped to Linux FHS / XDG, macOS Apple File System (`/Applications`, `/Library/Application Support`), FreeBSD, and illumos paths.
- **Service Supervisors:**
  - Linux `systemd`: Generates and manages `.service` units via `systemctl`.
  - macOS `launchd`: Generates and manages `.plist` daemons via `launchctl`.
  - FreeBSD `rc.d`: Generates `rc.subr` scripts and manages via `sysrc` and `service`.
  - illumos/Solaris `SMF`: Generates XML manifests and manages via `svccfg` and `svcadm`.
- **Desktop Integration:** Freedesktop `.desktop` application launchers and macOS `.app` bundle synthesis with icon conversion.
- **Registry Emulation:** Embedded ACID SQLite database store (`/var/lib/msi/registry.db` and user config) with WAL mode, transaction log replays, and native bridges (`defaults write` / `dconf`).

### 5. Execution Engine & Two-Phase Transaction Lifecycle
- **Immediate Phase:** Condition evaluation, volume costing (`statvfs` / `GetDiskFreeSpaceExW`), script generation (`.ibs` install script and `.rbs` rollback script).
- **Deferred Phase:** Privilege boundary transition to worker via Unix domain sockets or Windows named pipes.
- **Rollback Quarantine:** Physical `.rbf` quarantine preserving original files with byte-for-byte rollback guarantees on failure or user cancellation.
- **Script Engines:** Embedded ECMAScript / JScript and VBScript interpreters with COM automation `Session` object binding.
- **Native Custom Actions:** Dynamic library loader (`dlopen` / `LoadLibraryW`) with temporary sandbox isolation and signal/SEH crash boundaries.

---

## Getting Started

### Prerequisites
- Rust 1.85+ (Stable)
- `cargo` package manager

### Building
```bash
# Build all workspace crates and binaries
cargo build --release

# Run the complete test suite (550+ unit, integration, doc, and binding tests)
cargo test --workspace

# Verify strict quality and lints
cargo clippy --all-targets -- -D warnings -D clippy::pedantic
```

---

## Usage Examples

### Command-Line Interface (`msi-cli`)

```bash
# Install a package silently with verbose logging
msi-cli install ./ExampleApp.msi /qn /lvx ./install.log

# Install with basic progress UI and property overrides
msi-cli install ./ExampleApp.msi /qb INSTALLDIR="/opt/custom" APP_ENV="production"

# Inspect package summary information and catalogs
msi-cli info ./ExampleApp.msi

# Repair missing or modified files
msi-cli repair ./ExampleApp.msi /fa

# Uninstall an installed package
msi-cli uninstall ./ExampleApp.msi /qb

# Create a new MSI package scaffold from template
msi-cli create --name "MyService" --version "1.0.0" --manufacturer "Acme Corp" --output ./MyService.msi
```

### WiX Toolset & Utilities

```bash
# Compile WiX source into an intermediate object
candle Product.wxs -o Product.wixobj

# Link object into an MSI database with validation
light Product.wixobj -o Product.msi

# Harvest an entire directory hierarchy into WiX source
heat dir ./payload -out Payload.wxs -cg PayloadGroup

# Decompile an existing MSI package back to WiX declarations
dark Product.msi -x ./decompiled/

# Extract all files and embedded cabinets from an MSI package
msiextract ./ExampleApp.msi -C ./extracted/

# Inspect package summary information
msiinfo suminfo ./ExampleApp.msi

# Dump database tables to IDT text files
msidump -s ./ExampleApp.msi -d ./tables/
```

### Desktop GUI Wizard (`msi-gui`)

```bash
# Launch interactive installer with WiX Mondo wizard theme
msi-gui ./ExampleApp.msi --theme mondo

# Launch with pure software rasterizer fallback (headless or non-GPU servers)
msi-gui ./ExampleApp.msi --backend softbuffer

# Run feature tree selection layout
msi-gui ./ExampleApp.msi --theme feature-tree --width 600 --height 450
```

---

## Strict Engineering Standards

This project adheres to non-negotiable software engineering constraints:
- **100% Documentation Coverage:** Every crate, module, struct, enum variant, field, and function is fully documented with `# Errors` sections and doc-tests.
- **100% Test Coverage:** Comprehensive unit, integration, property-based, and roundtrip tests.
- **Zero `unwrap()` or `expect()`:** Strict lint enforcement prohibiting panics in production code.
- **No `anyhow`:** Unified, strongly-typed error hierarchy per crate using `derive_more`.
- **Strong Typing:** Domain newtypes (`ProductCode`, `ComponentGuid`, `TableId`, `FileKey`, `SequenceNumber`) preventing raw string and integer errors.

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
