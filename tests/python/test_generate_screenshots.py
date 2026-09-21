"""Tests for screenshot generator utilities and layout wrapping."""

import sys
from pathlib import Path

from PIL import ImageFont

repo_root = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(repo_root / "scripts"))

from generate_screenshots import wrap_text_to_width


def test_wrap_text_to_width_empty() -> None:
    """Verifies that wrapping empty text returns an empty list."""
    f = ImageFont.load_default()
    assert wrap_text_to_width("", f, 100) == []


def test_wrap_text_to_width_short() -> None:
    """Verifies that text fitting within max_width is returned unchanged in one line."""
    f = ImageFont.load_default()
    assert wrap_text_to_width("Short", f, 200) == ["Short"]


def test_wrap_text_to_width_guid() -> None:
    """Verifies that long GUID strings wrap at hyphens to fit within bounding width."""
    f = ImageFont.load_default()
    guid = "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"
    lines = wrap_text_to_width(guid, f, 150)
    assert len(lines) >= 2
    for line in lines:
        bbox = f.getbbox(line)
        w = bbox[2] - bbox[0]
        assert w <= 150
    assert "".join(lines) == guid


def test_wrap_text_to_width_words() -> None:
    """Verifies that words with spaces wrap properly at space boundaries."""
    f = ImageFont.load_default()
    text = "Alpha Beta Gamma Delta Epsilon Zeta"
    lines = wrap_text_to_width(text, f, 80)
    assert len(lines) > 1
    for line in lines:
        bbox = f.getbbox(line)
        assert bbox[2] - bbox[0] <= 80


def test_wrap_text_to_width_unbroken() -> None:
    """Verifies that unbroken long text without hyphens or spaces wraps at characters."""
    f = ImageFont.load_default()
    long_token = "ABCDEFGHIJKLMN"
    lines = wrap_text_to_width(long_token, f, 30)
    assert len(lines) > 1
    assert "".join(lines) == long_token
