"""Tests for real screenshot generator utilities, state parsing, and rendering."""

from __future__ import annotations

import runpy
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

from PIL import Image, ImageDraw, ImageFont

SCRIPTS_DIR = Path(__file__).resolve().parent.parent.parent / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import generate_screenshots  # noqa: E402


def _make_sample_artifacts() -> generate_screenshots.CapturedWorkspaceArtifacts:
    """Creates a sample `CapturedWorkspaceArtifacts` fixture for unit testing.

    Returns:
        Populated `CapturedWorkspaceArtifacts`.
    """
    welcome_state = generate_screenshots.GuiRuntimeState(
        window_title="ExampleApp Setup - msi-gui",
        active_dialog="WelcomeDlg",
        dialog_title="ExampleApp Setup",
        theme_style="Mondo",
        properties={
            "ProductName": "ExampleApp",
            "ProductVersion": "1.2.0",
            "Manufacturer": "Contoso Systems Inc.",
            "ProductCode": "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}",
        },
        controls=[
            generate_screenshots.GuiControlRecord(
                "Title",
                "Text",
                135,
                20,
                220,
                60,
                True,
                True,
                False,
                0,
                "{"
                + chr(92)
                + "WixUI_Font_Bigger}Welcome to the ExampleApp Setup Wizard",
            ),
            generate_screenshots.GuiControlRecord(
                "Description",
                "Text",
                135,
                70,
                220,
                40,
                True,
                True,
                False,
                0,
                "The Setup Wizard will install ExampleApp on your computer.",
            ),
            generate_screenshots.GuiControlRecord(
                "Next", "PushButton", 236, 243, 56, 17, True, True, True, 0, "&Next"
            ),
            generate_screenshots.GuiControlRecord(
                "Cancel",
                "PushButton",
                304,
                243,
                56,
                17,
                True,
                True,
                False,
                0,
                "Cancel",
            ),
        ],
    )
    license_state = generate_screenshots.GuiRuntimeState(
        controls=[
            generate_screenshots.GuiControlRecord(
                "LicenseText",
                "ScrollableText",
                20,
                60,
                330,
                140,
                True,
                True,
                False,
                0,
                "Apache License" + chr(92) + "n" + chr(92) + "nVersion 2.0",
            ),
            generate_screenshots.GuiControlRecord(
                "Next", "PushButton", 236, 243, 56, 17, True, True, True, 0, "I &agree"
            ),
        ]
    )
    customize_state = generate_screenshots.GuiRuntimeState(
        window_title="ExampleApp Setup - msi-gui",
        active_dialog="CustomizeDlg",
        properties={
            "SelectionDescription": "Core execution binaries and runtime daemon services.",
            "SelectionSize": "This feature requires 52 KB.",
        },
        controls=[
            generate_screenshots.GuiControlRecord(
                "Title",
                "Text",
                15,
                6,
                200,
                15,
                True,
                True,
                False,
                0,
                "{" + chr(92) + "WixUI_Font_Title}Custom Setup",
            ),
            generate_screenshots.GuiControlRecord(
                "Description",
                "Text",
                25,
                23,
                280,
                15,
                True,
                True,
                False,
                0,
                "Select the way you want features to be installed.",
            ),
            generate_screenshots.GuiControlRecord(
                "Back", "PushButton", 180, 243, 56, 17, True, True, False, 0, "< &Back"
            ),
            generate_screenshots.GuiControlRecord(
                "Next", "PushButton", 236, 243, 56, 17, True, True, True, 0, "&Next"
            ),
            generate_screenshots.GuiControlRecord(
                "Cancel", "PushButton", 304, 243, 56, 17, True, True, False, 0, "Cancel"
            ),
        ],
        tree_nodes=[
            generate_screenshots.GuiTreeNode(
                0, "Core", "ExampleApp Core Runtime", 53248, "Local", "Core desc"
            ),
            generate_screenshots.GuiTreeNode(
                1, "Srv", "Server Daemon & Service", 24576, "Local", "Srv desc"
            ),
            generate_screenshots.GuiTreeNode(
                1, "OptChild", "Optional Child", 8192, "Absent", "Opt desc"
            ),
            generate_screenshots.GuiTreeNode(
                0, "Docs", "Documentation & SDK", 4096, "Absent", "Docs desc"
            ),
        ],
        volume_costs=[("/", 65536, 142 * 1024 * 1024 * 1024)],
    )
    progress_state = generate_screenshots.GuiRuntimeState(
        window_title="Open edX Stack Setup - msi-gui",
        active_dialog="ProgressDlg",
        properties={"ProductName": "Open edX Stack"},
        controls=[
            generate_screenshots.GuiControlRecord(
                "Title",
                "Text",
                20,
                15,
                200,
                15,
                True,
                True,
                False,
                0,
                "Installing Open edX Stack",
            ),
            generate_screenshots.GuiControlRecord(
                "ActionText",
                "Text",
                70,
                100,
                265,
                10,
                True,
                True,
                False,
                0,
                "Status: Executing transaction...",
            ),
            generate_screenshots.GuiControlRecord(
                "ProgressBar",
                "ProgressBar",
                35,
                115,
                300,
                10,
                True,
                True,
                False,
                68,
                "",
            ),
        ],
        action_logs=[
            "MSI (s) (i): Starting installation",
            "MSI (s) (i): " + ("Very long telemetry log line " * 4),
        ],
    )
    tui_lines = [
        "┌────── Welcome to the ExampleApp Setup Wizard ──────┐",
        "│  Welcome to the ExampleApp Setup Wizard            │",
        "├┬┴┼─────────────────────────────────────────────────┤",
        "│  [██████████░░░░░] 68%                             │",
        "│  [> Next <]   [ Cancel ]                           │",
        "└────────────────────────────────────────────────────┘",
    ]
    cli_install_lines = [
        ("$ msi-cli install ./ExampleApp.msi --ui basic --log ./install.log", "cmd"),
        ("  MSI (s) (i): Starting installation", "dim"),
        ("", "dim"),
        ("Successfully installed package 'ExampleApp' v1.2.0", "green"),
    ]
    cli_chainer_lines = [
        ("$ msi-cli pack -v -o OpenEdX-Setup.msi Product.wxs Payload.wxs", "cmd"),
        ("[INFO] Media cabinets: 4", "blue"),
        ("  MSI (s) (i): Executing MsiEmbeddedChainer 'LibScriptChainer'", "yellow"),
    ]
    return generate_screenshots.CapturedWorkspaceArtifacts(
        welcome_state=welcome_state,
        license_state=license_state,
        customize_state=customize_state,
        progress_state=progress_state,
        tui_lines=tui_lines,
        cli_install_lines=cli_install_lines,
        cli_chainer_lines=cli_chainer_lines,
    )


