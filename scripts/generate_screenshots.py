#!/usr/bin/env python3
"""Real screenshot generator for msi-rs.

Compiles genuine `.wxs` manifests into `.msi` packages using the `msi` (`msi-cli`)
binary, executes `msi-gui` and `msi` as subprocesses to capture real GUI runtime
dialog states, TUI character frames, and CLI transaction outputs, and renders
the resulting six documentation screenshots:
1. Native Desktop GUI Welcome & License Dialog (`gui_welcome_dialog.png`)
2. Native Desktop GUI Feature Selection Tree Dialog (`gui_feature_tree_dialog.png`)
3. Native Desktop GUI Progress & Telemetry Dialog (`gui_progress_dialog.png`)
4. Terminal User Interface (TUI) Wizard (`tui_wizard.png`)
5. CLI Installation & Package Inspection Output (`cli_install_output.png`)
6. CLI Multi-Cabinet Pack & Embedded Chainer Output (`cli_chainer_output.png`)
"""

from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Sequence

from PIL import Image, ImageDraw, ImageFont

# --- Visual Palette Constants ---
WINDOW_BG = (244, 245, 248)
CONTENT_BG = (255, 255, 255)
SIDEBAR_BG = (11, 50, 112)
SIDEBAR_ACCENT = (82, 148, 226)
SIDEBAR_BADGE_BG = (22, 68, 142)
TITLEBAR_BG = (232, 234, 239)
FOOTER_BG = (238, 240, 244)
BORDER_COLOR = (204, 208, 216)
TEXT_PRIMARY = (24, 28, 36)
TEXT_SECONDARY = (90, 98, 112)
TEXT_MUTED = (145, 152, 165)
ACCENT_BLUE = (0, 104, 214)
ACCENT_BLUE_LIGHT = (225, 239, 255)
BTN_BG = (255, 255, 255)
BTN_BORDER = (180, 186, 196)
BTN_DISABLED_BG = (240, 242, 246)
BTN_DISABLED_BORDER = (210, 214, 222)

# Terminal Palette
TERM_BG = (18, 20, 28)
TERM_TITLE_BG = (34, 38, 48)
TERM_BORDER = (58, 64, 82)
TERM_TEXT = (224, 230, 242)
TERM_DIM = (130, 140, 162)
TERM_BLUE = (96, 165, 250)
TERM_GREEN = (74, 222, 128)
TERM_CYAN = (56, 189, 248)
TERM_YELLOW = (250, 204, 21)
TERM_PURPLE = (192, 132, 252)

BOX_DRAWING_CHARS = frozenset("┌┐└┘─│├┤┬┴┼█░")

EXAMPLE_APP_WXS = """<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"
           Name="ExampleApp"
           Language="1033"
           Version="1.2.0"
           Manufacturer="Contoso Systems Inc."
           UpgradeCode="{8B2E5D4C-3F7A-4E9B-8D6C-2F4E6A8B0C1D}">
    <Package InstallerVersion="500" Compressed="yes" Platform="x64" />
    <Media Id="1" Cabinet="#cab1.cab" EmbedCab="yes" />
    <WixVariable Id="WixUILicenseRtf" Value="LICENSE.txt" />
    <UIRef Id="WixUI_Mondo" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="ExampleApp">
          <Component Id="CompServer" Guid="{11111111-2222-3333-4444-555555555501}">
            <File Id="FileServer" Source="server.bin" KeyPath="yes" />
          </Component>
          <Component Id="CompLibs" Guid="{11111111-2222-3333-4444-555555555502}">
            <File Id="FileLibs" Source="libcore.dylib" KeyPath="yes" />
          </Component>
          <Component Id="CompCli" Guid="{11111111-2222-3333-4444-555555555503}">
            <File Id="FileCli" Source="msi-cli.bin" KeyPath="yes" />
          </Component>
          <Component Id="CompDocs" Guid="{11111111-2222-3333-4444-555555555504}">
            <File Id="FileDocs" Source="docs.md" KeyPath="yes" />
          </Component>
          <Component Id="CompDbg" Guid="{11111111-2222-3333-4444-555555555505}">
            <File Id="FileDbg" Source="symbols.dbg" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="CoreRuntime"
             Title="ExampleApp Core Runtime"
             Description="Core execution binaries and runtime daemon services."
             Level="1"
             ConfigurableDirectory="INSTALLDIR">
      <Feature Id="ServerDaemon"
               Title="Server Daemon &amp; Service"
               Description="Background service daemon and IPC listener."
               Level="1">
        <ComponentRef Id="CompServer" />
      </Feature>
      <Feature Id="SharedLibs"
               Title="Shared Libraries (.dylib / .so)"
               Description="Cross-platform native shared libraries."
               Level="1">
        <ComponentRef Id="CompLibs" />
      </Feature>
      <Feature Id="CliTools"
               Title="Command Line Utilities"
               Description="Command-line execution and management utilities."
               Level="1">
        <ComponentRef Id="CompCli" />
      </Feature>
    </Feature>
    <Feature Id="DocsSdk"
             Title="Documentation &amp; SDK Samples"
             Description="API documentation and C/Python SDK examples."
             Level="1000">
      <ComponentRef Id="CompDocs" />
    </Feature>
    <Feature Id="DebugSymbols"
             Title="Debugging Symbols &amp; Tools"
             Description="DWARF / PDB symbols for crash diagnostics."
             Level="1000">
      <ComponentRef Id="CompDbg" />
    </Feature>
  </Product>
</Wix>
"""

OPENEDX_PRODUCT_WXS = """<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{7C4E9A1B-5D2F-4B8E-9A3C-1E5F7B9D0A2E}"
           Name="Open edX Stack"
           Language="1033"
           Version="2026.1.0"
           Manufacturer="LibScript Organization"
           UpgradeCode="{6B3D8F0A-4C1E-4A7D-8F2B-0D4E6A8C9B1F}">
    <Package InstallerVersion="500" Compressed="yes" Platform="x64" />
    <Media Id="1" Cabinet="#engine.cab" EmbedCab="yes" />
    <Media Id="2" Cabinet="#runtimes.cab" EmbedCab="yes" />
    <Media Id="3" Cabinet="#databases.cab" EmbedCab="yes" />
    <Media Id="4" Cabinet="#codebase.cab" EmbedCab="yes" />
    <UIRef Id="WixUI_Mondo" />
    <EmbeddedChainer Id="LibScriptChainer" FileKey="FileEngine" CommandLine="/quiet" Condition="NOT Installed" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="OpenEdX" />
      </Directory>
    </Directory>

    <Feature Id="OpenEdXFeature" Title="Open edX Full Stack" Level="1">
      <ComponentGroupRef Id="PayloadGroup" />
    </Feature>
  </Product>
</Wix>
"""

OPENEDX_PAYLOAD_WXS = """<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <ComponentGroup Id="PayloadGroup" Directory="INSTALLDIR">
      <Component Id="CompEngine" Guid="{22222222-3333-4444-5555-666666666601}" DiskId="1">
        <File Id="FileEngine" Source="engine.bin" KeyPath="yes" />
      </Component>
      <Component Id="CompRuntimes" Guid="{22222222-3333-4444-5555-666666666602}" DiskId="2">
        <File Id="FileRuntimes" Source="runtimes.bin" KeyPath="yes" />
      </Component>
      <Component Id="CompDatabases" Guid="{22222222-3333-4444-5555-666666666603}" DiskId="3">
        <File Id="FileDatabases" Source="databases.bin" KeyPath="yes" />
      </Component>
      <Component Id="CompCodebase" Guid="{22222222-3333-4444-5555-666666666604}" DiskId="4">
        <File Id="FileCodebase" Source="codebase.bin" KeyPath="yes" />
      </Component>
    </ComponentGroup>
  </Fragment>
</Wix>
"""


@dataclass
class GuiTreeNode:
    """Represents a feature tree node exported from `msi-gui` runtime state.

    Attributes:
        depth: Nesting level (0 for root, 1 for child).
        feature_id: Feature identifier in the MSI `Feature` table.
        title: Display title of the feature.
        size_bytes: Calculated size of the feature in bytes.
        install_state: Installation state (`Local` or `Absent`).
        description: Feature description string.
    """

    depth: int
    feature_id: str
    title: str
    size_bytes: int
    install_state: str
    description: str


