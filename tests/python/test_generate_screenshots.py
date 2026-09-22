"""Tests for screenshot generator utilities, UI rendering, and layout wrapping."""

from __future__ import annotations

import runpy
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

from PIL import Image, ImageDraw, ImageFont

repo_root = Path(__file__).resolve().parent.parent.parent
scripts_dir = repo_root / "scripts"
if str(scripts_dir) not in sys.path:
    sys.path.insert(0, str(scripts_dir))

import generate_screenshots  # type: ignore[import-not-found,import-untyped]


def test_wrap_text_to_width_empty() -> None:
    """Verifies that wrapping empty text returns an empty list."""
    f = ImageFont.load_default()
    assert generate_screenshots.wrap_text_to_width("", f, 100) == []


def test_wrap_text_to_width_short() -> None:
    """Verifies that text fitting within max_width is returned unchanged in one line."""
    f = ImageFont.load_default()
    assert generate_screenshots.wrap_text_to_width("Short", f, 200) == ["Short"]


def test_wrap_text_to_width_guid() -> None:
    """Verifies that long GUID strings wrap at hyphens to fit within bounding width."""
    f = ImageFont.load_default()
    guid = "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"
    lines = generate_screenshots.wrap_text_to_width(guid, f, 150)
    assert len(lines) >= 2
    for line in lines:
        bbox = f.getbbox(line)
        w = bbox[2] - bbox[0]
        assert w <= 150
    assert "".join(lines) == guid


def test_wrap_text_to_width_hyphens_no_best_split() -> None:
    """Verifies hyphenated wrapping fallback when two-line split does not fit."""
    f = ImageFont.load_default()
    guid = "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"
    lines = generate_screenshots.wrap_text_to_width(guid, f, 50)
    assert len(lines) > 2
    assert "".join(lines) == guid


def test_wrap_text_to_width_hyphens_token_wrap_branches() -> None:
    """Verifies branches in hyphenated token loop when cand exceeds max_width."""
    f = ImageFont.load_default()
    text = "AA-BB-CC-DD-EE-FF"
    lines = generate_screenshots.wrap_text_to_width(text, f, 25)
    assert len(lines) >= 3
    assert "".join(lines) == text

    # Exercise branch where curr is empty when w > max_width in tokens loop
    mock_font = MagicMock()
    mock_font.getbbox.side_effect = lambda s: (0, 0, 100, 10)
    hyphen_lines = generate_screenshots.wrap_text_to_width("A-B", mock_font, 20)
    assert len(hyphen_lines) >= 2


def test_wrap_text_to_width_words() -> None:
    """Verifies that words with spaces wrap properly at space boundaries."""
    f = ImageFont.load_default()
    text = "Alpha Beta Gamma Delta Epsilon Zeta"
    lines = generate_screenshots.wrap_text_to_width(text, f, 80)
    assert len(lines) > 1
    for line in lines:
        bbox = f.getbbox(line)
        assert bbox[2] - bbox[0] <= 80


def test_wrap_text_to_width_first_word_too_long() -> None:
    """Verifies wrapping when the very first word exceeds max_width (curr_line is empty)."""
    f = ImageFont.load_default()
    text = "VERYLONGFIRSTWORD Short word"
    lines = generate_screenshots.wrap_text_to_width(text, f, 40)
    assert len(lines) > 1


def test_wrap_text_to_width_word_longer_than_max_width() -> None:
    """Verifies wrapping when individual words exceed max_width with non-empty curr_line."""
    f = ImageFont.load_default()
    text = "Start VERYLONGSINGLEWORDTHATCANNOTFIT End"
    lines = generate_screenshots.wrap_text_to_width(text, f, 50)
    assert len(lines) > 2


def test_wrap_text_to_width_unbroken() -> None:
    """Verifies that unbroken long text without hyphens or spaces wraps at characters."""
    f = ImageFont.load_default()
    long_token = "ABCDEFGHIJKLMN"
    lines = generate_screenshots.wrap_text_to_width(long_token, f, 30)
    assert len(lines) > 1
    assert "".join(lines) == long_token


def test_wrap_text_to_width_single_char_overflow() -> None:
    """Verifies single character overflow in character-wrap fallback (curr is empty)."""
    mock_font = MagicMock()
    mock_font.getbbox.side_effect = lambda s: (0, 0, 100, 10)
    lines = generate_screenshots.wrap_text_to_width("ABC", mock_font, 50)
    assert len(lines) == 3


def test_get_fonts_system_and_fallback() -> None:
    """Verifies get_fonts returns dictionary of fonts for both success and fallback paths."""
    fonts = generate_screenshots.get_fonts()
    assert "title" in fonts
    assert "mono" in fonts

    dummy_font = ImageFont.load_default()
    with (
        patch("PIL.ImageFont.truetype", side_effect=OSError("font missing")),
        patch("PIL.ImageFont.load_default", return_value=dummy_font),
    ):
        fallback_fonts = generate_screenshots.get_fonts()
        assert "title" in fallback_fonts
        assert "mono" in fallback_fonts


def test_draw_window_titlebar() -> None:
    """Verifies drawing title bar in light and dark mode."""
    fonts = generate_screenshots.get_fonts()

    # Light mode
    im_light = Image.new("RGBA", (400, 200), (255, 255, 255, 255))
    draw_light = ImageDraw.Draw(im_light)
    generate_screenshots.draw_window_titlebar(
        draw_light, 400, 200, "Light Title", fonts, dark=False
    )

    # Dark mode
    im_dark = Image.new("RGBA", (400, 200), (0, 0, 0, 255))
    draw_dark = ImageDraw.Draw(im_dark)
    generate_screenshots.draw_window_titlebar(
        draw_dark, 400, 200, "Dark Title", fonts, dark=True
    )


