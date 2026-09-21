"""
Comprehensive test suite for the msi Python package.
"""

import os
import shutil
import sys
import tempfile
from pathlib import Path

# Ensure crates/msi-python/python is in sys.path
repo_root = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(repo_root / "crates" / "msi-python" / "python"))

import msi


def test_product_version_parsing_and_comparisons() -> None:
    v1 = msi.ProductVersion(1, 0, 0)
    assert v1.major == 1
    assert v1.minor == 0
    assert v1.build == 0
    assert v1.as_tuple() == (1, 0, 0)
    assert str(v1) == "1.0.0"

    v2 = msi.ProductVersion.parse("1.2.3")
    assert v2.as_tuple() == (1, 2, 3)

    assert v1 < v2
    assert v1 <= v2
    assert v1 != v2
    assert v2 > v1
    assert v2 >= v1
    assert v1 == msi.ProductVersion(1, 0, 0)


def test_package_builder_and_package_lifecycle() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        msi_out = tmp_path / "builder_test.msi"

        with msi.PackageBuilder(
            product_name="TestApp",
            manufacturer="Acme Corp",
            version=(2, 1, 0),
            product_code="{11111111-2222-3333-4444-555555555555}",
            upgrade_code="{88888888-9999-0000-1111-222222222222}",
        ) as builder:
            builder.set_product_name("UpdatedTestApp")
            builder.set_manufacturer("Acme Updated")
            builder.set_version("2.1.5")
            builder.add_property("CustomGreeting", "HelloFromPython")

            builder.add_directory("TARGETDIR", None, "SourceDir")
            builder.add_directory("INSTALLDIR", "TARGETDIR", "AppDir|TestApp")

            builder.add_component(
                comp_id="MainComp",
                dir_id="INSTALLDIR",
                comp_guid="{33333333-4444-5555-6666-777777777777}",
            )

            builder.add_feature(
                feat_id="Complete",
                title="Complete Feature",
                description="Main application files",
                level=1,
            )
            builder.add_feature_component("Complete", "MainComp")

            builder.add_file(
                file_id="AppExe",
                comp_id="MainComp",
                file_name="app.exe",
                file_size=2048,
            )

            builder.add_media(
                disk_id=1,
                last_sequence=1,
                cabinet="#cab1.cab",
            )

            builder.add_embedded_cabinet("#cab1.cab", b"dummy_cabinet_stream_data")

            # Build in-memory bytes
            raw_bytes = builder.build_to_bytes()
            assert len(raw_bytes) > 512

            # Build and save to file
            builder.build_to_file(str(msi_out))
            assert msi_out.exists()

        # Open and inspect
        with msi.Package.open(str(msi_out)) as pkg:
            assert pkg.get_property("ProductName") == "UpdatedTestApp"
            assert pkg.get_property("Manufacturer") == "Acme Updated"
            assert pkg.get_property("ProductVersion") == "2.1.5"
            assert pkg.get_property("CustomGreeting") == "HelloFromPython"

            tables = pkg.table_names
            assert "Property" in tables
            assert "Directory" in tables
            assert "Component" in tables
            assert "Feature" in tables
            assert "File" in tables

            prop_rows = pkg.get_table("Property")
            assert any(
                r.get("Property") == "CustomGreeting"
                and r.get("Value") == "HelloFromPython"
                for r in prop_rows
            )


def test_build_msi_from_directory_tree() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        source_dir = tmp_path / "source_tree"
        source_dir.mkdir()

        # Create nested directory tree
        sub_dir = source_dir / "assets"
        sub_dir.mkdir()

        f1 = source_dir / "main.py"
        f1.write_text("print('hello world')", encoding="utf-8")

        f2 = sub_dir / "config.json"
        f2.write_text('{"app": "msi_test", "version": 1}', encoding="utf-8")

        f3 = sub_dir / "icon.bin"
        f3.write_bytes(b"\x00\x01\x02\x03\x04\x05\x06\x07")

        out_msi = tmp_path / "packaged.msi"

        progress_events = []

        def on_progress(phase: str, cur: int, total: int) -> None:
            progress_events.append((phase, cur, total))

        msi.build_msi(
            source_dir=source_dir,
            output_path=out_msi,
            product_name="DirectoryTreeApp",
            version="3.0.0",
            manufacturer="Packer Vendor",
            progress_callback=on_progress,
        )

        assert out_msi.exists()
        assert len(progress_events) > 0

        # Inspect generated package
        info = msi.inspect_msi(out_msi)
        assert info["metadata"]["ProductName"] == "DirectoryTreeApp"
        assert info["metadata"]["ProductVersion"] == "3.0.0"
        assert "File" in info["tables"]
        assert len(info["tables"]["File"]) == 3

        # Test extraction
        extract_dir = tmp_path / "extracted"
        pkg = msi.Package.open(str(out_msi))
        extracted_files = pkg.extract_cabinet("#cab1.cab", str(extract_dir))
        assert len(extracted_files) == 3

        # Verify extracted contents match original bytes
        extracted_paths = [p for p in extract_dir.glob("**/*") if p.is_file()]
        assert len(extracted_paths) == 3

        # Build with explicit product_code, upgrade_code, custom install_dir_name, and no progress_callback
        out_msi_explicit = tmp_path / "explicit.msi"
        msi.build_msi(
            source_dir=source_dir,
            output_path=out_msi_explicit,
            product_name="ExplicitApp",
            version="1.0.0",
            manufacturer="Explicit Vendor",
            product_code="{11111111-2222-3333-4444-555555555555}",
            upgrade_code="{66666666-7777-8888-9999-000000000000}",
            install_dir_name="CustomInstallFolder",
            progress_callback=None,
        )
        assert out_msi_explicit.exists()
        info_exp = msi.inspect_msi(out_msi_explicit)
        assert (
            info_exp["metadata"]["ProductCode"]
            == "{11111111-2222-3333-4444-555555555555}"
        )
        assert (
            info_exp["metadata"]["UpgradeCode"]
            == "{66666666-7777-8888-9999-000000000000}"
        )