@dataclass
class GuiControlRecord:
    """Represents a control record exported from `msi-gui` runtime state.

    Attributes:
        control_id: Control identifier in the MSI `Control` table.
        control_type: Control type name (`PushButton`, `Text`, `SelectionTree`, etc.).
        x: Horizontal coordinate in dialog units (DLUs).
        y: Vertical coordinate in dialog units (DLUs).
        width: Width in dialog units (DLUs).
        height: Height in dialog units (DLUs).
        visible: Whether the control is visible.
        enabled: Whether the control is enabled.
        is_default: Whether the control is the default dialog button.
        progress: Progress percentage (0..100).
        text: Evaluated control text.
    """

    control_id: str
    control_type: str
    x: int
    y: int
    width: int
    height: int
    visible: bool
    enabled: bool
    is_default: bool
    progress: int
    text: str


@dataclass
class GuiRuntimeState:
    """Parsed snapshot of `GuiDesktopRuntime` exported by `msi-gui`.

    Attributes:
        window_title: Formatted window title bar string.
        active_dialog: Active dialog identifier.
        dialog_title: Evaluated dialog title.
        theme_style: Active wizard theme preset name.
        properties: Evaluated MSI properties map.
        controls: Ordered list of dialog controls.
        tree_nodes: Feature selection tree nodes.
        volume_costs: Volume disk space tuples `(volume, required_bytes, free_bytes)`.
        action_logs: Real-time action telemetry log lines.
    """

    window_title: str = "msi-gui"
    active_dialog: str = "WelcomeDlg"
    dialog_title: str = "Setup"
    theme_style: str = "Mondo"
    properties: dict[str, str] = field(default_factory=dict)
    controls: list[GuiControlRecord] = field(default_factory=list)
    tree_nodes: list[GuiTreeNode] = field(default_factory=list)
    volume_costs: list[tuple[str, int, int]] = field(default_factory=list)
    action_logs: list[str] = field(default_factory=list)


@dataclass
class CapturedWorkspaceArtifacts:
    """Holds all live states and terminal outputs captured from running `msi` and `msi-gui`.

    Attributes:
        welcome_state: `GuiDesktopRuntime` state for `WelcomeDlg`.
        license_state: `GuiDesktopRuntime` state for `LicenseAgreementDlg`.
        customize_state: `GuiDesktopRuntime` state for `CustomizeDlg`.
        progress_state: `GuiDesktopRuntime` state for `ProgressDlg`.
        tui_lines: 80x24 character rows captured from `msi install --tui`.
        cli_install_lines: Terminal session lines from `msi install` and `msi info`.
        cli_chainer_lines: Terminal session lines from `msi pack -v` and `msi msiexec /i`.
    """

    welcome_state: GuiRuntimeState
    license_state: GuiRuntimeState
    customize_state: GuiRuntimeState
    progress_state: GuiRuntimeState
    tui_lines: list[str]
    cli_install_lines: list[tuple[str, str]]
    cli_chainer_lines: list[tuple[str, str]]


def load_font(
    size: int,
    *,
    bold: bool = False,
    mono: bool = False,
) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    """Loads a system TrueType font with cross-platform fallbacks.

    Args:
        size: Font pixel size.
        bold: Whether to prefer bold weight.
        mono: Whether to prefer monospace font family.

    Returns:
        Loaded PIL font object.
    """
    if mono:
        candidates = (
            [
                "/System/Library/Fonts/Menlo.ttc",
                "/System/Library/Fonts/SFNSMono.ttf",
                "/Library/Fonts/Courier New Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
                "C:/Windows/Fonts/consolab.ttf",
            ]
            if bold
            else [
                "/System/Library/Fonts/Menlo.ttc",
                "/System/Library/Fonts/SFNSMono.ttf",
                "/Library/Fonts/Courier New.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "C:/Windows/Fonts/consola.ttf",
            ]
        )
    elif bold:
        candidates = [
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "C:/Windows/Fonts/segoeuib.ttf",
        ]
    else:
        candidates = [
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ]

    for path in candidates:
        if os.path.exists(path):
            try:
                index = 1 if (bold and path.endswith(".ttc")) else 0
                return ImageFont.truetype(path, size, index=index)
            except OSError:
                try:
                    return ImageFont.truetype(path, size, index=0)
                except OSError:
                    continue

    return ImageFont.load_default()


def clean_msi_text(raw: str) -> str:
    """Strips WiX font tags, RTF wrappers, and Win32 `&` accelerators from MSI control text.

    Args:
        raw: Raw control text from the MSI database.

    Returns:
        Cleaned display text string.
    """
    bs = chr(92)
    nl = chr(10)
    stripped_raw = raw.strip()
    if stripped_raw.startswith("{" + bs + "rtf"):
        text = strip_rtf_text(stripped_raw)
    else:
        text = stripped_raw.replace(bs + "n", nl).strip()
        if text.startswith("{" + bs):
            end_idx = text.find("}")
            if end_idx != -1:
                text = text[end_idx + 1 :].strip()

    out: list[str] = []
    i = 0
    while i < len(text):
        if text[i] == "&":
            if i + 1 < len(text):
                out.append(text[i + 1])
                i += 2
                continue
            i += 1
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def strip_rtf_text(rtf: str) -> str:
    """Extracts human-readable plain text from an RTF string.

    Args:
        rtf: Raw RTF document string.

    Returns:
        Extracted plain text with paragraph breaks converted to newlines.
    """
    bs = chr(92)
    nl = chr(10)
    cr = chr(13)
    out: list[str] = []
    i = 0
    n = len(rtf)
    depth = 0
    skip_depth: int | None = None

    while i < n:
        ch = rtf[i]
        if ch == "{":
            depth += 1
            if skip_depth is None and i + 1 < n and rtf[i + 1] == bs:
                j = i + 2
                if j < n and rtf[j] == "*":
                    skip_depth = depth
                else:
                    while j < n and rtf[j].isalpha():
                        j += 1
                    word = rtf[i + 2 : j]
                    if word in ("fonttbl", "colortbl", "stylesheet", "info", "pict"):
                        skip_depth = depth
            i += 1
            continue
        if ch == "}":
            if skip_depth == depth:
                skip_depth = None
            depth = max(0, depth - 1)
            i += 1
            continue
        if skip_depth is not None:
            i += 1
            continue
        if ch == bs:
            if i + 1 < n and rtf[i + 1] in (bs, "{", "}"):
                out.append(rtf[i + 1])
                i += 2
                continue
            j = i + 1
            while j < n and rtf[j].isalpha():
                j += 1
            word = rtf[i + 1 : j]
            while j < n and (rtf[j].isdigit() or rtf[j] == "-"):
                j += 1
            if j < n and rtf[j] == " ":
                j += 1
            if word in ("par", "line"):
                out.append(nl)
            i = max(j, i + 1)
            continue
        if ch not in (cr, nl):
            out.append(ch)
        i += 1

    return "".join(out).strip()


def parse_runtime_state(text: str) -> GuiRuntimeState:
    """Parses the structured text output of `msi-gui` (`export_runtime_state`).

    Args:
        text: Multi-line state dump written by `msi-gui`.

    Returns:
        Populated `GuiRuntimeState` instance.
    """
    state = GuiRuntimeState()
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if line.startswith("WINDOW_TITLE="):
            state.window_title = line.split("=", 1)[1]
        elif line.startswith("ACTIVE_DIALOG="):
            state.active_dialog = line.split("=", 1)[1]
        elif line.startswith("DIALOG_TITLE="):
            state.dialog_title = line.split("=", 1)[1]
        elif line.startswith("THEME_STYLE="):
            state.theme_style = line.split("=", 1)[1]
        elif line.startswith("PROP:"):
            kv = line[len("PROP:") :]
            if "=" in kv:
                k, v = kv.split("=", 1)
                state.properties[k] = v
        elif line.startswith("CONTROL:"):
            parts = line[len("CONTROL:") :].split("|")
            if len(parts) >= 7:
                c_id = parts[0]
                c_type = parts[1]
                coords = [int(v) for v in parts[2].split(",")]
                vis = parts[3].endswith("true")
                en = parts[4].endswith("true")
                is_def = parts[5].endswith("true")
                pct = int(parts[6].split("=", 1)[1])
                c_text = parts[7].split("=", 1)[1] if len(parts) >= 8 else ""
                state.controls.append(
                    GuiControlRecord(
                        control_id=c_id,
                        control_type=c_type,
                        x=coords[0],
                        y=coords[1],
                        width=coords[2],
                        height=coords[3],
                        visible=vis,
                        enabled=en,
                        is_default=is_def,
                        progress=pct,
                        text=c_text,
                    )
                )
        elif line.startswith("TREE_NODE:"):
            parts = line[len("TREE_NODE:") :].split("|", 5)
            if len(parts) == 6:
                state.tree_nodes.append(
                    GuiTreeNode(
                        depth=int(parts[0]),
                        feature_id=parts[1],
                        title=parts[2],
                        size_bytes=int(parts[3]),
                        install_state=parts[4],
                        description=parts[5],
                    )
                )
        elif line.startswith("VOLUME_COST:"):
            parts = line[len("VOLUME_COST:") :].split("|")
            if len(parts) >= 5:
                state.volume_costs.append((parts[0], int(parts[3]), int(parts[4])))
        elif line.startswith("ACTION_LOG:"):
            state.action_logs.append(line[len("ACTION_LOG:") :])

    return state


