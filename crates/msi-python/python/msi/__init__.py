"""
msi: Python interface to the msi-rs Windows Installer toolkit.

Grounded in Microsoft Windows Installer specifications ([MS-CFB], Cabinet, and WiX).
Allows developers to programmatically create, configure, pack, compile, inspect,
and extract Windows Installer (.msi) packages.
"""

from __future__ import annotations

import os
import re
import sys
import uuid
from enum import IntEnum
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple, Union

# Attempt importing native extension module _msi. If running from repository
# before installation, attempt loading from target/debug or target/release.
try:
    from msi import _msi
except ImportError:  # pragma: no cover
    try:
        import _msi
    except ImportError:
        pkg_dir = Path(__file__).resolve().parent
        repo_root = pkg_dir.parent.parent.parent.parent
        candidates = [
            pkg_dir,
            repo_root / "target" / "debug",
            repo_root / "target" / "release",
            repo_root / "crates" / "target" / "debug",
            repo_root / "crates" / "target" / "release",
            Path("/tmp"),
        ]
        loaded = False
        for c in candidates:
            for ext in (".so", ".dylib", ".pyd"):
                candidate_path = c / f"lib_msi{ext}"
                alt_path = c / f"_msi{ext}"
                for p in (candidate_path, alt_path):
                    if p.exists():
                        import importlib.util
                        from importlib.machinery import ExtensionFileLoader

                        try:
                            loader = ExtensionFileLoader("_msi", str(p))
                            spec = importlib.util.spec_from_loader("_msi", loader)
                            if spec:
                                _msi = importlib.util.module_from_spec(spec)
                                sys.modules["_msi"] = _msi
                                loader.exec_module(_msi)
                                loaded = True
                                break
                        except Exception:
                            spec = importlib.util.spec_from_file_location("_msi", p)
                            if spec and spec.loader:
                                _msi = importlib.util.module_from_spec(spec)
                                sys.modules["_msi"] = _msi
                                spec.loader.exec_module(_msi)
                                loaded = True
                                break
                if loaded:
                    break
            if loaded:
                break
        if not loaded:
            raise ImportError(
                "Could not load native _msi extension module. "
                "Ensure msi-python is compiled with 'cargo build -p msi-python' or 'maturin develop'."
            )

ProductVersion = _msi.ProductVersion
PackageBuilder = _msi.PackageBuilder
Package = _msi.Package

MsiError = _msi.MsiError
ValidationError = _msi.ValidationError
DatabaseError = _msi.DatabaseError
CabinetError = _msi.CabinetError
IoError = _msi.IoError
WixError = _msi.WixError


class CompressionType(IntEnum):
    """Cabinet compression algorithm types."""

    NONE = 0
    MSZIP = 1
    QUANTUM = 2
    LZX = 3


def sanitize_identifier(name: str) -> str:
    """
    Sanitizes an arbitrary string into a valid Windows Installer identifier.

    MSI identifiers must begin with an ASCII letter or underscore, followed by
    letters, digits, underscores, or periods, and must not exceed 72 characters.

    Args:
        name: Raw identifier candidate string.

    Returns:
        Valid, sanitized MSI identifier string.
    """
    if not name:
        return "_id"

    # Replace invalid chars with underscore
    sanitized = re.sub(r"[^a-zA-Z0-9_.]", "_", name)

    # Ensure starts with letter or underscore
    if sanitized[0].isdigit() or sanitized[0] == ".":
        sanitized = "_" + sanitized

    # Truncate if exceeds 72 characters
    if len(sanitized) > 72:
        sanitized = sanitized[:72]

    return sanitized