def test_sanitize_identifier() -> None:
    assert msi.sanitize_identifier("") == "_id"
    assert msi.sanitize_identifier("123abc") == "_123abc"
    assert msi.sanitize_identifier(".hidden") == "_.hidden"
    assert msi.sanitize_identifier("Valid_Id.123") == "Valid_Id.123"
    assert msi.sanitize_identifier("a" * 100) == "a" * 72
    assert msi.sanitize_identifier("hello-world!@#") == "hello_world___"


def test_compile_wix() -> None:
    wxs_content = """
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="$(var.AppName)" Version="1.5.0" Manufacturer="WiXPythonCorp">
        <Package Description="Testing WiX via Python" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="C1">
                <File Id="F1" Source="test.txt" />
            </Component>
        </Directory>
        <Feature Id="F" Title="Main" Level="1">
            <ComponentRef Id="C1" />
        </Feature>
    </Product>
</Wix>
"""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        out_msi = tmp_path / "wix_output.msi"

        # Test from string with variables
        msi.compile_wix(wxs_content, out_msi, variables={"AppName": "WiXPythonApp"})
        assert out_msi.exists()

        pkg = msi.Package.open(str(out_msi))
        assert pkg.get_property("ProductName") == "WiXPythonApp"
        assert pkg.get_property("ProductVersion") == "1.5.0"

        # Test from raw string without variables
        plain_wxs = wxs_content.replace("$(var.AppName)", "DirectFileApp")
        out_raw_plain_msi = tmp_path / "raw_plain_output.msi"
        msi.compile_wix(plain_wxs, out_raw_plain_msi)
        assert out_raw_plain_msi.exists()
        pkg_raw = msi.Package.open(str(out_raw_plain_msi))
        assert pkg_raw.get_property("ProductName") == "DirectFileApp"
        wxs_file = tmp_path / "app.wxs"
        wxs_file.write_text(plain_wxs, encoding="utf-8")
        out_file_msi = tmp_path / "wix_file_output.msi"
        msi.compile_wix(wxs_file, out_file_msi)
        assert out_file_msi.exists()
        pkg_file = msi.Package.open(str(out_file_msi))
        assert pkg_file.get_property("ProductName") == "DirectFileApp"

        # Test from file on disk with variables
        var_wxs_file = tmp_path / "app_var.wxs"
        var_wxs_file.write_text(wxs_content, encoding="utf-8")
        out_file_var_msi = tmp_path / "wix_file_var_output.msi"
        msi.compile_wix(
            var_wxs_file, out_file_var_msi, variables={"AppName": "FileVarApp"}
        )
        assert out_file_var_msi.exists()
        pkg_file_var = msi.Package.open(str(out_file_var_msi))
        assert pkg_file_var.get_property("ProductName") == "FileVarApp"


def test_error_handling() -> None:
    # Invalid version
    try:
        msi.ProductVersion.parse("invalid_version")
        assert False, "Expected ValidationError"
    except msi.ValidationError:
        pass

    # Non-existent source dir
    try:
        msi.build_msi(
            source_dir="/non/existent/path/never/found",
            output_path="/tmp/test.msi",
            product_name="BadApp",
            version="1.0",
            manufacturer="Bad",
        )
        assert False, "Expected IoError"
    except msi.IoError:
        pass


if __name__ == "__main__":
    print("Running Python tests manually...")
    test_product_version_parsing_and_comparisons()
    test_package_builder_and_package_lifecycle()
    test_build_msi_from_directory_tree()
    test_compile_wix()
    test_error_handling()
    print("All Python tests passed successfully!")