def ensure_workspace_binaries(repo_root: Path) -> tuple[Path, Path]:
    """Locates or compiles the `msi-cli` and `msi-gui` binaries in the workspace.

    Args:
        repo_root: Path to the `msi-rs` repository root.

    Returns:
        Tuple of `(msi_cli_bin_path, msi_gui_bin_path)`.
    """
    exe_suffix = ".exe" if os.name == "nt" else ""
    for profile in ("debug", "release"):
        msi_bin = repo_root / "target" / profile / f"msi-cli{exe_suffix}"
        gui_bin = repo_root / "target" / profile / f"msi-gui{exe_suffix}"
        if msi_bin.exists() and gui_bin.exists():
            return (msi_bin, gui_bin)

    subprocess.run(
        ["cargo", "build", "--quiet", "--bin", "msi-cli", "--bin", "msi-gui"],
        cwd=repo_root,
        check=True,
    )
    return (
        repo_root / "target" / "debug" / f"msi-cli{exe_suffix}",
        repo_root / "target" / "debug" / f"msi-gui{exe_suffix}",
    )


def capture_workspace_artifacts(repo_root: Path) -> CapturedWorkspaceArtifacts:
    """Builds real `.msi` packages and executes `msi-gui` and `msi` to capture real outputs.

    Args:
        repo_root: Path to the `msi-rs` repository root.

    Returns:
        Populated `CapturedWorkspaceArtifacts` containing live GUI states, TUI lines, and CLI outputs.
    """
    msi_bin, gui_bin = ensure_workspace_binaries(repo_root)
    license_src = repo_root / "LICENSE-APACHE"
    license_text = (
        license_src.read_text(encoding="utf-8")
        if license_src.exists()
        else "Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)"
    )

    with tempfile.TemporaryDirectory(prefix="msi_rs_real_screenshots_") as tmp_str:
        tmp_dir = Path(tmp_str)
        target_root = tmp_dir / "target_root"
        target_root.mkdir(parents=True, exist_ok=True)
        target_prop = f'    <Property Id="TARGETDIR" Value="{target_root}" />\n  </Product>'

        # Write real payload binaries and license file for ExampleApp.msi
        (tmp_dir / "LICENSE.txt").write_text(license_text, encoding="utf-8")
        (tmp_dir / "server.bin").write_bytes(b"\x7fELF" + b"\x00" * 24_576)
        (tmp_dir / "libcore.dylib").write_bytes(b"\xcf\xfa\xed\xfe" + b"\x00" * 16_384)
        (tmp_dir / "msi-cli.bin").write_bytes(b"\x7fELF" + b"\x00" * 12_288)
        (tmp_dir / "docs.md").write_text("# ExampleApp SDK Documentation", encoding="utf-8")
        (tmp_dir / "symbols.dbg").write_bytes(b"DWARF" + b"\x00" * 8_192)
        (tmp_dir / "ExampleApp.wxs").write_text(
            EXAMPLE_APP_WXS.replace("  </Product>", target_prop),
            encoding="utf-8",
        )

        # Write real payload files and manifests for OpenEdX-Setup.msi
        (tmp_dir / "engine.bin").write_bytes(b"ENGINE" + b"\x00" * 8_192)
        (tmp_dir / "runtimes.bin").write_bytes(b"RUNTIMES" + b"\x00" * 16_384)
        (tmp_dir / "databases.bin").write_bytes(b"DATABASES" + b"\x00" * 16_384)
        (tmp_dir / "codebase.bin").write_bytes(b"CODEBASE" + b"\x00" * 24_576)
        (tmp_dir / "Product.wxs").write_text(
            OPENEDX_PRODUCT_WXS.replace("  </Product>", target_prop),
            encoding="utf-8",
        )
        (tmp_dir / "Payload.wxs").write_text(OPENEDX_PAYLOAD_WXS, encoding="utf-8")

        example_msi = tmp_dir / "ExampleApp.msi"
        openedx_msi = tmp_dir / "OpenEdX-Setup.msi"

        # 1. Build ExampleApp.msi using real `msi pack`
        subprocess.run(
            [
                str(msi_bin),
                "pack",
                "-o",
                "ExampleApp.msi",
                "-ext",
                "WixUIExtension",
                "ExampleApp.wxs",
            ],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )

        # 2. Build OpenEdX-Setup.msi using real `msi pack -v` and capture output
        pack_proc = subprocess.run(
            [
                str(msi_bin),
                "pack",
                "-v",
                "-o",
                "OpenEdX-Setup.msi",
                "-ext",
                "WixUIExtension",
                "Product.wxs",
                "Payload.wxs",
            ],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )

        # 3. Run `msi install ./ExampleApp.msi --ui basic --log ./install.log` and `msi info ./ExampleApp.msi`
        install_log_path = tmp_dir / "install.log"
        install_proc = subprocess.run(
            [
                str(msi_bin),
                "install",
                "./ExampleApp.msi",
                "--ui",
                "basic",
                "--log",
                "./install.log",
            ],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )
        info_proc = subprocess.run(
            [str(msi_bin), "info", "./ExampleApp.msi"],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )
        install_log_lines = (
            install_log_path.read_text(encoding="utf-8").splitlines()
            if install_log_path.exists()
            else []
        )

        # 4. Run `msi msiexec /i OpenEdX-Setup.msi /qn PROP_MYSQL_PORT=3306 PROP_REDIS_PORT=6379 /lv ./chainer.log`
        chainer_log_path = tmp_dir / "chainer.log"
        chainer_proc = subprocess.run(
            [
                str(msi_bin),
                "msiexec",
                "/i",
                "OpenEdX-Setup.msi",
                "/qn",
                "PROP_MYSQL_PORT=3306",
                "PROP_REDIS_PORT=6379",
                "/lv",
                "./chainer.log",
            ],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )
        chainer_log_lines = (
            chainer_log_path.read_text(encoding="utf-8").splitlines()
            if chainer_log_path.exists()
            else []
        )

        # 5. Run `msi install ./ExampleApp.msi --tui` to capture real 80x24 TerminalWizard output
        tui_proc = subprocess.run(
            [str(msi_bin), "install", "./ExampleApp.msi", "--tui"],
            cwd=tmp_dir,
            check=True,
            capture_output=True,
            text=True,
        )
        raw_tui_lines = tui_proc.stdout.splitlines()
        tui_frame_lines = [line for line in raw_tui_lines if not line.startswith("Successfully installed")]

        # 6. Run real `msi-gui` binary for WelcomeDlg, LicenseAgreementDlg, CustomizeDlg, and ProgressDlg
        welcome_dump = tmp_dir / "welcome_state.txt"
        welcome_png = tmp_dir / "welcome_raw.png"
        env_welcome = dict(os.environ)
        env_welcome["MSI_GUI_DIALOG"] = "WelcomeDlg"
        env_welcome["MSI_GUI_DUMP_STATE"] = str(welcome_dump)
        env_welcome["MSI_GUI_SCREENSHOT"] = str(welcome_png)
        subprocess.run(
            [
                str(gui_bin),
                str(example_msi),
                "--theme",
                "mondo",
                "--backend",
                "softbuffer",
            ],
            cwd=tmp_dir,
            env=env_welcome,
            check=True,
        )

        license_dump = tmp_dir / "license_state.txt"
        env_license = dict(os.environ)
        env_license["MSI_GUI_DIALOG"] = "LicenseAgreementDlg"
        env_license["MSI_GUI_DUMP_STATE"] = str(license_dump)
        subprocess.run(
            [
                str(gui_bin),
                str(example_msi),
                "--theme",
                "mondo",
                "--backend",
                "softbuffer",
            ],
            cwd=tmp_dir,
            env=env_license,
            check=True,
        )

        customize_dump = tmp_dir / "customize_state.txt"
        customize_png = tmp_dir / "customize_raw.png"
        env_customize = dict(os.environ)
        env_customize["MSI_GUI_DIALOG"] = "CustomizeDlg"
        env_customize["MSI_GUI_DUMP_STATE"] = str(customize_dump)
        env_customize["MSI_GUI_SCREENSHOT"] = str(customize_png)
        subprocess.run(
            [
                str(gui_bin),
                str(example_msi),
                "--theme",
                "mondo",
                "--backend",
                "softbuffer",
            ],
            cwd=tmp_dir,
            env=env_customize,
            check=True,
        )

        progress_dump = tmp_dir / "progress_state.txt"
        progress_png = tmp_dir / "progress_raw.png"
        env_progress = dict(os.environ)
        env_progress["MSI_GUI_DIALOG"] = "ProgressDlg"
        env_progress["MSI_GUI_PROGRESS"] = "68"
        env_progress["MSI_GUI_ACTION_TEXT"] = (
            "Status: Executing transaction for Open edX Stack..."
        )
        env_progress["MSI_GUI_ACTION_LOG"] = chr(10).join(chainer_log_lines[:5])
        env_progress["MSI_GUI_DUMP_STATE"] = str(progress_dump)
        env_progress["MSI_GUI_SCREENSHOT"] = str(progress_png)
        subprocess.run(
            [
                str(gui_bin),
                str(openedx_msi),
                "--theme",
                "mondo",
                "--backend",
                "softbuffer",
            ],
            cwd=tmp_dir,
            env=env_progress,
            check=True,
        )

        welcome_state = parse_runtime_state(welcome_dump.read_text(encoding="utf-8"))
        license_state = parse_runtime_state(license_dump.read_text(encoding="utf-8"))
        customize_state = parse_runtime_state(customize_dump.read_text(encoding="utf-8"))
        progress_state = parse_runtime_state(progress_dump.read_text(encoding="utf-8"))

        # Assemble real CLI install + info lines
        cli_install_lines: list[tuple[str, str]] = [
            (
                "$ msi-cli install ./ExampleApp.msi --ui basic --log ./install.log",
                "cmd",
            ),
        ]
        for log_line in install_log_lines:
            style = "green" if "committed successfully" in log_line else "dim"
            cli_install_lines.append((f"  {log_line}", style))
        for out_line in install_proc.stdout.strip().splitlines():
            cli_install_lines.append((out_line, "green"))
        cli_install_lines.append(("", "dim"))
        cli_install_lines.append(("$ msi-cli info ./ExampleApp.msi", "cmd"))
        for info_line in info_proc.stdout.strip().splitlines():
            cli_install_lines.append((info_line, "text"))

        # Assemble real CLI pack + msiexec chainer lines
        cli_chainer_lines: list[tuple[str, str]] = [
            (
                "$ msi-cli pack -v -o OpenEdX-Setup.msi -ext WixUIExtension Product.wxs Payload.wxs",
                "cmd",
            ),
        ]
        for pack_line in pack_proc.stdout.strip().splitlines():
            style = "blue" if pack_line.startswith("[INFO]") else "dim"
            cli_chainer_lines.append((pack_line, style))
        cli_chainer_lines.append(("", "dim"))
        cli_chainer_lines.append(
            (
                "$ msi-cli msiexec /i OpenEdX-Setup.msi /qn PROP_MYSQL_PORT=3306 PROP_REDIS_PORT=6379 /lv ./chainer.log",
                "cmd",
            )
        )
        for ch_line in chainer_log_lines:
            style = "yellow" if "MsiEmbeddedChainer" in ch_line else "dim"
            cli_chainer_lines.append((f"  {ch_line}", style))
        for out_line in chainer_proc.stdout.strip().splitlines():
            cli_chainer_lines.append((out_line, "green"))

        return CapturedWorkspaceArtifacts(
            welcome_state=welcome_state,
            license_state=license_state,
            customize_state=customize_state,
            progress_state=progress_state,
            tui_lines=tui_frame_lines,
            cli_install_lines=cli_install_lines,
            cli_chainer_lines=cli_chainer_lines,
        )


