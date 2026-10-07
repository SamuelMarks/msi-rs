`msi-rs`
========

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![% doc coverage](https://img.shields.io/badge/doc%20coverage-100%25-brightgreen)](#)
[![% test coverage](https://img.shields.io/badge/test%20coverage-98.7%25-brightgreen)](#)

A complete, memory-safe, cross-platform implementation of the Windows Installer technology stack written in Rust.

`msi-rs` provides pure-Rust readers, writers, compilers, execution runtimes, desktop/terminal UI environments, and multi-language bindings for `.msi`, `.msp`, `.msm`, `.mst`, `.wim`, and `.esd` packages across Linux, macOS, FreeBSD, illumos/Solaris, and Windows.

---

## Direct Replacement for WiX Toolset, CMake CPack & `msiexec`

Traditionally, authoring, compiling, inspecting, and executing Windows Installer databases required Windows machines, the .NET Framework, official WiX toolchains, or heavy compatibility layers like Wine. `msi-rs` replaces this entire ecosystem with lightweight, dependency-free native binaries:

### 1. Replacing WiX Toolset, CMake CPack & msitools (16 Standalone Binaries)
`msi-rs` replaces the entire WiX compilation, linking, harvesting, and decompilation toolchain without requiring .NET, Java, or Windows SDKs, and serves as an exact drop-in toolchain for CMake's `CPackWIX` generator:
- **WiX Toolset Suite (9 Binaries):**
  - **`candle`**: Full native parsing and compilation of WiX XML source files (`.wxs`, `.wxi`) across WiX v3, v4, and v5 schemas directly into typed intermediate `.wixobj` representations. Supports response files (`@response.txt`), `-sw<N>`, `-wx`, and CPack discovery banners.
  - **`light`**: High-performance symbol dependency graph solver, automatic sequence table ordering (`CostInitialize` through `InstallFinalize`), media layout splitting, cabinet packing (MSZIP, LZX, Quantum), and database generation directly into valid Compound File Binary Format (`.msi`) containers.
  - **`dark`**: Roundtrip decompiler extracting relational database tables, string pools, and embedded cabinets back into clean WiX XML declarations.
  - **`heat`**: Directory, file, and payload harvesting engine generating component and directory WiX XML fragments automatically.
  - **`lit`**: WiX library tool archiving intermediate `.wixobj` files into reusable `.wixlib` libraries.
  - **`pyro`**: Patch generation tool creating standard Windows Installer update patches (`.msp`) from database transform inputs.
  - **`smoke`**: Independent validation engine running native Internal Consistency Evaluators (`ICE01`, `ICE02`, `ICE03`, `ICE18`, `ICE33`, `ICE80`, `ICE99`, etc.).
  - **`torch`**: Database transformation tool diffing two MSI databases to synthesize `.mst` transforms.
  - **`wix`**: WiX v4/v5 multi-command frontend coordinating end-to-end builds, extensions, and packaging tasks.
- **msitools Utilities (6 Binaries):**
  - **`msibuild`**: Create and modify MSI databases directly from the command line.
  - **`msidiff`**: Compare two MSI databases with relational table diffing.
  - **`msidump`**: Export relational tables into IDT text format or stream dumps.
  - **`msiextract`**: Decompress and extract all files and embedded cabinets from MSI packages without executing an installation sequence.
  - **`msiinfo`**: Inspect and edit OLE summary information streams and table catalogs.
  - **`wixl`**: Cross-compiler executable shim replicating GNOME `wixl` (msitools) compiling and linking `.wxs` source directly into `.msi` packages.
- **Windows Installer Runtime Shim (1 Binary):**
  - **`msiexec`**: Dedicated command-line execution shim replicating Microsoft `msiexec.exe` parameter handling, return codes, and transaction dispatch across POSIX and Windows.
- **Cross-Platform Compilation & CPack Parity**: Build Windows Installer packages natively in Linux or macOS CI/CD workflows without spinning up Windows runners or Docker containers, with direct compatibility for CMake CPack (`CPackWIX`).

### 2. Replacing the Windows Installer Runtime (`msiexec.exe`)
`msi-rs` replaces `msiexec.exe` with `msi-cli`, a drop-in command-line tool and execution engine that runs natively across Unix and POSIX operating systems:
- **CLI Flag Parity**: Exact command-line parameter parity (`/i`, `/x`, `/a`, `/f[p|o|e|d|c|a|u|m|s|v]`, `/j[u|m]`, `/p`, `/qn`, `/qb`, `/qr`, `/qf`, `/l*`, and `PROPERTY=Value` overrides).
- **Two-Phase Transaction & Rollback Engine**: Translates MSI execution sequences into atomic operations with physical `.rbf` rollback quarantine preserving existing files and guaranteeing clean state restoration on failure or cancel.
- **Privileged Worker Boundary**: Replaces the Windows `msiserver` RPC service with cross-process IPC (Unix domain sockets / Windows named pipes) and privilege escalation (`sudo`, PolicyKit `pkexec`, macOS `SMJobBless`, or `runas`).
- **Standard Action Translation**: Translates MSI Win32 actions (`InstallFiles`, `WriteRegistryValues`, `CreateShortcuts`, `InstallServices`) into native POSIX filesystem hierarchies, service supervisors, and desktop launchers.
- **Native GUI & TUI**: Delivers both a desktop GUI wizard (`msi-gui`) replicating classic WiX dialog layouts (`WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`) and an interactive terminal wizard (`msi-cli install --tui`) for headless server installations.

### 3. Bare-Metal OS Provisioning (WIM/ESD Extraction)
Beyond standard userspace `.msi` installers, `msi-rs` operates as a privileged execution engine for bare-metal OS deployments:
- **Windows Imaging Format (WIM) & Solid ESD Engine**: Natively parses and validates `WIMHEADER` architectures, XML image manifests, and SHA-1 chunked offset lookup tables.
- **LZX, XPRESS, & LZMS Decompression**: Streams payloads from official Microsoft deployment media (`install.wim`, `install.esd`) using memory-safe delta-range coders and chunk sliding windows.
- **Offline Sysroot Targeting**: Integrates WIM extraction alongside native disk partitioning (`platform::disk`, `platform::partition`), EFI bootloader staging (`systemd-stub`), and offline registry editing (`platform::hive`) to deploy full Operating Systems without an underlying kernel installer.


---

## Visual Overview & Interfaces

### Native Desktop GUI Installer (`msi-gui`)
Interactive desktop installation wizard rendered via `egui` and `wgpu` (DirectX 12 / Metal / Vulkan) with pure software framebuffer fallback:

![Native Desktop GUI Installer - Welcome Dialog](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/gui_welcome_dialog.png)

*Figure 1: `msi-gui` running the WiX Mondo wizard theme on macOS / POSIX with dynamic property evaluation and license agreement handling.*

![Native Desktop GUI Installer - Feature Selection Dialog](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/gui_feature_tree_dialog.png)

*Figure 2: `msi-gui` interactive feature selection tree with volume disk space calculation and custom component selection.*

![Native Desktop GUI Installer - Live Progress Dialog](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/gui_progress_dialog.png)

*Figure 3: `msi-gui` real-time transaction worker streaming `ActionText` tickers, time remaining estimates, socket binding probes, and rollback quarantine monitoring.*

---

### Terminal TUI Wizard (`msi-cli --tui`)
Interactive terminal text wizard powered by the exact same `UiEngine` state machine for headless or remote SSH environments (or run automatically when no display server is detected):

![Interactive Terminal TUI Wizard](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/tui_wizard.png)

*Figure 4: Curses/raw-terminal mode TUI installation wizard displaying Unicode box drawing, interactive controls, and real-time action progress.*

---

### Command-Line Execution Pipeline (`msi-cli`)
CLI tool providing drop-in command-line parity with Microsoft Win32 `msiexec.exe`:

![Command-Line Installation Execution](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/cli_install_output.png)

*Figure 5: `msi-cli` running a transaction with volume costing, privileged worker IPC transition, LZX cabinet decompression, and POSIX translations.*

![Zero-.EXE Multi-Package EmbeddedChainer & Multi-Cabinet Pipeline](https://raw.githubusercontent.com/SamuelMarks/cc0-assets/master/msi-rs/screenshots/cli_chainer_output.png)

*Figure 6: `msi-cli` multi-cabinet media partitioning (`engine.cab`, `runtimes.cab`, `databases.cab`, `codebase.cab`) and zero-.EXE `MsiEmbeddedChainer` orchestration.*

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
| **WIM/ESD** | Windows Imaging Format (WIM) & LZMS Solid ESD | `crates/msi/src/wim/` |
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
│   ├── msi-cli/     # msiexec-compatible CLI tool and 16 standalone WiX/msitools/msiexec binaries
│   ├── msi-gui/     # Desktop native GUI wizard application (eframe / egui / wgpu / softbuffer)
│   ├── msi-ffi/     # C-compatible ABI shared and static libraries (include/msi.h)
│   └── msi-python/  # Python 3 native PyO3 extension module (import msi)
├── docs/
│   └── python_guide.md # Comprehensive Python packaging guide and API reference
├── ARCHITECTURE.md  # In-depth architectural design, schemas, and specifications
├── USAGE.md         # Comprehensive usage manual, CLI recipes, library guides, and CI/CD examples
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
print(f"Product: {pkg.get_property('ProductName')} v{pkg.get_property('ProductVersion')}")
for table in pkg.table_names:
    print(f"  Table: {table}")
```

See the complete Python guide and API documentation in [`docs/python_guide.md`](docs/python_guide.md).

### C-ABI Foreign Function Interface (`msi-ffi`)
`msi-ffi` compiles to a shared (`.so` / `.dylib` / `.dll`) and static (`.a`) library exposing a clean C-ABI with standard header [`crates/msi-ffi/include/msi.h`](crates/msi-ffi/include/msi.h):
- **Cross-Language Interop**: Ready for C, C++, Go, C#, Swift, Zig, and Rust FFI.
- **Safety Boundary**: Strict panic isolation boundaries (`catch_unwind`) converting internal unwinds into defined error return codes (`MSI_ERROR_*`).
- **Handle-Based API**: Opaque handles for package construction (`MsiPackageBuilderHandle`) and inspection (`MsiPackageHandle`).

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
- **SQL Parser & Query Engine:** AST, lexer, recursive descent parser, and relational execution engine supporting the Windows Installer SQL dialect (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `ALTER TABLE`, joins, and WHERE expressions).
- **Transforms & IDT:** `.mst` database transform generation, diffing, and patching; IDT tab-delimited table import and export.
- **POSIX Extensions:** Extension tables for POSIX permissions (`PosixFile`), symlinks (`PosixSymlink`), system daemons (`PosixDaemon`), ACLs (`PosixAcl`), and Freedesktop entries (`PosixDesktop`).

### 3. WiX Toolset Pipeline & Pure-Rust `.msi` Creation
- **WiX Compiler & Linker:**
  - Multi-source `.wxs` input support (`msi pack -o App.msi --multi-fragment Product.wxs Payload.wxs`).
  - Cross-fragment symbol resolution (`<ComponentGroupRef Id="..." />`).
  - Preprocessor defines (`-d VAR=VAL` / `-dVAR=VAL`) and suppression flags (`-sval`, `-sice:<rule>`).
  - Authoring of platform daemons and services (`<ServiceInstall>` & `<ServiceControl>`) with automatic standard action injection (`StopServices`, `DeleteServices`, `InstallServices`, `StartServices`).
  - Authoring of desktop and Start Menu shortcuts, system environment variables, registry searches (`<RegistrySearch>`, `<FileSearch>`), and launch conditions (`<Condition Message="...">`).
  - Multi-cabinet deterministic media partitioning for offline air-gapped installers (`engine.cab`, `runtimes.cab`, `databases.cab`, `codebase.cab`) with non-overlapping sequence boundaries and compression levels (`high` with LZX vs `medium` with MSZIP).
  - Native Payload Harvester (`msi harvest`) drop-in replacement for shell harvesting scripts: strictly respects `.gitignore`, generates deterministic RFC 4122 v5 UUIDs, and auto-hashes identifiers exceeding 72 characters (`CMP_<hash>`, `FIL_<hash>`).
- **Dynamic WiX Extension Ecosystem (10 Built-In Extensions):**
  - Modular extension registry (`ExtensionRegistry::with_builtin_extensions()`) executing custom XML element compilers and linker mutations natively:
    - **`WixUIExtension`**: Standard dialog presets (`WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`, `WixUI_Minimal`), fonts, and branding variables (`WixUIBannerBmp`, `WixUIDialogBmp`, `WixUILicenseRtf`).
    - **`WixUtilExtension`**: User and local group creation (`<util:User>`, `<util:Group>`), XML file mutations (`<util:XmlFile>`, `<util:XmlConfig>`), SMB file shares (`<util:FileShare>`), and performance counters.
    - **`WixFirewallExtension`**: Windows Defender Firewall exception rules (`<firewall:FirewallException>`) with transactional rollback.
    - **`WixBalExtension`**: Burn Application Logic schemas and UI state machines (`<bal:WixStandardBootstrapperApplication>`).
    - **`WixNetFxExtension`**: Native Image Generator optimization (`<netfx:NativeImage>`).
    - **`WixHttpExtension`**: HTTP Server API URL reservations (`<http:UrlReservation>`) and SSL certificate bindings.
    - **`WixIIsExtension`**: Internet Information Services web sites, application pools, and virtual directories (`<iis:WebSite>`, `<iis:WebAppPool>`, `<iis:WebVirtualDir>`).
    - **`WixSqlExtension`**: SQL Server database instance creation and SQL script execution (`<sql:SqlDatabase>`, `<sql:SqlScript>`).
    - **`WixComPlusExtension`**: COM+ application, component, and role configuration (`<complus:ComPlusApplication>`).
    - **`WixDependencyExtension`**: Inter-package ref-counting and dependency resolution (`<dep:Provides>`, `<dep:Requires>`).
- **CMake / CPack WIX Drop-In Replacement:**
  - Complete CLI banner, option, and execution parity for CMake's `CPackWIX` generator (`candle`, `light`, and `wix`).
  - Response file support (`@response.txt`) for expansive file and directory lists generated by CMake.
  - Automatic handling of CMake properties: `CPACK_WIX_UPGRADE_GUID`, `CPACK_WIX_PRODUCT_GUID`, `CPACK_WIX_PRODUCT_ICON`, `CPACK_WIX_UI_BANNER`, `CPACK_WIX_UI_DIALOG`, `CPACK_RESOURCE_FILE_LICENSE`.
  - CPack XML patch file support (`CPACK_WIX_PATCH_FILE`) with fragment targeting (`#PRODUCT`, `#PRODUCTFEATURE`), attribute merging, and element deletion.
  - Component-based multi-cabinet splitting (`CPACK_WIX_CAB_PER_COMPONENT`) and multi-culture localization (`CPACK_WIX_CULTURES` with `-cultures:<list>` and `-loc`).
- **Complete Internal Consistency Evaluators (100% ICE Suite / 105 Rules):**
  - Full modular validation engine (`crates/msi/src/wix/ice/`) implementing all 105 official Windows Installer rules across 7 categories:
    - **Structural & Metadata**: `ICE16`, `ICE29`, `ICE35`, `ICE37`, `ICE39`, `ICE40`, `ICE41`, `ICE45`, `ICE46`, `ICE48`, `ICE51`, `ICE53`, `ICE58`, `ICE70`, `ICE71`, `ICE73`, `ICE74`, `ICE82`, `ICE84`, `ICE87`, `ICE92`, `ICE93`, `ICE95`.
    - **Sequence Tables & Pipeline**: `ICE11`, `ICE12`, `ICE26`, `ICE27`, `ICE28`, `ICE42`, `ICE63`, `ICE67`, `ICE68`, `ICE72`, `ICE75`, `ICE77`, `ICE78`, `SequenceGraph`.
    - **Components & Relational Integrity**: `ICE10`, `ICE14`, `ICE19`, `ICE21`, `ICE22`, `ICE43`, `ICE47`, `ICE57`, `ICE59`, `ICE64`, `ICE69`, `ICE79`, `ICE89`, `ICE90`, `ICE91`.
    - **UI & Dialog Controls**: `ICE13`, `ICE23`, `ICE24`, `ICE34`, `ICE44`, `ICE86`, `ICE102`, `ICE104`.
    - **File Table Rules**: `ICE31`, `ICE54`, `ICE60`, `ICE96`.
    - **System & Search Tables**: `ICE15`, `ICE17`, `ICE32`, `ICE36`, `ICE49`, `ICE50`, `ICE52`, `ICE55`, `ICE56`, `ICE65`, `ICE85`, `ICE88`, `ICE100`.
    - **Advanced Subsystems**: `ICE25`, `ICE62`, `ICE66`, `ICE76`, `ICE81`, `ICE83`, `ICE94`, `ICE97`, `ICE98`, `ICE105`.
  - Registered in `IceRegistry::with_standard_rules()` and exposed through `smoke.exe`, `light.exe`, and `msibuild.exe`.

### 4. Cross-Platform Platform Translation & Offline Sysroot Provisioning (`msi-platform`)
- **Transparent Directory Translation:**
  - Standard Windows directories translated automatically across Windows, Linux FHS/XDG, macOS Darwin/Library, FreeBSD `hier(7)`, and SunOS/illumos:
    - `ProgramFiles64Folder`: `/opt/<Vendor>` (Linux, SunOS) / `/usr/local/<Vendor>` (FreeBSD) / `/Applications` (macOS).
    - `CommonFilesFolder`: `/usr/share` (Linux, SunOS) / `/usr/local/share` (FreeBSD) / `/Library/Application Support` (macOS).
    - `CommonAppDataFolder`: `/var/lib/<Product>` (Linux) / `/var/db/<Product>` (FreeBSD) / `/etc/opt/<Product>` (SunOS).
    - `AppDataFolder`: `$XDG_CONFIG_HOME/<Product>` (Linux, FreeBSD, SunOS) / `~/Library/Application Support/<Product>` (macOS).
    - `DesktopFolder`: `$XDG_DESKTOP_DIR` / XDG `.desktop` (Linux, FreeBSD, SunOS) / `~/Desktop` or `.app` alias (macOS).
    - `TempFolder`: `$TMPDIR` / `/tmp`.
- **Service Supervisors:**
  - Linux `systemd`: Auto-generates unit files in `/etc/systemd/system/<service>.service`, managed via `systemctl`.
  - macOS `launchd`: Auto-generates property lists in `/Library/LaunchDaemons/<service>.plist`, managed via `launchctl`.
  - FreeBSD `rc.d`: Auto-generates `rc.subr` scripts in `/usr/local/etc/rc.d/<service>`, managed via `sysrc` and `service`.
  - SunOS/illumos `SMF`: Auto-generates XML manifests in `/var/svc/manifest/site/`, managed via `svccfg` and `svcadm`.
  - Windows: Native Windows Service Control Manager (`sc.exe` / SCM API).
- **Offline Sysroots & Bare-Metal OS Provisioning:**
  - Pre-boot sysroot targeting with configurable execution sandboxing policies (`PermissiveChroot`, `SkipWithSuccess`, `StrictReject`).
  - Synthetic in-process mocking of live subsystem APIs unavailable during offline pre-boot installation (`SCM`, `RPC`, `NetApi`).
  - UEFI bootloader staging (`/EFI/BOOT/BOOTX64.EFI`), offline Windows Boot Configuration Data (BCD) hive synthesis (`{bootmgr}`, `{default}`), and Linux bootloader generator support (`systemd-boot`, `GRUB2`, `Limine`).
  - Non-volatile EFI NVRAM variable manipulation (`efivarfs`) programming `BootXXXX` and `BootOrder` entries.
  - Disk block device scanning, GPT partition table layout synthesis, and native filesystem formatters (FAT32, ext4, NTFS).
  - Offline Windows registry hive editing (`SYSTEM`, `SOFTWARE`, `BCD`) directly from disk images without requiring running Windows kernels.
- **Strict Incompatibility Rejection:**
  - Rejects Windows-only kernel mechanisms (`SERVICE_KERNEL_DRIVER`, file system filter drivers, COM+ DCOM catalog registration `ICE97`) with typed `Error::UnsupportedPlatformFeature { feature, target_os, reason }` ensuring zero silent failures or corruption.
- **Desktop Integration:** Freedesktop `.desktop` application launchers and macOS `.app` bundle synthesis with icon conversion.
- **Registry Emulation:** Embedded ACID `RegistryStore` database mapping `HKLM`, `HKCU`, `HKCR`, and `HKU` to JSON or SQLite files on POSIX, and native Win32 Registry APIs on Windows.

### 5. Execution Engine & Two-Phase Transaction Lifecycle
- **Standard Actions Sequence Lifecycle:**
  - `CostInitialize`, `FileCost`, `CostFinalize` with volume and mount point space calculations.
  - `InstallValidate` checking component run states and disk quotas.
  - `InstallInitialize` and `InstallFinalize` managing transactional boundaries.
  - `ProcessComponents` evaluating install states and reference counting.
  - `InstallFiles` and `RemoveFiles` with atomic writes, backup quarantine, and rollback journals.
  - `WriteRegistryValues`, `WriteEnvironmentStrings`, `CreateShortcuts`.
- **Custom Action Translation & Interception:**
  - **Type 18 / 34 Shell Command Translation**: Translates `cmd.exe /c [INSTALLFOLDER]libscript\libscript.cmd ...` to `/bin/sh [INSTALLFOLDER]/libscript/libscript.sh ...` on POSIX, passing `CustomActionData` in the environment.
  - **Type 6 VBScript Replacement**: Intercepts `validate_*.vbs` port checks and executes in-process socket probes using `std::net::TcpListener::bind`, setting `VALID_<pkg>="1"` / `VALID_<pkg>="0"` with diagnostics.
  - **Type 1 DLL In-Process SQL Provisioning**: Intercepts `sql_provisioner.dll` actions, running schema creation, user creation, and grants in-process via `SqlProvisionerClient`, respecting `PURGE_DATA="1"` on rollback.
- **Zero-.EXE Multi-Package Orchestration (`EmbeddedChainer`):**
  - Reads `MsiEmbeddedChainer` table and extracts child packages to secure temporary spool storage.
  - Sequentially executes child packages within an atomic multi-package transaction with cascading rollback.
- **Shared Component Reference Counting (`SharedDllRefCount`):**
  - Tracks client `ProductCode` associations per `ComponentId` GUID in `RegistryStore`.
  - Retains shared services and files until reference count reaches zero.
- **Non-Blocking Background Worker (`BackgroundTransactionWorker`):**
  - Runs the installation transaction on a background thread while keeping GUI responsive at 60 FPS.
  - Cross-thread progress channel streaming `percent`, `time_remaining_secs`, `progress1`, `progress2`, and `action_text` ticker messages.
  - Handles cancel requests gracefully, unwinding compensating rollback script actions.
- **Terminal TUI Wizard (`TerminalWizard`):**
  - Unicode box-drawing console windowing, interactive EULA viewer, radio mode selector, component checklist, sensitive password masking, live progress bar, F2 diagnostics log drawer, and exit summary screen.
  - Auto-launches when interactive TTY is detected without a display server.

---

## Getting Started

### Prerequisites
- Rust 1.85+ (Stable)
- `cargo` package manager

### Building
```bash
# Build all workspace crates and binaries
cargo build --release

# Run the complete test suite (1,030+ unit, integration, doc, and binding tests)
cargo test --workspace

# Verify strict quality and lints
cargo clippy --all-targets -- -D warnings -D clippy::pedantic
```

---

## Usage Examples

### Command-Line Interface (`msi-cli`) & `msiexec`

```bash
# Install a package silently with verbose logging (subcommand syntax)
msi-cli install ./ExampleApp.msi --ui quiet --log ./install.log

# Install with basic progress UI and property overrides
msi-cli install ./ExampleApp.msi --ui basic INSTALLDIR="/opt/custom" APP_ENV="production"

# Direct msiexec flag parity via msi-cli (drop-in replacement)
msi-cli /i ./ExampleApp.msi /qn /lvx ./install.log
msi-cli /x ./ExampleApp.msi /qb
msi-cli /fa ./ExampleApp.msi

# Or use the standalone msiexec binary directly
msiexec /i ./ExampleApp.msi /qn /lvx ./install.log

# Inspect package summary information and catalogs
msi-cli info ./ExampleApp.msi

# Repair missing or modified files via subcommand
msi-cli repair ./ExampleApp.msi --flags a

# Uninstall an installed package via subcommand
msi-cli uninstall ./ExampleApp.msi --ui basic

# Create a new MSI package scaffold from template
msi-cli create --name "MyService" --version "1.0.0" --manufacturer "Acme Corp" --product-code "{12345678-1234-1234-1234-1234567890AB}"
```

### Terminal TUI Wizard

```bash
# Launch interactive terminal wizard in curses/TUI mode
msi-cli install ./ExampleApp.msi --tui

# Runs automatically when executing without a GUI display server (headless/SSH)
msi-cli install ./ExampleApp.msi
```

### WiX Toolset, CPack & Utilities

```bash
# Compile WiX source into an intermediate object
candle Product.wxs -o Product.wixobj

# Link object into an MSI database with validation
light Product.wixobj -o Product.msi

# Compile and link in one step with GNOME wixl shim (msitools)
wixl -a x64 -o Product.msi Product.wxs

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