def test_load_font_variants_and_fallbacks() -> None:
    """Tests `load_font` across regular, bold, mono, and fallback branches."""
    f1 = generate_screenshots.load_font(12, bold=False, mono=False)
    f2 = generate_screenshots.load_font(12, bold=True, mono=False)
    f3 = generate_screenshots.load_font(12, bold=False, mono=True)
    f4 = generate_screenshots.load_font(12, bold=True, mono=True)
    assert f1 is not None and f2 is not None and f3 is not None and f4 is not None

    with patch("os.path.exists", return_value=False):
        default_font = generate_screenshots.load_font(12)
        assert default_font is not None

    mock_font = MagicMock()
    with (
        patch("os.path.exists", return_value=True),
        patch(
            "PIL.ImageFont.truetype",
            side_effect=[OSError("bad index 1"), mock_font],
        ) as mock_tt,
    ):
        res = generate_screenshots.load_font(12, bold=True, mono=False)
        assert res is mock_font
        assert mock_tt.call_count == 2

    with (
        patch("os.path.exists", return_value=True),
        patch("PIL.ImageFont.truetype", side_effect=OSError("all failed")),
        patch("PIL.ImageFont.load_default", return_value=mock_font),
    ):
        fallback_font = generate_screenshots.load_font(12, bold=True)
        assert fallback_font is mock_font