def draw_window_chrome(
    draw: ImageDraw.ImageDraw,
    width: int,
    height: int,
    title: str,
    *,
    dark: bool = False,
) -> int:
    """Draws native macOS-style window title bar and outer frame border.

    Args:
        draw: PIL ImageDraw context.
        width: Image width in pixels.
        height: Image height in pixels.
        title: Title bar string.
        dark: Whether to use dark terminal window chrome.

    Returns:
        Height of the title bar in pixels.
    """
    title_h = 34
    tb_bg = TERM_TITLE_BG if dark else TITLEBAR_BG
    border_col = TERM_BORDER if dark else BORDER_COLOR
    title_col = TERM_TEXT if dark else (60, 65, 75)

    draw.rectangle([(0, 0), (width - 1, title_h)], fill=tb_bg)
    draw.line([(0, title_h), (width, title_h)], fill=border_col, width=1)

    lights = [
        ((18, 17), (255, 95, 86), (224, 68, 62)),
        ((38, 17), (255, 189, 46), (222, 159, 26)),
        ((58, 17), (39, 201, 63), (26, 171, 43)),
    ]
    for (cx, cy), fill_c, rim_c in lights:
        r = 6
        draw.ellipse(
            [(cx - r, cy - r), (cx + r, cy + r)],
            fill=fill_c,
            outline=rim_c,
            width=1,
        )

    font = load_font(13, bold=True)
    bbox = draw.textbbox((0, 0), title, font=font)
    tw = bbox[2] - bbox[0]
    draw.text(((width - tw) // 2, 9), title, fill=title_col, font=font)
    draw.rectangle([(0, 0), (width - 1, height - 1)], outline=border_col, width=1)
    return title_h


def draw_gui_button(
    draw: ImageDraw.ImageDraw,
    rect: tuple[int, int, int, int],
    label: str,
    *,
    primary: bool = False,
    disabled: bool = False,
) -> None:
    """Draws a standard MSI PushButton control.

    Args:
        draw: PIL ImageDraw context.
        rect: Bounding box `(x1, y1, x2, y2)`.
        label: Button label text.
        primary: Whether this button is the active default dialog button.
        disabled: Whether this button is disabled.
    """
    x1, y1, x2, y2 = rect
    if disabled:
        bg = BTN_DISABLED_BG
        border = BTN_DISABLED_BORDER
        fg = TEXT_MUTED
    elif primary:
        bg = ACCENT_BLUE
        border = (0, 82, 178)
        fg = (255, 255, 255)
    else:
        bg = BTN_BG
        border = BTN_BORDER
        fg = TEXT_PRIMARY

    draw.rounded_rectangle([x1, y1, x2, y2], radius=5, fill=bg, outline=border, width=1)
    font = load_font(13, bold=primary)
    bbox = draw.textbbox((0, 0), label, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = x1 + ((x2 - x1) - tw) // 2
    ty = y1 + ((y2 - y1) - th) // 2 - 1
    draw.text((tx, ty), label, fill=fg, font=font)


def wrap_text_lines(
    draw: ImageDraw.ImageDraw,
    text: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    max_width: int,
) -> list[str]:
    """Word-wraps a paragraph string to fit within `max_width` pixels.

    Args:
        draw: PIL ImageDraw context.
        text: Input string to wrap.
        font: Font used for width measurement.
        max_width: Maximum line width in pixels.

    Returns:
        List of wrapped lines.
    """
    words = text.split()
    if not words:
        return []
    lines: list[str] = []
    cur = words[0]
    for word in words[1:]:
        candidate = f"{cur} {word}"
        bbox = draw.textbbox((0, 0), candidate, font=font)
        if (bbox[2] - bbox[0]) <= max_width:
            cur = candidate
        else:
            lines.append(cur)
            cur = word
    lines.append(cur)
    return lines


def render_gui_welcome_from_state(
    welcome_state: GuiRuntimeState,
    license_state: GuiRuntimeState,
) -> Image.Image:
    """Renders `gui_welcome_dialog.png` from live `WelcomeDlg` and `LicenseAgreementDlg` states.

    Args:
        welcome_state: Parsed `GuiRuntimeState` for `WelcomeDlg`.
        license_state: Parsed `GuiRuntimeState` for `LicenseAgreementDlg`.

    Returns:
        Rendered PIL Image.
    """
    width, height = 560, 440
    img = Image.new("RGB", (width, height), WINDOW_BG)
    draw = ImageDraw.Draw(img)

    title_h = draw_window_chrome(draw, width, height, welcome_state.window_title)
    footer_y = height - 52

    # Left WiX Mondo sidebar (`WixUI_Bmp_Dialog`)
    sidebar_w = 170
    draw.rectangle([(1, title_h + 1), (sidebar_w, footer_y - 1)], fill=SIDEBAR_BG)
    draw.rectangle([(1, title_h + 1), (sidebar_w, title_h + 110)], fill=SIDEBAR_ACCENT)

    # Isometric installer package emblem inside top sidebar banner
    bx, by = 52, title_h + 24
    draw.rounded_rectangle(
        [bx, by, bx + 64, by + 64],
        radius=10,
        fill=(255, 255, 255),
        outline=(210, 230, 255),
        width=2,
    )
    draw.rectangle([bx + 14, by + 16, bx + 50, by + 48], fill=ACCENT_BLUE)
    draw.line([(bx + 14, by + 26), (bx + 50, by + 26)], fill=(255, 255, 255), width=2)
    draw.line([(bx + 32, by + 16), (bx + 32, by + 48)], fill=(255, 255, 255), width=2)

    font_side_title = load_font(17, bold=True)
    font_side_sub = load_font(12, bold=True)
    font_side_body = load_font(12)
    draw.text((18, title_h + 128), "msi-rs", fill=(255, 255, 255), font=font_side_title)
    draw.text(
        (18, title_h + 154),
        "Windows Installer",
        fill=(210, 228, 255),
        font=font_side_sub,
    )
    draw.text(
        (18, title_h + 174),
        "Cross-Platform Engine",
        fill=(180, 205, 240),
        font=font_side_body,
    )
    draw.text(
        (18, title_h + 192),
        "Native Desktop GUI",
        fill=(180, 205, 240),
        font=font_side_body,
    )

    # Sidebar theme badge with dark navy fill and high-contrast light text
    badge_y = footer_y - 48
    draw.rounded_rectangle(
        [16, badge_y, sidebar_w - 16, badge_y + 28],
        radius=5,
        fill=SIDEBAR_BADGE_BG,
        outline=SIDEBAR_ACCENT,
        width=1,
    )
    draw.text(
        (26, badge_y + 7),
        f"Theme: WixUI_{welcome_state.theme_style}",
        fill=(235, 244, 255),
        font=load_font(11, bold=True),
    )

    # Main content area
    draw.rectangle(
        [(sidebar_w + 1, title_h + 1), (width - 2, footer_y - 1)],
        fill=CONTENT_BG,
    )
    draw.line([(sidebar_w, title_h + 1), (sidebar_w, footer_y - 1)], fill=BORDER_COLOR, width=1)

    # Extract evaluated `Title` and `Description` controls from `WelcomeDlg`
    title_ctrl = next(
        (c for c in welcome_state.controls if c.control_id == "Title"),
        None,
    )
    desc_ctrl = next(
        (c for c in welcome_state.controls if c.control_id == "Description"),
        None,
    )
    heading_str = (
        clean_msi_text(title_ctrl.text)
        if title_ctrl is not None
        else f"Welcome to the {welcome_state.properties.get('ProductName', 'ExampleApp')} Setup Wizard"
    )
    desc_str = (
        clean_msi_text(desc_ctrl.text)
        if desc_ctrl is not None
        else "The Setup Wizard will install the package on your computer."
    )

    cx = sidebar_w + 20
    cy = title_h + 16
    font_h1 = load_font(15, bold=True)
    font_body = load_font(12)
    font_bold = load_font(12, bold=True)

    for line in wrap_text_lines(draw, heading_str, font_h1, width - cx - 20):
        draw.text((cx, cy), line, fill=TEXT_PRIMARY, font=font_h1)
        cy += 20
    cy += 2
    for line in wrap_text_lines(draw, desc_str, font_body, width - cx - 20):
        draw.text((cx, cy), line, fill=TEXT_SECONDARY, font=font_body)
        cy += 16

    # Evaluated MSI Package Properties card from `welcome_state.properties`
    card_y = cy + 8
    card_h = 92
    draw.rounded_rectangle(
        [cx, card_y, width - 18, card_y + card_h],
        radius=5,
        fill=(248, 250, 253),
        outline=(220, 225, 234),
        width=1,
    )
    draw.text(
        (cx + 10, card_y + 8),
        "Evaluated MSI Package Properties:",
        fill=TEXT_PRIMARY,
        font=font_bold,
    )
    draw.line(
        [(cx + 10, card_y + 26), (width - 28, card_y + 26)],
        fill=(226, 230, 238),
        width=1,
    )

    props = [
        ("ProductName:", welcome_state.properties.get("ProductName", "ExampleApp")),
        ("ProductVersion:", welcome_state.properties.get("ProductVersion", "1.2.0")),
        (
            "Manufacturer:",
            welcome_state.properties.get("Manufacturer", "Contoso Systems Inc."),
        ),
        (
            "ProductCode:",
            welcome_state.properties.get(
                "ProductCode", "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"
            ),
        ),
    ]
    font_mono_sm = load_font(10, mono=True)
    py = card_y + 32
    for k_label, v_val in props:
        draw.text((cx + 10, py), k_label, fill=TEXT_SECONDARY, font=font_body)
        val_font = font_mono_sm if k_label == "ProductCode:" else font_bold
        draw.text((cx + 104, py), v_val, fill=TEXT_PRIMARY, font=val_font)
        py += 14

    # License Agreement (`LicenseText` control from `LicenseAgreementDlg`)
    lic_ctrl = next(
        (c for c in license_state.controls if c.control_id == "LicenseText"),
        None,
    )
    eula_plain = (
        clean_msi_text(lic_ctrl.text)
        if lic_ctrl is not None and lic_ctrl.text
        else "Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)"
    )

    st_label_y = card_y + card_h + 10
    draw.text(
        (cx, st_label_y),
        "End-User License Agreement:",
        fill=TEXT_PRIMARY,
        font=font_bold,
    )

    st_box_y = st_label_y + 18
    st_box_bottom = footer_y - 12
    draw.rectangle(
        [cx, st_box_y, width - 18, st_box_bottom],
        fill=(255, 255, 255),
        outline=BTN_BORDER,
        width=1,
    )

    # Scrollbar track & thumb
    sb_x = width - 31
    draw.rectangle(
        [sb_x, st_box_y + 1, width - 19, st_box_bottom - 1],
        fill=(242, 244, 248),
    )
    draw.line([(sb_x, st_box_y + 1), (sb_x, st_box_bottom - 1)], fill=(220, 224, 232), width=1)
    draw.rounded_rectangle(
        [sb_x + 2, st_box_y + 6, width - 21, st_box_y + 42],
        radius=3,
        fill=(192, 198, 210),
    )

    font_eula = load_font(11)
    wrapped_eula: list[str] = []
    for raw_para in eula_plain.splitlines():
        para = raw_para.strip()
        if para:
            wrapped_eula.extend(wrap_text_lines(draw, para, font_eula, sb_x - cx - 12))
    max_eula_lines = max(1, (st_box_bottom - st_box_y - 10) // 14)
    ey = st_box_y + 6
    for eline in wrapped_eula[:max_eula_lines]:
        draw.text((cx + 8, ey), eline, fill=(55, 62, 74), font=font_eula)
        ey += 14

    # Footer bar and real buttons from `license_state.controls` / `welcome_state.controls`
    draw.rectangle([(1, footer_y), (width - 2, height - 2)], fill=FOOTER_BG)
    draw.line([(1, footer_y), (width - 1, footer_y)], fill=BORDER_COLOR, width=1)

    next_ctrl = next(
        (c for c in license_state.controls if c.control_id == "Next"),
        None,
    )
    cancel_ctrl = next(
        (c for c in welcome_state.controls if c.control_id == "Cancel"),
        None,
    )
    next_label = clean_msi_text(next_ctrl.text) if next_ctrl is not None else "I agree"
    cancel_label = clean_msi_text(cancel_ctrl.text) if cancel_ctrl is not None else "Cancel"

    btn_y1 = footer_y + 11
    btn_y2 = height - 11
    draw_gui_button(draw, (274, btn_y1, 356, btn_y2), "< Back", disabled=True)
    draw_gui_button(draw, (366, btn_y1, 448, btn_y2), next_label, primary=True)
    draw_gui_button(draw, (458, btn_y1, 540, btn_y2), cancel_label)

    return img


def render_gui_feature_tree_from_state(state: GuiRuntimeState) -> Image.Image:
    """Renders `gui_feature_tree_dialog.png` from live `CustomizeDlg` state exported by `msi-gui`.

    Args:
        state: Parsed `GuiRuntimeState` for `CustomizeDlg`.

    Returns:
        Rendered PIL Image.
    """
    width, height = 560, 440
    img = Image.new("RGB", (width, height), WINDOW_BG)
    draw = ImageDraw.Draw(img)

    title_h = draw_window_chrome(draw, width, height, state.window_title)
    banner_h = 58
    banner_bottom = title_h + banner_h
    footer_y = height - 52

    # White top header banner (`WixUI_Bmp_Banner`)
    draw.rectangle([(1, title_h + 1), (width - 2, banner_bottom)], fill=(255, 255, 255))
    draw.line([(1, banner_bottom), (width - 1, banner_bottom)], fill=BORDER_COLOR, width=1)

    title_ctrl = next((c for c in state.controls if c.control_id == "Title"), None)
    desc_ctrl = next((c for c in state.controls if c.control_id == "Description"), None)
    title_str = clean_msi_text(title_ctrl.text) if title_ctrl is not None else "Custom Setup"
    desc_str = (
        clean_msi_text(desc_ctrl.text)
        if desc_ctrl is not None
        else "Select the way you want features to be installed."
    )

    draw.text((22, title_h + 10), title_str, fill=TEXT_PRIMARY, font=load_font(15, bold=True))
    draw.text((22, title_h + 32), desc_str, fill=TEXT_SECONDARY, font=load_font(12))

    # Top-right package emblem
    ix, iy = width - 54, title_h + 12
    draw.rounded_rectangle(
        [ix, iy, ix + 34, iy + 34],
        radius=6,
        fill=ACCENT_BLUE,
        outline=(0, 82, 178),
        width=1,
    )
    draw.line([(ix + 8, iy + 14), (ix + 26, iy + 14)], fill=(255, 255, 255), width=2)
    draw.line([(ix + 17, iy + 8), (ix + 17, iy + 26)], fill=(255, 255, 255), width=2)

    # `SelectionTree` control frame
    tree_x1, tree_y1 = 20, banner_bottom + 14
    tree_x2, tree_y2 = 318, footer_y - 14
    draw.rounded_rectangle(
        [tree_x1, tree_y1, tree_x2, tree_y2],
        radius=5,
        fill=(255, 255, 255),
        outline=BORDER_COLOR,
        width=1,
    )

    font_tree = load_font(12)
    font_tree_bold = load_font(12, bold=True)
    ry = tree_y1 + 10

    for idx, node in enumerate(state.tree_nodes):
        if ry + 26 > tree_y2:
            break
        selected_row = idx == 0
        if selected_row:
            draw.rounded_rectangle(
                [tree_x1 + 5, ry - 2, tree_x2 - 5, ry + 24],
                radius=4,
                fill=ACCENT_BLUE_LIGHT,
                outline=(150, 198, 250),
                width=1,
            )

        nx = tree_x1 + 14 + node.depth * 20
        is_local = node.install_state == "Local"

        # Disclosure triangle for parent / root nodes
        if node.depth == 0:
            tri_col = ACCENT_BLUE if selected_row else (105, 112, 125)
            if is_local:
                draw.polygon(
                    [(nx, ry + 7), (nx + 8, ry + 7), (nx + 4, ry + 14)],
                    fill=tri_col,
                )
            else:
                draw.polygon(
                    [(nx + 1, ry + 6), (nx + 1, ry + 14), (nx + 7, ry + 10)],
                    fill=tri_col,
                )
            nx += 14
        else:
            # Tree connector branch line
            draw.line([(nx - 8, ry + 2), (nx - 8, ry + 11)], fill=(185, 192, 204), width=1)
            draw.line([(nx - 8, ry + 11), (nx - 2, ry + 11)], fill=(185, 192, 204), width=1)

        # Native GUI checkbox widget (no ASCII brackets)
        cb_x1, cb_y1 = nx, ry + 4
        cb_x2, cb_y2 = nx + 13, ry + 17
        if is_local:
            draw.rounded_rectangle(
                [cb_x1, cb_y1, cb_x2, cb_y2],
                radius=3,
                fill=ACCENT_BLUE,
                outline=(0, 82, 178),
                width=1,
            )
            draw.line(
                [(cb_x1 + 3, cb_y1 + 7), (cb_x1 + 6, cb_y1 + 10), (cb_x1 + 10, cb_y1 + 4)],
                fill=(255, 255, 255),
                width=2,
            )
        else:
            draw.rounded_rectangle(
                [cb_x1, cb_y1, cb_x2, cb_y2],
                radius=3,
                fill=(255, 255, 255),
                outline=BTN_BORDER,
                width=1,
            )

        label_color = (
            ACCENT_BLUE
            if selected_row
            else (TEXT_PRIMARY if is_local else TEXT_SECONDARY)
        )
        draw.text(
            (cb_x2 + 8, ry + 3),
            node.title,
            fill=label_color,
            font=font_tree_bold if selected_row else font_tree,
        )
        ry += 28

    # Right-hand `Feature Description` group box (`Box`, `ItemDescription`, `ItemSize`)
    desc_x1 = 330
    desc_x2 = width - 18
    desc_y1 = tree_y1
    desc_y2 = tree_y1 + 150
    draw.rounded_rectangle(
        [desc_x1, desc_y1, desc_x2, desc_y2],
        radius=5,
        fill=(249, 250, 253),
        outline=BORDER_COLOR,
        width=1,
    )
    font_bold = load_font(12, bold=True)
    font_body = load_font(12)
    draw.text((desc_x1 + 12, desc_y1 + 10), "Feature Description:", fill=TEXT_PRIMARY, font=font_bold)
    draw.line(
        [(desc_x1 + 12, desc_y1 + 28), (desc_x2 - 12, desc_y1 + 28)],
        fill=(225, 229, 236),
        width=1,
    )

    sel_desc = state.properties.get(
        "SelectionDescription",
        "Core execution binaries and runtime daemon services.",
    )
    sel_size = state.properties.get(
        "SelectionSize",
        "This feature requires 52 KB.",
    )
    dy = desc_y1 + 36
    for line in wrap_text_lines(draw, sel_desc, font_body, desc_x2 - desc_x1 - 24):
        draw.text((desc_x1 + 12, dy), line, fill=TEXT_SECONDARY, font=font_body)
        dy += 16
    dy += 6
    for line in wrap_text_lines(draw, sel_size, font_body, desc_x2 - desc_x1 - 24):
        draw.text((desc_x1 + 12, dy), line, fill=TEXT_PRIMARY, font=font_body)
        dy += 16

    # Volume Disk Space Cost card (`VOLUME_COST` from `UiEngine`)
    cost_y1 = desc_y2 + 12
    cost_y2 = tree_y2
    draw.rounded_rectangle(
        [desc_x1, cost_y1, desc_x2, cost_y2],
        radius=5,
        fill=(249, 250, 253),
        outline=BORDER_COLOR,
        width=1,
    )
    draw.text(
        (desc_x1 + 12, cost_y1 + 10),
        "Volume Disk Space Cost:",
        fill=TEXT_PRIMARY,
        font=font_bold,
    )
    draw.line(
        [(desc_x1 + 12, cost_y1 + 28), (desc_x2 - 12, cost_y1 + 28)],
        fill=(225, 229, 236),
        width=1,
    )

    if state.volume_costs:
        vol_name, req_bytes, free_bytes = state.volume_costs[0]
    else:
        vol_name, req_bytes, free_bytes = ("/", 65_536, 142 * 1024 * 1024 * 1024)

    req_kb = max(1, req_bytes // 1024)
    free_gb = free_bytes / (1024**3)
    draw.text((desc_x1 + 12, cost_y1 + 36), f"Volume '{vol_name}' Required:", fill=TEXT_SECONDARY, font=font_body)
    draw.text((desc_x1 + 145, cost_y1 + 36), f"{req_kb} KB", fill=TEXT_PRIMARY, font=font_bold)
    draw.text((desc_x1 + 12, cost_y1 + 56), "Volume Available:", fill=TEXT_SECONDARY, font=font_body)
    draw.text(
        (desc_x1 + 145, cost_y1 + 56),
        f"{free_gb:.1f} GB",
        fill=(18, 138, 72),
        font=font_bold,
    )

    # Footer bar and real buttons from `state.controls`
    draw.rectangle([(1, footer_y), (width - 2, height - 2)], fill=FOOTER_BG)
    draw.line([(1, footer_y), (width - 1, footer_y)], fill=BORDER_COLOR, width=1)
    back_ctrl = next((c for c in state.controls if c.control_id == "Back"), None)
    next_ctrl = next((c for c in state.controls if c.control_id == "Next"), None)
    cancel_ctrl = next((c for c in state.controls if c.control_id == "Cancel"), None)
    back_label = clean_msi_text(back_ctrl.text) if back_ctrl is not None else "< Back"
    next_label = clean_msi_text(next_ctrl.text) if next_ctrl is not None else "Next >"
    if next_label == "Next":
        next_label = "Next >"
    cancel_label = clean_msi_text(cancel_ctrl.text) if cancel_ctrl is not None else "Cancel"
    btn_y1 = footer_y + 11
    btn_y2 = height - 11
    draw_gui_button(draw, (274, btn_y1, 356, btn_y2), back_label)
    draw_gui_button(draw, (366, btn_y1, 448, btn_y2), next_label, primary=True)
    draw_gui_button(draw, (458, btn_y1, 540, btn_y2), cancel_label)

    return img


def render_gui_progress_from_state(state: GuiRuntimeState) -> Image.Image:
    """Renders `gui_progress_dialog.png` from live `ProgressDlg` state exported by `msi-gui`.

    Args:
        state: Parsed `GuiRuntimeState` for `ProgressDlg`.

    Returns:
        Rendered PIL Image.
    """
    width, height = 560, 440
    img = Image.new("RGB", (width, height), WINDOW_BG)
    draw = ImageDraw.Draw(img)

    title_h = draw_window_chrome(draw, width, height, state.window_title)
    banner_h = 58
    banner_bottom = title_h + banner_h
    footer_y = height - 52

    draw.rectangle([(1, title_h + 1), (width - 2, banner_bottom)], fill=(255, 255, 255))
    draw.line([(1, banner_bottom), (width - 1, banner_bottom)], fill=BORDER_COLOR, width=1)

    title_ctrl = next((c for c in state.controls if c.control_id == "Title"), None)
    title_str = (
        clean_msi_text(title_ctrl.text)
        if title_ctrl is not None
        else f"Installing {state.properties.get('ProductName', 'Open edX Stack')}"
    )
    draw.text((22, title_h + 10), title_str, fill=TEXT_PRIMARY, font=load_font(15, bold=True))
    draw.text(
        (22, title_h + 32),
        "Please wait while the Setup Wizard installs and configures packages.",
        fill=TEXT_SECONDARY,
        font=load_font(12),
    )

    ix, iy = width - 54, title_h + 12
    draw.rounded_rectangle(
        [ix, iy, ix + 34, iy + 34],
        radius=6,
        fill=ACCENT_BLUE,
        outline=(0, 82, 178),
        width=1,
    )
    draw.line([(ix + 8, iy + 14), (ix + 26, iy + 14)], fill=(255, 255, 255), width=2)
    draw.line([(ix + 17, iy + 8), (ix + 17, iy + 26)], fill=(255, 255, 255), width=2)

    action_ctrl = next((c for c in state.controls if c.control_id == "ActionText"), None)
    prog_ctrl = next((c for c in state.controls if c.control_id == "ProgressBar"), None)
    action_str = (
        clean_msi_text(action_ctrl.text)
        if action_ctrl is not None and action_ctrl.text
        else "Status: Executing transaction..."
    )
    pct = prog_ctrl.progress if prog_ctrl is not None else 68

    px1, px2 = 36, width - 36
    py = banner_bottom + 28
    draw.text((px1, py), action_str, fill=TEXT_PRIMARY, font=load_font(13, bold=True))

    bar_y1 = py + 24
    bar_y2 = bar_y1 + 22
    draw.rounded_rectangle(
        [px1, bar_y1, px2, bar_y2],
        radius=5,
        fill=(228, 232, 240),
        outline=BTN_BORDER,
        width=1,
    )
    fill_w = int((px2 - px1) * (pct / 100.0))
    if fill_w > 0:
        draw.rounded_rectangle(
            [px1, bar_y1, px1 + fill_w, bar_y2],
            radius=5,
            fill=ACCENT_BLUE,
        )
    draw.text(
        (px1, bar_y2 + 8),
        f"Progress: {pct}% — Cabinets: #engine.cab, #runtimes.cab, #databases.cab, #codebase.cab",
        fill=TEXT_SECONDARY,
        font=load_font(11),
    )

    # Light native GUI Action Telemetry log panel displaying real `state.action_logs`
    log_y1 = bar_y2 + 32
    log_y2 = footer_y - 16
    draw.rounded_rectangle(
        [px1, log_y1, px2, log_y2],
        radius=5,
        fill=(255, 255, 255),
        outline=BORDER_COLOR,
        width=1,
    )
    draw.rectangle(
        [px1 + 1, log_y1 + 1, px2 - 1, log_y1 + 24],
        fill=(244, 246, 250),
    )
    draw.line([(px1, log_y1 + 24), (px2, log_y1 + 24)], fill=BORDER_COLOR, width=1)
    draw.text(
        (px1 + 10, log_y1 + 6),
        "Transaction & Worker Action Telemetry Log:",
        fill=TEXT_PRIMARY,
        font=load_font(11, bold=True),
    )

    font_mono = load_font(11, mono=True)
    ly = log_y1 + 32
    for log_entry in state.action_logs[:5]:
        trimmed = log_entry if len(log_entry) <= 68 else f"{log_entry[:65]}..."
        draw.text((px1 + 10, ly), trimmed, fill=(40, 52, 70), font=font_mono)
        ly += 18

    draw.rectangle([(1, footer_y), (width - 2, height - 2)], fill=FOOTER_BG)
    draw.line([(1, footer_y), (width - 1, footer_y)], fill=BORDER_COLOR, width=1)
    btn_y1 = footer_y + 11
    btn_y2 = height - 11
    draw_gui_button(draw, (274, btn_y1, 356, btn_y2), "< Back", disabled=True)
    draw_gui_button(draw, (366, btn_y1, 448, btn_y2), "Next >", disabled=True)
    draw_gui_button(draw, (458, btn_y1, 540, btn_y2), "Cancel")

    return img


def draw_box_drawing_cell(
    draw: ImageDraw.ImageDraw,
    ch: str,
    x: int,
    y: int,
    cell_w: int,
    cell_h: int,
    color: tuple[int, int, int],
) -> None:
    """Draws a single Unicode box-drawing or block character using crisp pixel lines.

    Eliminates missing-glyph tofu boxes (`▯`) across all operating systems and fonts.

    Args:
        draw: PIL ImageDraw context.
        ch: Box-drawing or block character (`┌┐└┘─│├┤┬┴┼█░`).
        x: Top-left pixel X coordinate of the character cell.
        y: Top-left pixel Y coordinate of the character cell.
        cell_w: Character cell width in pixels.
        cell_h: Character cell height in pixels.
        color: RGB stroke/fill color.
    """
    cx = x + cell_w // 2
    cy = y + cell_h // 2
    right = x + cell_w
    bottom = y + cell_h

    if ch == "─":
        draw.line([(x, cy), (right, cy)], fill=color, width=2)
    elif ch == "│":
        draw.line([(cx, y), (cx, bottom)], fill=color, width=2)
    elif ch == "┌":
        draw.line([(cx, cy), (right, cy)], fill=color, width=2)
        draw.line([(cx, cy), (cx, bottom)], fill=color, width=2)
    elif ch == "┐":
        draw.line([(x, cy), (cx, cy)], fill=color, width=2)
        draw.line([(cx, cy), (cx, bottom)], fill=color, width=2)
    elif ch == "└":
        draw.line([(cx, y), (cx, cy)], fill=color, width=2)
        draw.line([(cx, cy), (right, cy)], fill=color, width=2)
    elif ch == "┘":
        draw.line([(cx, y), (cx, cy)], fill=color, width=2)
        draw.line([(x, cy), (cx, cy)], fill=color, width=2)
    elif ch == "├":
        draw.line([(cx, y), (cx, bottom)], fill=color, width=2)
        draw.line([(cx, cy), (right, cy)], fill=color, width=2)
    elif ch == "┤":
        draw.line([(cx, y), (cx, bottom)], fill=color, width=2)
        draw.line([(x, cy), (cx, cy)], fill=color, width=2)
    elif ch == "┬":
        draw.line([(x, cy), (right, cy)], fill=color, width=2)
        draw.line([(cx, cy), (cx, bottom)], fill=color, width=2)
    elif ch == "┴":
        draw.line([(x, cy), (right, cy)], fill=color, width=2)
        draw.line([(cx, y), (cx, cy)], fill=color, width=2)
    elif ch == "┼":
        draw.line([(x, cy), (right, cy)], fill=color, width=2)
        draw.line([(cx, y), (cx, bottom)], fill=color, width=2)
    elif ch == "█":
        draw.rectangle([x, y + 2, right - 1, bottom - 2], fill=color)
    elif ch == "░":
        draw.rectangle([x, y + 2, right - 1, bottom - 2], outline=color, width=1)


def render_tui_from_lines(tui_lines: Sequence[str]) -> Image.Image:
    """Renders `tui_wizard.png` from the 80x24 character grid output by `msi install --tui`.

    Args:
        tui_lines: 80x24 character rows captured from `msi install ./ExampleApp.msi --tui`.

    Returns:
        Rendered PIL Image.
    """
    cell_w, cell_h = 8, 16
    pad_x, pad_y = 20, 14
    cols = 80
    rows = max(24, len(tui_lines))
    width = pad_x * 2 + cols * cell_w
    title_h = 34
    height = title_h + pad_y * 2 + rows * cell_h

    img = Image.new("RGB", (width, height), TERM_BG)
    draw = ImageDraw.Draw(img)
    draw_window_chrome(
        draw,
        width,
        height,
        "zsh — msi-cli install ./ExampleApp.msi --tui (80x24)",
        dark=True,
    )

    font_mono = load_font(13, mono=True)
    font_bold = load_font(13, bold=True, mono=True)

    for row_idx, line in enumerate(tui_lines[:rows]):
        y = title_h + pad_y + row_idx * cell_h
        is_focused_btn = "[>" in line
        is_title_border = "┌" in line or "┘" in line
        for col_idx, ch in enumerate(line[:cols]):
            if ch == " ":
                continue
            x = pad_x + col_idx * cell_w
            if ch in BOX_DRAWING_CHARS:
                box_col = TERM_BLUE if is_title_border or ch in "┌┐└┘─│├┤" else TERM_GREEN
                draw_box_drawing_cell(draw, ch, x, y, cell_w, cell_h, box_col)
            else:
                fg = (
                    TERM_GREEN
                    if is_focused_btn
                    else (TERM_BLUE if is_title_border else TERM_TEXT)
                )
                draw.text(
                    (x, y),
                    ch,
                    fill=fg,
                    font=font_bold if (is_focused_btn or is_title_border) else font_mono,
                )

    return img


def render_terminal_session(
    title: str,
    lines: Sequence[tuple[str, str]],
) -> Image.Image:
    """Renders a captured CLI terminal execution session into a PNG image.

    Args:
        title: Terminal window title bar caption.
        lines: Sequence of `(line_text, color_style)` pairs captured from `msi`.

    Returns:
        Rendered PIL Image.
    """
    width = 780
    line_h = 20
    title_h = 34
    pad_y = 16
    max_chars = 100

    wrapped_entries: list[tuple[str, str]] = []
    for text, style in lines:
        if len(text) <= max_chars:
            wrapped_entries.append((text, style))
        else:
            split_idx = text.rfind(" ", 0, max_chars)
            if split_idx <= 0:
                split_idx = max_chars
            wrapped_entries.append((text[:split_idx], style))
            wrapped_entries.append((f"  {text[split_idx:].lstrip()}", style))

    height = max(320, title_h + pad_y * 2 + len(wrapped_entries) * line_h)

    img = Image.new("RGB", (width, height), TERM_BG)
    draw = ImageDraw.Draw(img)
    draw_window_chrome(draw, width, height, title, dark=True)

    font_reg = load_font(12, mono=True)
    font_bold = load_font(12, bold=True, mono=True)

    palette = {
        "cmd": TERM_TEXT,
        "text": TERM_TEXT,
        "dim": TERM_DIM,
        "blue": TERM_BLUE,
        "green": TERM_GREEN,
        "cyan": TERM_CYAN,
        "yellow": TERM_YELLOW,
        "purple": TERM_PURPLE,
    }

    y = title_h + pad_y
    for text, style in wrapped_entries:
        if not text:
            y += line_h // 2
            continue
        is_cmd = style == "cmd"
        color = palette.get(style, TERM_TEXT)
        draw.text(
            (20, y),
            text,
            fill=color,
            font=font_bold if (is_cmd or style == "green") else font_reg,
        )
        y += line_h

    return img


def generate_gui_welcome(artifacts: CapturedWorkspaceArtifacts | None = None) -> Image.Image:
    """Generates `gui_welcome_dialog.png` from real `msi-gui` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_gui_welcome_from_state(cap.welcome_state, cap.license_state)


def generate_gui_feature_tree(
    artifacts: CapturedWorkspaceArtifacts | None = None,
) -> Image.Image:
    """Generates `gui_feature_tree_dialog.png` from real `msi-gui` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_gui_feature_tree_from_state(cap.customize_state)


def generate_gui_progress(artifacts: CapturedWorkspaceArtifacts | None = None) -> Image.Image:
    """Generates `gui_progress_dialog.png` from real `msi-gui` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_gui_progress_from_state(cap.progress_state)


def generate_tui_wizard(artifacts: CapturedWorkspaceArtifacts | None = None) -> Image.Image:
    """Generates `tui_wizard.png` from real `msi install --tui` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_tui_from_lines(cap.tui_lines)


def generate_cli_install(artifacts: CapturedWorkspaceArtifacts | None = None) -> Image.Image:
    """Generates `cli_install_output.png` from real `msi install` and `msi info` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_terminal_session(
        "zsh — msi-cli install ./ExampleApp.msi --ui basic --log ./install.log",
        cap.cli_install_lines,
    )


def generate_cli_chainer(artifacts: CapturedWorkspaceArtifacts | None = None) -> Image.Image:
    """Generates `cli_chainer_output.png` from real `msi-cli pack -v` and `msi-cli msiexec` execution.

    Args:
        artifacts: Optional pre-captured workspace artifacts.

    Returns:
        Rendered PIL Image.
    """
    repo_root = Path(__file__).resolve().parent.parent
    cap = artifacts if artifacts is not None else capture_workspace_artifacts(repo_root)
    return render_terminal_session(
        "zsh — msi-cli pack -v & msi-cli msiexec /i OpenEdX-Setup.msi",
        cap.cli_chainer_lines,
    )


def main() -> None:
    """Builds real `.msi` packages, executes `msi-gui` and `msi`, and saves all screenshots."""
    repo_root = Path(__file__).resolve().parent.parent
    artifacts = capture_workspace_artifacts(repo_root)

    candidates = [
        repo_root.parent / "cc0-assets" / "msi-rs" / "screenshots",
    ]

    output_dirs = [c for c in candidates if c.parent.exists()]
    if not output_dirs:
        fallback = repo_root / "target" / "screenshots"
        fallback.mkdir(parents=True, exist_ok=True)
        output_dirs = [fallback]

    images = {
        "gui_welcome_dialog.png": generate_gui_welcome(artifacts),
        "gui_feature_tree_dialog.png": generate_gui_feature_tree(artifacts),
        "gui_progress_dialog.png": generate_gui_progress(artifacts),
        "tui_wizard.png": generate_tui_wizard(artifacts),
        "cli_install_output.png": generate_cli_install(artifacts),
        "cli_chainer_output.png": generate_cli_chainer(artifacts),
    }

    for target_dir in output_dirs:
        target_dir.mkdir(parents=True, exist_ok=True)
        for name, img in images.items():
            out_path = target_dir / name
            img.save(out_path, format="PNG", optimize=True)
            print(f"Generated {out_path} ({img.width}x{img.height})")


if __name__ == "__main__":
    main()