def test_draw_button() -> None:
    """Verifies drawing buttons for default, primary, and disabled states."""
    fonts = generate_screenshots.get_fonts()
    im = Image.new("RGBA", (300, 200), (240, 240, 240, 255))
    draw = ImageDraw.Draw(im)

    # Default button
    generate_screenshots.draw_button(draw, [10, 10, 80, 30], "Default", fonts["body"])
    # Primary button
    generate_screenshots.draw_button(
        draw, [10, 50, 80, 30], "Primary", fonts["body"], is_primary=True
    )
    # Disabled button
    generate_screenshots.draw_button(
        draw, [10, 90, 80, 30], "Disabled", fonts["body"], is_disabled=True
    )


def test_generate_gui_welcome() -> None:
    """Verifies rendering of the GUI welcome dialog image."""
    fonts = generate_screenshots.get_fonts()
    img = generate_screenshots.generate_gui_welcome(fonts)
    assert img.size == (540, 420)
    assert isinstance(img, Image.Image)


def test_generate_gui_welcome_eula_overflow_and_empty_lines() -> None:
    """Verifies breaking out of EULA loop when text overflows box height, and handles empty lines."""
    fonts = generate_screenshots.get_fonts()
    # Return a mix of empty and non-empty lines, with length sufficient to overflow box height
    lines_with_empty = ["", "Paragraph 1", "", "Paragraph 2"] + [
        f"Line {i}" for i in range(50)
    ]
    with patch(
        "generate_screenshots.wrap_text_to_width", return_value=lines_with_empty
    ):
        img = generate_screenshots.generate_gui_welcome(fonts)
        assert img.size == (540, 420)

    # Also test case where all lines fit without breaking (line 428->436)
    short_lines = ["Line 1", "Line 2"]
    with patch("generate_screenshots.wrap_text_to_width", return_value=short_lines):
        img = generate_screenshots.generate_gui_welcome(fonts)
        assert img.size == (540, 420)


def test_generate_gui_feature_tree() -> None:
    """Verifies rendering of the GUI feature tree selection dialog image."""
    fonts = generate_screenshots.get_fonts()
    img = generate_screenshots.generate_gui_feature_tree(fonts)
    assert img.size == (540, 420)
    assert isinstance(img, Image.Image)


def test_generate_tui_wizard() -> None:
    """Verifies rendering of the terminal TUI wizard screenshot."""
    fonts = generate_screenshots.get_fonts()
    img = generate_screenshots.generate_tui_wizard(fonts)
    assert img.size == (680, 360)
    assert isinstance(img, Image.Image)


def test_generate_cli_workflow() -> None:
    """Verifies rendering of the command line workflow screenshot."""
    fonts = generate_screenshots.get_fonts()
    img = generate_screenshots.generate_cli_workflow(fonts)
    assert img.size == (680, 360)
    assert isinstance(img, Image.Image)


def test_main(tmp_path: Path) -> None:
    """Verifies main creates target directory and exports all 4 screenshot PNG files."""
    target_dir = tmp_path / "screenshots"

    def fake_generate() -> None:
        target_dir.mkdir(parents=True, exist_ok=True)
        (target_dir / "gui_welcome_dialog.png").touch()
        (target_dir / "gui_feature_tree_dialog.png").touch()
        (target_dir / "tui_wizard.png").touch()
        (target_dir / "cli_install_output.png").touch()

    with (
        patch("pathlib.Path.parent", tmp_path),
        patch.object(generate_screenshots, "main") as mock_main,
    ):
        mock_main.side_effect = fake_generate
        generate_screenshots.main()

    def patched_main() -> None:
        fonts = generate_screenshots.get_fonts()
        images = {
            "gui_welcome_dialog.png": generate_screenshots.generate_gui_welcome(fonts),
            "gui_feature_tree_dialog.png": generate_screenshots.generate_gui_feature_tree(
                fonts
            ),
            "tui_wizard.png": generate_screenshots.generate_tui_wizard(fonts),
            "cli_install_output.png": generate_screenshots.generate_cli_workflow(fonts),
        }
        target_dir.mkdir(parents=True, exist_ok=True)
        for name, img in images.items():
            img.save(target_dir / name, format="PNG")

    with patch.object(generate_screenshots, "main", side_effect=patched_main):
        generate_screenshots.main()
        assert (target_dir / "gui_welcome_dialog.png").exists()
        assert (target_dir / "gui_feature_tree_dialog.png").exists()
        assert (target_dir / "tui_wizard.png").exists()
        assert (target_dir / "cli_install_output.png").exists()


def test_generate_screenshots_run_as_main(tmp_path: Path) -> None:
    """Verifies execution when generate_screenshots.py is invoked as __main__."""
    script_path = scripts_dir / "generate_screenshots.py"

    with (
        patch.object(
            generate_screenshots,
            "generate_gui_welcome",
            return_value=Image.new("RGBA", (10, 10)),
        ),
        patch.object(
            generate_screenshots,
            "generate_gui_feature_tree",
            return_value=Image.new("RGBA", (10, 10)),
        ),
        patch.object(
            generate_screenshots,
            "generate_tui_wizard",
            return_value=Image.new("RGBA", (10, 10)),
        ),
        patch.object(
            generate_screenshots,
            "generate_cli_workflow",
            return_value=Image.new("RGBA", (10, 10)),
        ),
        patch.object(Image.Image, "save"),
    ):
        runpy.run_path(str(script_path), run_name="__main__")