def test_clean_msi_text_and_strip_rtf() -> None:
    """Tests `clean_msi_text` and `strip_rtf_text` across all syntax branches."""
    bs = chr(92)
    assert generate_screenshots.clean_msi_text("< &Back") == "< Back"
    assert generate_screenshots.clean_msi_text("Trailing &") == "Trailing "
    assert (
        generate_screenshots.clean_msi_text("{" + bs + "WixUI_Font_Bigger}Title")
        == "Title"
    )
    assert (
        generate_screenshots.clean_msi_text("{" + bs + "Unclosed")
        == "{" + bs + "Unclosed"
    )

    rtf_doc = (
        "{"
        + bs
        + "rtf1"
        + bs
        + "ansi{"
        + bs
        + "fonttbl{"
        + bs
        + "f0 Arial;}}{"
        + bs
        + "*"
        + bs
        + "generator x;}{"
        + bs
        + "b Bold}"
        + bs
        + "cf-1 "
        + bs
        + "fs20 Line 1"
        + bs
        + bs
        + " "
        + bs
        + "{brace"
        + bs
        + "}"
        + bs
        + "par Line 2"
        + bs
        + "line Line 3"
        + chr(13)
        + chr(10)
        + bs
        + "}"
    )
    cleaned = generate_screenshots.clean_msi_text(rtf_doc)
    assert "Bold" in cleaned
    assert "Line 1" in cleaned
    assert "{brace}" in cleaned
    assert "Line 2" + chr(10) + "Line 3" in cleaned

    assert (
        generate_screenshots.strip_rtf_text("{" + bs + "rtf1 Trailing" + bs)
        == "Trailing"
    )
    assert (
        generate_screenshots.strip_rtf_text("{" + bs + "rtf1 A" + bs + ".B}") == "A.B"
    )


def test_parse_runtime_state() -> None:
    """Tests `parse_runtime_state` on all record types and malformed lines."""
    dump = """
WINDOW_TITLE=Test Setup - msi-gui
ACTIVE_DIALOG=CustomizeDlg
DIALOG_TITLE=Custom Setup
THEME_STYLE=Mondo
PROP:ProductName=TestApp
PROP:MalformedPropWithoutEquals
CONTROL:Next|PushButton|236,243,56,17|visible=true|enabled=true|default=true|progress=0|text=&Next
CONTROL:NoText|Line|0,234,370,0|visible=true|enabled=true|default=false|progress=0
CONTROL:Short|Malformed
TREE_NODE:0|Root|Root Title|4096|Local|Root Desc
TREE_NODE:1|BadNode
VOLUME_COST:/|Fixed Disk|100000|2048|80000
VOLUME_COST:ShortVol
ACTION_LOG:Step 1 completed
UNKNOWN_LINE=Ignored
"""
    state = generate_screenshots.parse_runtime_state(dump)
    assert state.window_title == "Test Setup - msi-gui"
    assert state.active_dialog == "CustomizeDlg"
    assert state.dialog_title == "Custom Setup"
    assert state.theme_style == "Mondo"
    assert state.properties["ProductName"] == "TestApp"
    assert len(state.controls) == 2
    assert state.controls[0].text == "&Next"
    assert state.controls[1].text == ""
    assert len(state.tree_nodes) == 1
    assert state.tree_nodes[0].title == "Root Title"
    assert len(state.volume_costs) == 1
    assert state.action_logs == ["Step 1 completed"]


