# Python Guide: Building Windows Installer (.msi) Packages with `msi`

The `msi` Python package provides high-performance native bindings to `msi-rs`, allowing you to create, inspect, and extract `.msi` packages from Python without needing Windows or the WiX toolset installed.

---

## 1. Quick Start: One-Liner Directory-to-MSI

To package an entire local folder into a standard, installable Windows Installer package:

```python
import msi

msi.build_msi(
    source_dir="path/to/my_application",
    output_path="output/MyApp.msi",
    product_name="My Awesome Application",
    version="1.0.0",
    manufacturer="Acme Corporation",
)
```

This automatically:
- Traverses the directory recursively.
- Generates standard `Directory`, `Component`, and `File` records.
- Calculates MD5 file hashes for the `FileHash` table.
- Compresses files into an embedded `#cab1.cab` archive using MSZIP compression.
- Generates standard installation sequence tables (`InstallExecuteSequence`, `InstallUISequence`).
- Writes a valid Compound File Binary `.msi` file.

---

## 2. Advanced: Fluent `PackageBuilder` API

For precise control over directory IDs, GUIDs, features, components, and properties:

```python
import msi

# Initialize package builder
builder = msi.PackageBuilder(
    product_name="Custom App",
    manufacturer="Acme Corp",
    version=msi.ProductVersion(1, 2, 0),
    product_code="{11111111-2222-3333-4444-555555555555}",
    upgrade_code="{99999999-8888-7777-6666-555555555555}",
)

# Define Directory Hierarchy
builder.add_directory("TARGETDIR", None, "SourceDir")
builder.add_directory("ProgramFilesFolder", "TARGETDIR", "PFiles|Program Files")
builder.add_directory("INSTALLDIR", "ProgramFilesFolder", "AppDir|Custom App")

# Define Component & Feature
builder.add_component(
    comp_id="MainComponent",
    dir_id="INSTALLDIR",
    comp_guid="{22222222-3333-4444-5555-666666666666}",
)

builder.add_feature(
    feat_id="Complete",
    title="Main Feature",
    description="All application files",
    level=1,
)

builder.add_feature_component("Complete", "MainComponent")

# Add Files from Disk
builder.add_file_from_disk(
    source_path="app.exe",
    target_dir_id="INSTALLDIR",
    feature_id="Complete",
    component_id="MainComponent",
)

# Build and write package
builder.build_to_file("CustomApp.msi")
```

---

## 3. Reading and Inspecting `.msi` Files

You can open and inspect any Windows Installer package:

```python
import msi

package = msi.Package.open("CustomApp.msi")

# Query metadata
print(package.get_property("ProductName"))
print(package.get_property("ProductVersion"))

# Inspect tables
print("Tables in package:", package.table_names)
file_rows = package.get_table("File")
for row in file_rows:
    print(row["FileName"], row["FileSize"])

# Extract embedded cabinets
package.extract_cabinet("#cab1.cab", "extracted_files/")
```

---

## 4. Compiling WiX XML (`.wxs`) Directly

Compile WiX v3 and v4 source XML directly to `.msi`:

```python
import msi

wxs_content = """
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="WiXApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="WiX via Python" />
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

msi.compile_wix(wxs_content, "wix_app.msi")
```