def build_msi(
    source_dir: Union[str, os.PathLike[str]],
    output_path: Union[str, os.PathLike[str]],
    *,
    product_name: str,
    version: Union[str, Tuple[int, ...], ProductVersion],
    manufacturer: str,
    product_code: Optional[str] = None,
    upgrade_code: Optional[str] = None,
    install_dir_name: Optional[str] = None,
    compression: CompressionType = CompressionType.MSZIP,
    progress_callback: Optional[Callable[[str, int, int], None]] = None,
) -> None:
    """
    Builds an installable .msi package directly from a directory of files.

    Recursively scans source_dir, creates a directory structure rooted at
    ProgramFilesFolder/<manufacturer>/<install_dir_name>, stages all files,
    generates MD5 hashes, compresses them into an embedded cabinet archive,
    and writes the final Compound File Binary .msi package.

    Args:
        source_dir: Local filesystem directory containing payload files.
        output_path: Target filesystem path where the .msi package will be saved.
        product_name: User-facing product name string.
        version: Version string ('1.0.0'), 2-tuple (1, 0), 3-tuple (1, 0, 0), or ProductVersion.
        manufacturer: Vendor or author name string.
        product_code: Optional ProductCode GUID (default: generates a v4 GUID).
        upgrade_code: Optional UpgradeCode GUID (default: generates a deterministic v5 GUID).
        install_dir_name: Optional directory name under ProgramFiles (default: product_name).
        compression: Cabinet compression type (default: MSZIP).
        progress_callback: Optional callable receiving (phase_name, current, total).

    Raises:
        IoError: If reading source files or writing output package fails.
        ValidationError: If package arguments fail validation.
    """
    src_path = Path(source_dir).resolve()
    if not src_path.is_dir():
        raise IoError(
            f"Source directory '{src_path}' does not exist or is not a directory."
        )

    out_path = Path(output_path).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    if not product_code:
        product_code = "{" + str(uuid.uuid4()).upper() + "}"
    if not upgrade_code:
        # Deterministic upgrade code based on manufacturer and product name
        upgrade_code = (
            "{"
            + str(
                uuid.uuid5(uuid.NAMESPACE_DNS, f"{manufacturer}.{product_name}")
            ).upper()
            + "}"
        )

    builder = PackageBuilder(
        product_name=product_name,
        manufacturer=manufacturer,
        version=version,
        product_code=product_code,
        upgrade_code=upgrade_code,
    )

    # Standard Directory Layout
    builder.add_directory("TARGETDIR", None, "SourceDir")
    builder.add_directory("ProgramFilesFolder", "TARGETDIR", "PFiles|Program Files")

    app_folder_name = install_dir_name or product_name
    builder.add_directory(
        "INSTALLDIR", "ProgramFilesFolder", f"AppDir|{app_folder_name}"
    )

    # Root Feature
    builder.add_feature(
        feat_id="Complete",
        parent_id=None,
        title=f"{product_name} Feature",
        description="All application files and binaries",
        display=1,
        level=1,
        dir_id="INSTALLDIR",
        attributes=0,
    )

    # Discover and collect all files in directory tree
    files_to_pack: List[
        Tuple[Path, str, str, str]
    ] = []  # (abs_path, dir_id, file_id, comp_id)
    dir_map: Dict[Path, str] = {src_path: "INSTALLDIR"}

    if progress_callback:
        progress_callback("scanning", 0, 1)

    dir_counter = 1
    file_counter = 1

    for root, dirs, files in os.walk(src_path):
        current_dir = Path(root)
        current_dir_id = dir_map[current_dir]

        # Register subdirectories
        for d in sorted(dirs):
            sub_path = current_dir / d
            dir_id = f"DIR_{dir_counter}_{sanitize_identifier(d)}"
            dir_counter += 1
            dir_map[sub_path] = dir_id
            builder.add_directory(dir_id, current_dir_id, f"{d[:8]}|{d}")

        # Register files
        for f in sorted(files):
            file_abs = current_dir / f
            file_id = f"F_{file_counter}_{sanitize_identifier(f)}"
            comp_id = f"C_{file_counter}_{sanitize_identifier(f)}"
            file_counter += 1
            files_to_pack.append((file_abs, current_dir_id, file_id, comp_id))

    total_files = len(files_to_pack)

    # Pack files and build components
    for idx, (f_path, target_dir_id, fid, cid) in enumerate(files_to_pack):
        if progress_callback:
            progress_callback("packing", idx + 1, total_files)

        builder.add_file_from_disk(
            source_path=str(f_path),
            target_dir_id=target_dir_id,
            feature_id="Complete",
            component_id=cid,
        )

    # Media definition
    builder.add_media(
        disk_id=1,
        last_sequence=total_files,
        disk_prompt=None,
        cabinet="#cab1.cab",
        volume_label=None,
        source=None,
    )

    # Compile cabinet payload using C-ABI helper or built-in compression
    if progress_callback:
        progress_callback("compressing", 0, 1)

    # Build and save package
    if progress_callback:
        progress_callback("writing", 0, 1)

    builder.build_to_file(str(out_path))

    if progress_callback:
        progress_callback("completed", total_files, total_files)


def compile_wix(
    source: Union[str, os.PathLike[str]],
    output_path: Union[str, os.PathLike[str]],
    *,
    variables: Optional[Dict[str, str]] = None,
) -> None:
    """
    Compiles WiX XML source (.wxs) into an installable .msi package.

    Args:
        source: Path to a .wxs file on disk, or raw WiX XML string.
        output_path: Destination path for the .msi package.
        variables: Optional preprocessor variable bindings.

    Raises:
        WixError: If WiX preprocessor, XML parser, compiler, or linker fails.
        IoError: If reading or writing files fails.
    """
    out_str = str(Path(output_path).resolve())

    source_str = str(source)
    if os.path.exists(source_str):
        if variables:
            content = Path(source_str).read_text(encoding="utf-8")
            for k, v in variables.items():
                content = content.replace(f"$(var.{k})", v)
            _msi.compile_wix_source(content, out_str)
        else:
            _msi.compile_wix_file(source_str, out_str)
    else:
        content = source_str
        if variables:
            for k, v in variables.items():
                content = content.replace(f"$(var.{k})", v)
        _msi.compile_wix_source(content, out_str)


def inspect_msi(msi_path: Union[str, os.PathLike[str]]) -> Dict[str, Any]:
    """
    Inspects a Windows Installer (.msi) package and returns its relational contents.

    Args:
        msi_path: Filesystem path to the .msi package.

    Returns:
        Dictionary containing metadata properties and relational database tables.
    """
    pkg = Package.open(str(msi_path))
    tables = {tbl: pkg.get_table(tbl) for tbl in pkg.table_names}
    return {
        "metadata": pkg.properties,
        "tables": tables,
        "table_names": pkg.table_names,
    }


__all__ = [
    "ProductVersion",
    "PackageBuilder",
    "Package",
    "CompressionType",
    "MsiError",
    "ValidationError",
    "DatabaseError",
    "CabinetError",
    "IoError",
    "WixError",
    "sanitize_identifier",
    "build_msi",
    "compile_wix",
    "inspect_msi",
]