def test_rendering_functions_and_fallbacks() -> None:
    """Tests GUI, TUI, and CLI rendering functions including fallback branches."""
    artifacts = _make_sample_artifacts()

    img1 = generate_screenshots.generate_gui_welcome(artifacts)
    assert img1.size == (560, 440)

    # Test welcome fallback when controls are empty and Next label is already "Next >"
    empty_welcome = generate_screenshots.GuiRuntimeState()
    empty_license = generate_screenshots.GuiRuntimeState()
    img1_empty = generate_screenshots.render_gui_welcome_from_state(
        empty_welcome, empty_license
    )
    assert img1_empty.size == (560, 440)

    img2 = generate_screenshots.generate_gui_feature_tree(artifacts)
    assert img2.size == (560, 440)

    # Test feature tree overflow and empty volume_costs fallback
    many_nodes = [
        generate_screenshots.GuiTreeNode(0, f"F{i}", f"Feat {i}", 1024, "Local", "")
        for i in range(15)
    ]
    overflow_customize = generate_screenshots.GuiRuntimeState(tree_nodes=many_nodes)
    img2_overflow = generate_screenshots.render_gui_feature_tree_from_state(
        overflow_customize
    )
    assert img2_overflow.size == (560, 440)

    img3 = generate_screenshots.generate_gui_progress(artifacts)
    assert img3.size == (560, 440)

    # Test progress fallback when controls are missing and progress is 0
    zero_progress = generate_screenshots.GuiRuntimeState(
        controls=[
            generate_screenshots.GuiControlRecord(
                "ProgressBar", "ProgressBar", 0, 0, 100, 10, True, True, False, 0, ""
            )
        ]
    )
    img3_zero = generate_screenshots.render_gui_progress_from_state(zero_progress)
    assert img3_zero.size == (560, 440)

    img4 = generate_screenshots.generate_tui_wizard(artifacts)
    assert img4.width == 680

    # Test draw_box_drawing_cell with unknown char
    draw = ImageDraw.Draw(img4)
    generate_screenshots.draw_box_drawing_cell(draw, "?", 0, 0, 8, 16, (255, 255, 255))
    assert (
        generate_screenshots.wrap_text_lines(draw, "", ImageFont.load_default(), 100)
        == []
    )

    # Test render_terminal_session with long spaced and unspaced lines
    long_spaced = "word " * 30
    long_unspaced = "X" * 120
    term_img = generate_screenshots.render_terminal_session(
        "Title",
        [(long_spaced, "cmd"), (long_unspaced, "green"), ("", "dim")],
    )
    assert term_img.width == 780

    img5 = generate_screenshots.generate_cli_install(artifacts)
    assert img5.width == 780

    img6 = generate_screenshots.generate_cli_chainer(artifacts)
    assert img6.width == 780

    # Test calling generate_* with artifacts=None
    with patch.object(
        generate_screenshots,
        "capture_workspace_artifacts",
        return_value=artifacts,
    ):
        assert generate_screenshots.generate_gui_welcome() is not None
        assert generate_screenshots.generate_gui_feature_tree() is not None
        assert generate_screenshots.generate_gui_progress() is not None
        assert generate_screenshots.generate_tui_wizard() is not None
        assert generate_screenshots.generate_cli_install() is not None
        assert generate_screenshots.generate_cli_chainer() is not None


def test_ensure_binaries_and_capture_artifacts(tmp_path: Path) -> None:
    """Tests `ensure_workspace_binaries` and `capture_workspace_artifacts`."""
    # 1. When binaries do not exist yet, cargo build is called
    with patch("subprocess.run") as mock_run:
        msi_bin, gui_bin = generate_screenshots.ensure_workspace_binaries(tmp_path)
        assert mock_run.called
        assert "msi-cli" in msi_bin.name
        assert "msi-gui" in gui_bin.name

    # 2. When binaries already exist in target/debug
    debug_dir = tmp_path / "target" / "debug"
    debug_dir.mkdir(parents=True, exist_ok=True)
    exe_suffix = ".exe" if generate_screenshots.os.name == "nt" else ""
    (debug_dir / f"msi-cli{exe_suffix}").write_text("bin", encoding="utf-8")
    (debug_dir / f"msi-gui{exe_suffix}").write_text("bin", encoding="utf-8")
    with patch("subprocess.run") as mock_run2:
        msi_bin2, gui_bin2 = generate_screenshots.ensure_workspace_binaries(tmp_path)
        assert not mock_run2.called
        assert msi_bin2.exists() and gui_bin2.exists()

    # 3. Test capture_workspace_artifacts with mocked subprocesses
    nl = chr(10)

    def fake_run(cmd: list[str], **kwargs: object) -> MagicMock:
        cwd = kwargs.get("cwd")
        env = kwargs.get("env")
        if isinstance(cwd, Path):
            if "--log" in cmd:
                (cwd / "install.log").write_text(
                    f"MSI (s) (i): Starting{nl}MSI (s) (i): Installation committed successfully{nl}",
                    encoding="utf-8",
                )
            if "/lv" in cmd:
                (cwd / "chainer.log").write_text(
                    f"MSI (s) (i): Executing MsiEmbeddedChainer 'LibScriptChainer'{nl}",
                    encoding="utf-8",
                )
        if isinstance(env, dict) and "MSI_GUI_DUMP_STATE" in env:
            Path(str(env["MSI_GUI_DUMP_STATE"])).write_text(
                f"WINDOW_TITLE=App Setup{nl}ACTIVE_DIALOG=WelcomeDlg{nl}",
                encoding="utf-8",
            )
        res = MagicMock()
        res.stdout = f"[INFO] Media cabinets: 4{nl}  * DiskId 1: #engine.cab{nl}Successfully installed{nl}"
        return res

    with patch("subprocess.run", side_effect=fake_run):
        cap = generate_screenshots.capture_workspace_artifacts(tmp_path)
        assert cap.welcome_state.window_title == "App Setup"
        assert any(style == "green" for _, style in cap.cli_install_lines)
        assert any(style == "yellow" for _, style in cap.cli_chainer_lines)

    # Also test when LICENSE-APACHE exists and log files are absent
    (tmp_path / "LICENSE-APACHE").write_text("Apache 2.0", encoding="utf-8")

    def fake_run_nologs(cmd: list[str], **kwargs: object) -> MagicMock:
        env = kwargs.get("env")
        if isinstance(env, dict) and "MSI_GUI_DUMP_STATE" in env:
            Path(str(env["MSI_GUI_DUMP_STATE"])).write_text(
                f"WINDOW_TITLE=App Setup{nl}", encoding="utf-8"
            )
        res = MagicMock()
        res.stdout = f"Output line{nl}"
        return res

    with patch("subprocess.run", side_effect=fake_run_nologs):
        cap2 = generate_screenshots.capture_workspace_artifacts(tmp_path)
        assert cap2.welcome_state.window_title == "App Setup"


def test_main_and_dunder_main(tmp_path: Path) -> None:
    """Tests `main()` execution for both existing output dirs and fallback dir."""
    artifacts = _make_sample_artifacts()
    small_img = Image.new("RGB", (10, 10))
    nl = chr(10)

    with (
        patch.object(
            generate_screenshots,
            "capture_workspace_artifacts",
            return_value=artifacts,
        ),
        patch.object(
            generate_screenshots, "generate_gui_welcome", return_value=small_img
        ),
        patch.object(
            generate_screenshots, "generate_gui_feature_tree", return_value=small_img
        ),
        patch.object(
            generate_screenshots, "generate_gui_progress", return_value=small_img
        ),
        patch.object(
            generate_screenshots, "generate_tui_wizard", return_value=small_img
        ),
        patch.object(
            generate_screenshots, "generate_cli_install", return_value=small_img
        ),
        patch.object(
            generate_screenshots, "generate_cli_chainer", return_value=small_img
        ),
        patch.object(Image.Image, "save") as mock_save,
    ):
        generate_screenshots.main()
        assert mock_save.call_count >= 6

        with (
            patch.object(Path, "exists", return_value=False),
            patch.object(Path, "mkdir"),
        ):
            generate_screenshots.main()

    script_path = SCRIPTS_DIR / "generate_screenshots.py"
    with (
        patch("subprocess.run") as mock_subproc,
        patch.object(Path, "read_text", return_value=f"WINDOW_TITLE=MainTest{nl}"),
        patch.object(Path, "write_text"),
        patch.object(Path, "write_bytes"),
        patch.object(Image.Image, "save"),
    ):
        mock_res = MagicMock()
        mock_res.stdout = f"TUI Line{nl}"
        mock_subproc.return_value = mock_res
        runpy.run_path(str(script_path), run_name="__main__")
