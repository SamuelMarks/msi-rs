#!/usr/bin/env python3
"""Screenshot generator for msi-rs.

Generates pixel-perfect screenshots of:
1. Native Desktop GUI Welcome Dialog (`gui_welcome_dialog.png`)
2. Native Desktop GUI Feature Tree Dialog (`gui_feature_tree_dialog.png`)
3. Interactive Terminal TUI Wizard (`tui_wizard.png`)
4. Command-Line Installation Pipeline (`cli_install_output.png`)
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def get_fonts():
    """Resolves system fonts for crisp UI rendering."""
    try:
        font_title = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf", 15
        )
        font_heading = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf", 13
        )
        font_body = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial.ttf", 12
        )
        font_body_bold = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf", 12
        )
        font_small = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial.ttf", 11
        )
        font_small_bold = ImageFont.truetype(
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf", 11
        )
        font_mono = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 12)
        font_mono_bold = ImageFont.truetype(
            "/System/Library/Fonts/Menlo.ttc", 12, index=1
        )
        font_mono_small = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 11)
        return {
            "title": font_title,
            "heading": font_heading,
            "body": font_body,
            "body_bold": font_body_bold,
            "small": font_small,
            "small_bold": font_small_bold,
            "mono": font_mono,
            "mono_bold": font_mono_bold,
            "mono_small": font_mono_small,
        }
    except (OSError, RuntimeError):
        f = ImageFont.load_default()
        return {
            k: f
            for k in [
                "title",
                "heading",
                "body",
                "body_bold",
                "small",
                "small_bold",
                "mono",
                "mono_bold",
                "mono_small",
            ]
        }


def draw_window_titlebar(
    draw: ImageDraw.ImageDraw,
    w: int,
    h: int,
    title: str,
    fonts: dict,
    dark: bool = False,
):
    """Draws macOS-style window title bar with traffic lights and centered title."""
    bar_bg = (40, 44, 52, 255) if dark else (235, 237, 240, 255)
    border_color = (30, 33, 39, 255) if dark else (205, 208, 212, 255)
    title_color = (190, 195, 205, 255) if dark else (60, 64, 72, 255)

    draw.rounded_rectangle([0, 0, w - 1, 32], radius=8, fill=bar_bg)
    draw.rectangle([0, 24, w - 1, 32], fill=bar_bg)  # Square off bottom of titlebar
    draw.line([0, 32, w - 1, 32], fill=border_color, width=1)

    # Traffic light buttons
    lights = [
        (13, 10, (255, 95, 87, 255), (224, 68, 62, 255)),  # Close (Red)
        (33, 10, (254, 188, 46, 255), (214, 153, 31, 255)),  # Minimize (Yellow)
        (53, 10, (40, 200, 64, 255), (30, 165, 48, 255)),  # Maximize (Green)
    ]
    for x, y, fill_c, stroke_c in lights:
        draw.ellipse([x, y, x + 12, y + 12], fill=fill_c, outline=stroke_c, width=1)

    # Window title centered
    bbox = fonts["small_bold"].getbbox(title)
    tw = bbox[2] - bbox[0]
    draw.text(((w - tw) // 2, 8), title, fill=title_color, font=fonts["small_bold"])


def draw_button(
    draw: ImageDraw.ImageDraw,
    rect: list,
    text: str,
    font,
    is_primary: bool = False,
    is_disabled: bool = False,
):
    """Draws a styled button with centered label."""
    bx, by, bw, bh = rect
    if is_disabled:
        draw.rounded_rectangle(
            [bx, by, bx + bw, by + bh],
            radius=4,
            fill=(245, 246, 248, 255),
            outline=(218, 222, 228, 255),
            width=1,
        )
        text_color = (160, 165, 175, 255)
    elif is_primary:
        draw.rounded_rectangle(
            [bx, by, bx + bw, by + bh],
            radius=4,
            fill=(0, 102, 204, 255),
            outline=(0, 82, 163, 255),
            width=1,
        )
        # Subtle focus ring
        draw.rounded_rectangle(
            [bx - 1, by - 1, bx + bw + 1, by + bh + 1],
            radius=5,
            outline=(100, 160, 235, 120),
            width=1,
        )
        text_color = (255, 255, 255, 255)
    else:
        draw.rounded_rectangle(
            [bx, by, bx + bw, by + bh],
            radius=4,
            fill=(255, 255, 255, 255),
            outline=(190, 195, 205, 255),
            width=1,
        )
        text_color = (40, 45, 55, 255)

    bbox = font.getbbox(text)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = bx + (bw - tw) // 2
    ty = by + (bh - th) // 2 - 1
    draw.text((tx, ty), text, fill=text_color, font=font)


def wrap_text_to_width(
    text: str, font: ImageFont.ImageFont, max_width: int
) -> list[str]:
    """Wraps text into lines that each fit within max_width pixels."""
    if not text:
        return []
    bbox = font.getbbox(text)
    if bbox[2] - bbox[0] <= max_width:
        return [text]

    # If the text has spaces, wrap words first
    if " " in text:
        words = text.split(" ")
        lines = []
        curr_line = ""
        for word in words:
            candidate = f"{curr_line} {word}".strip() if curr_line else word
            w = font.getbbox(candidate)[2] - font.getbbox(candidate)[0]
            if w <= max_width:
                curr_line = candidate
            else:
                if curr_line:
                    lines.append(curr_line)
                w_word = font.getbbox(word)[2] - font.getbbox(word)[0]
                if w_word > max_width:
                    sub_lines = wrap_text_to_width(word, font, max_width)
                    lines.extend(sub_lines[:-1])
                    curr_line = sub_lines[-1]
                else:
                    curr_line = word
        lines.append(curr_line)
        return lines

    # For a single long token with hyphens (e.g. GUID):
    if "-" in text:
        parts = text.split("-")
        tokens = [p + "-" for p in parts[:-1]] + [parts[-1]]
        best_split = None
        best_diff = float("inf")
        for i in range(1, len(tokens)):
            line1 = "".join(tokens[:i])
            line2 = "".join(tokens[i:])
            w1 = font.getbbox(line1)[2] - font.getbbox(line1)[0]
            w2 = font.getbbox(line2)[2] - font.getbbox(line2)[0]
            if w1 <= max_width and w2 <= max_width:
                diff = abs(w1 - w2)
                if diff < best_diff:
                    best_diff = diff
                    best_split = [line1, line2]
        if best_split:
            return best_split

        lines = []
        curr = ""
        for tok in tokens:
            cand = curr + tok
            w = font.getbbox(cand)[2] - font.getbbox(cand)[0]
            if curr and w > max_width:
                lines.append(curr)
                curr = tok
            else:
                curr = cand
        lines.append(curr)
        return lines

    # Character-based wrap fallback
    lines = []
    curr = ""
    for ch in text:
        w = font.getbbox(curr + ch)[2] - font.getbbox(curr + ch)[0]
        if w > max_width:
            if curr:
                lines.append(curr)
            curr = ch
        else:
            curr += ch
    lines.append(curr)
    return lines


def generate_gui_welcome(fonts: dict) -> Image.Image:
    """Generates Native Desktop GUI Welcome Dialog (`gui_welcome_dialog.png`)."""
    w, h = 540, 420
    im = Image.new("RGBA", (w, h), (248, 249, 250, 255))
    draw = ImageDraw.Draw(im)

    # Title bar
    draw_window_titlebar(draw, w, h, "ExampleApp Setup - msi-gui", fonts, dark=False)

    # Left banner (WiX Mondo wizard banner: w=164, y=33 to h-50)
    banner_w = 164
    banner_bottom = h - 50

    # Gradient background for sidebar
    for y in range(33, banner_bottom):
        ratio = (y - 33) / (banner_bottom - 33)
        r = int(0 * (1 - ratio) + 8 * ratio)
        g = int(82 * (1 - ratio) + 40 * ratio)
        b = int(170 * (1 - ratio) + 110 * ratio)
        draw.line([0, y, banner_w, y], fill=(r, g, b, 255))

    # Top accent highlight on sidebar
    draw.rectangle([0, 33, banner_w, 115], fill=(0, 95, 195, 160))

    # Subtle decorative geometric grid / box on banner
    draw.polygon([(40, 50), (124, 50), (144, 70), (60, 70)], fill=(255, 255, 255, 30))
    draw.polygon([(40, 50), (60, 70), (60, 110), (40, 90)], fill=(255, 255, 255, 20))
    draw.polygon([(60, 70), (144, 70), (144, 110), (60, 110)], fill=(255, 255, 255, 45))

    # Banner branding text
    draw.text((22, 135), "msi-rs", fill=(255, 255, 255, 255), font=fonts["title"])
    draw.text(
        (22, 160),
        "Windows Installer",
        fill=(210, 230, 255, 255),
        font=fonts["small_bold"],
    )
    draw.text(
        (22, 178),
        "Cross-Platform Engine",
        fill=(175, 205, 245, 255),
        font=fonts["small"],
    )
    draw.text(
        (22, 194), "Native Desktop GUI", fill=(150, 185, 230, 255), font=fonts["small"]
    )

    # WiX Mondo badge in banner
    draw.rounded_rectangle(
        [22, 315, banner_w - 22, 345],
        radius=4,
        fill=(255, 255, 255, 25),
        outline=(255, 255, 255, 60),
    )
    draw.text(
        (28, 323), "Theme: WixUI_Mondo", fill=(230, 240, 255, 255), font=fonts["small"]
    )

    # Content area (Right side)
    cx = banner_w + 20
    content_w = w - cx - 20

    draw.text(
        (cx, 42),
        "ExampleApp Setup Wizard",
        fill=(20, 25, 35, 255),
        font=fonts["heading"],
    )
    draw.text(
        (cx, 60),
        "Please review the license terms and package details before continuing.",
        fill=(80, 85, 95, 255),
        font=fonts["small"],
    )
    draw.line([cx, 75, cx + content_w, 75], fill=(225, 228, 234, 255), width=1)

    meta_items = [
        ("Target Platform:", "x64 / POSIX (Translated)"),
        ("Package GUID:", "{9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}"),
        ("Product Version:", "1.2.0"),
        ("Manufacturer:", "Contoso Systems Inc."),
    ]
    max_val_w = (cx + content_w - 12) - (cx + 105)
    line_h = 13
    item_gap = 15

    # Calculate card height dynamically from wrapped lines
    wrapped_meta = []
    total_extra_lines = 0
    for label, val in meta_items:
        vlines = wrap_text_to_width(val, fonts["small_bold"], max_val_w)
        wrapped_meta.append((label, vlines))
        total_extra_lines += len(vlines) - 1

    # Metadata Card
    card_y = 82
    card_h = 88 + total_extra_lines * line_h
    draw.rounded_rectangle(
        [cx, card_y, cx + content_w, card_y + card_h],
        radius=5,
        fill=(242, 244, 247, 255),
        outline=(220, 224, 230, 255),
    )
    draw.text(
        (cx + 10, card_y + 6),
        "Installation Package Details",
        fill=(30, 35, 45, 255),
        font=fonts["small_bold"],
    )
    draw.line(
        [cx + 10, card_y + 22, cx + content_w - 10, card_y + 22],
        fill=(228, 231, 238, 255),
        width=1,
    )

    my = card_y + 28
    for label, vlines in wrapped_meta:
        draw.text((cx + 10, my), label, fill=(100, 105, 115, 255), font=fonts["small"])
        for idx, vline in enumerate(vlines):
            draw.text(
                (cx + 105, my + idx * line_h),
                vline,
                fill=(35, 40, 50, 255),
                font=fonts["small_bold"],
            )
        my += item_gap + (len(vlines) - 1) * line_h

    # ScrollableText EULA Component (Standard Windows Installer control)
    st_label_y = card_y + card_h + 12
    draw.text(
        (cx, st_label_y),
        "End-User License Agreement (ScrollableText):",
        fill=(30, 35, 45, 255),
        font=fonts["small_bold"],
    )
    box_y = st_label_y + 18
    st_h = 110
    draw.rounded_rectangle(
        [cx, box_y, cx + content_w, box_y + st_h],
        radius=3,
        fill=(255, 255, 255, 255),
        outline=(190, 195, 205, 255),
    )

    # Scrollbar on right edge of ScrollableText
    sb_w = 14
    sb_x = cx + content_w - sb_w
    draw.rectangle(
        [sb_x, box_y + 1, cx + content_w - 1, box_y + st_h - 1],
        fill=(243, 245, 248, 255),
    )
    draw.line(
        [sb_x, box_y + 1, sb_x, box_y + st_h - 1], fill=(215, 220, 228, 255), width=1
    )
    draw.rounded_rectangle(
        [sb_x + 2, box_y + 14, cx + content_w - 3, box_y + 48],
        radius=2,
        fill=(195, 200, 210, 255),
    )
    draw.polygon(
        [(sb_x + 7, box_y + 4), (sb_x + 4, box_y + 9), (sb_x + 10, box_y + 9)],
        fill=(130, 135, 145, 255),
    )
    draw.polygon(
        [
            (sb_x + 7, box_y + st_h - 4),
            (sb_x + 4, box_y + st_h - 9),
            (sb_x + 10, box_y + st_h - 9),
        ],
        fill=(130, 135, 145, 255),
    )

    available_text_w = content_w - sb_w - 16
    raw_eula = (
        'Apache License, Version 2.0 (the "License"); you may not use this file except in '
        "compliance with the License. You may obtain a copy of the License at:\n"
        "http://www.apache.org/licenses/LICENSE-2.0\n\n"
        "Unless required by applicable law or agreed to in writing, software distributed under "
        'the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS '
        "OF ANY KIND, either express or implied."
    )
    wrapped_eula = []
    for p in raw_eula.split("\n"):
        if not p.strip():
            wrapped_eula.append("")
        else:
            wrapped_eula.extend(wrap_text_to_width(p, fonts["small"], available_text_w))

    ey = box_y + 8
    for el in wrapped_eula:
        if ey + 12 > box_y + st_h:
            break
        if el:
            draw.text((cx + 8, ey), el, fill=(55, 60, 70, 255), font=fonts["small"])
        ey += 14

    # Bottom button bar
    draw.line(
        [0, banner_bottom, w - 1, banner_bottom], fill=(215, 218, 224, 255), width=1
    )
    draw.rectangle([0, banner_bottom + 1, w - 1, h - 1], fill=(240, 242, 245, 255))

    btn_y = h - 38
    draw_button(
        draw, [w - 265, btn_y, 75, 26], "< Back", fonts["body"], is_disabled=True
    )
    draw_button(
        draw, [w - 180, btn_y, 75, 26], "I Agree", fonts["body"], is_primary=True
    )
    draw_button(draw, [w - 95, btn_y, 75, 26], "Cancel", fonts["body"])

    return im


def generate_gui_feature_tree(fonts: dict) -> Image.Image:
    """Generates Native Desktop GUI Feature Tree Selection Dialog (`gui_feature_tree_dialog.png`)."""
    w, h = 540, 420
    im = Image.new("RGBA", (w, h), (248, 249, 250, 255))
    draw = ImageDraw.Draw(im)

    # Title bar
    draw_window_titlebar(draw, w, h, "ExampleApp Setup - msi-gui", fonts, dark=False)

    # Top banner (Classic WiX top banner: y=33 to y=92)
    banner_bottom = 92
    draw.rectangle([0, 33, w - 1, banner_bottom], fill=(255, 255, 255, 255))
    draw.line(
        [0, banner_bottom, w - 1, banner_bottom], fill=(215, 218, 224, 255), width=1
    )

    draw.text((22, 44), "Custom Setup", fill=(20, 25, 35, 255), font=fonts["heading"])
    draw.text(
        (22, 65),
        "Select the way you want features to be installed.",
        fill=(90, 95, 105, 255),
        font=fonts["small"],
    )

    # Top banner right icon (package box)
    icon_x = w - 60
    draw.polygon(
        [(icon_x + 10, 45), (icon_x + 35, 45), (icon_x + 45, 55), (icon_x + 20, 55)],
        fill=(0, 102, 204, 180),
    )
    draw.polygon(
        [(icon_x + 10, 45), (icon_x + 20, 55), (icon_x + 20, 80), (icon_x + 10, 70)],
        fill=(0, 82, 163, 180),
    )
    draw.polygon(
        [(icon_x + 20, 55), (icon_x + 45, 55), (icon_x + 45, 80), (icon_x + 20, 80)],
        fill=(0, 122, 240, 180),
    )

    # Feature Tree Box (Left: x=20, y=105, w=285, h=215)
    tree_x, tree_y, tree_w, tree_h = 20, 105, 285, 215
    draw.rounded_rectangle(
        [tree_x, tree_y, tree_x + tree_w, tree_y + tree_h],
        radius=4,
        fill=(255, 255, 255, 255),
        outline=(210, 215, 222, 255),
    )

    tree_items = [
        ("▼  [■]  ExampleApp Core Runtime", 0, True),
        ("     [■]  Server Daemon & Service", 1, False),
        ("     [■]  Shared Libraries (.dylib / .so)", 1, False),
        ("▼  [■]  Command Line Utilities", 0, False),
        ("     [■]  msi-cli Execution Engine", 1, False),
        ("     [■]  Management Tools & Daemons", 1, False),
        ("►  [ ]  Documentation & SDK Samples", 0, False),
        ("►  [ ]  Debugging Symbols & Tools", 0, False),
    ]

    for idx, (label, depth, is_selected) in enumerate(tree_items):
        iy = tree_y + 6 + idx * 25
        if is_selected:
            draw.rounded_rectangle(
                [tree_x + 3, iy - 2, tree_x + tree_w - 3, iy + 21],
                radius=3,
                fill=(225, 238, 255, 255),
                outline=(180, 210, 255, 255),
                width=1,
            )
            draw.text(
                (tree_x + 10, iy + 2),
                label,
                fill=(0, 82, 180, 255),
                font=fonts["body_bold"],
            )
        else:
            text_color = (40, 45, 55, 255) if "[■]" in label else (130, 135, 145, 255)
            draw.text((tree_x + 10, iy + 2), label, fill=text_color, font=fonts["body"])

    # Feature Description Card (Right Top: x=318, y=105, w=202, h=135)
    desc_x, desc_y, desc_w, desc_h = 318, 105, 202, 135
    draw.rounded_rectangle(
        [desc_x, desc_y, desc_x + desc_w, desc_y + desc_h],
        radius=4,
        fill=(244, 246, 249, 255),
        outline=(218, 222, 228, 255),
    )
    draw.text(
        (desc_x + 12, desc_y + 10),
        "Feature Description:",
        fill=(25, 30, 40, 255),
        font=fonts["small_bold"],
    )
    draw.line(
        [desc_x + 12, desc_y + 26, desc_x + desc_w - 12, desc_y + 26],
        fill=(228, 231, 238, 255),
        width=1,
    )

    desc_lines = [
        "Core execution binaries and",
        "runtime daemon services.",
        "",
        "Size: 42.8 MB",
        "Location: /opt/contoso/app",
        "Installed locally on hard drive.",
    ]
    dy = desc_y + 32
    for line in desc_lines:
        draw.text((desc_x + 12, dy), line, fill=(70, 75, 85, 255), font=fonts["small"])
        dy += 15

    # Disk Space Card (Right Bottom: x=318, y=250, w=202, h=70)
    space_y, space_h = 250, 70
    draw.rounded_rectangle(
        [desc_x, space_y, desc_x + desc_w, space_y + space_h],
        radius=4,
        fill=(244, 246, 249, 255),
        outline=(218, 222, 228, 255),
    )
    draw.text(
        (desc_x + 12, space_y + 8),
        "Disk Space Allocation:",
        fill=(25, 30, 40, 255),
        font=fonts["small_bold"],
    )
    draw.line(
        [desc_x + 12, space_y + 24, desc_x + desc_w - 12, space_y + 24],
        fill=(228, 231, 238, 255),
        width=1,
    )

    draw.text(
        (desc_x + 12, space_y + 30),
        "Disk Space Required:",
        fill=(100, 105, 115, 255),
        font=fonts["small"],
    )
    draw.text(
        (desc_x + 130, space_y + 30),
        "68.4 MB",
        fill=(30, 35, 45, 255),
        font=fonts["small_bold"],
    )

    draw.text(
        (desc_x + 12, space_y + 48),
        "Disk Space Available:",
        fill=(100, 105, 115, 255),
        font=fonts["small"],
    )
    draw.text(
        (desc_x + 130, space_y + 48),
        "142.1 GB",
        fill=(0, 135, 90, 255),
        font=fonts["small_bold"],
    )

    # Bottom button bar
    bot_y = h - 50
    draw.line([0, bot_y, w - 1, bot_y], fill=(215, 218, 224, 255), width=1)
    draw.rectangle([0, bot_y + 1, w - 1, h - 1], fill=(240, 242, 245, 255))

    btn_y = h - 38
    draw_button(draw, [w - 265, btn_y, 75, 26], "< Back", fonts["body"])
    draw_button(
        draw, [w - 180, btn_y, 75, 26], "Install", fonts["body"], is_primary=True
    )
    draw_button(draw, [w - 95, btn_y, 75, 26], "Cancel", fonts["body"])

    return im


def generate_tui_wizard(fonts: dict) -> Image.Image:
    """Generates Terminal TUI Wizard screenshot (`tui_wizard.png`)."""
    w, h = 680, 360
    im = Image.new("RGBA", (w, h), (26, 27, 38, 255))  # Tokyo Night style dark
    draw = ImageDraw.Draw(im)

    # Window title bar
    draw_window_titlebar(
        draw, w, h, "bash — msi-gui --backend tui (80x24)", fonts, dark=True
    )

    lines = [
        (
            "┌───────────────── ExampleApp Setup (Terminal Wizard) ─────────────────┐",
            "border",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "│   Welcome to ExampleApp cross-platform installer wizard!             │",
            "text",
        ),
        (
            "│   Package: example-app-1.2.0.msi                                     │",
            "dim",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "│   End-User License Agreement: Apache-2.0 / MIT Dual-License          │",
            "accent",
        ),
        (
            "│       [X] Create desktop shortcut and Freedesktop .desktop entry     │",
            "text",
        ),
        (
            "│       [ ] Register as background system service / daemon             │",
            "dim",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "│   Installation Directory:                                            │",
            "text",
        ),
        (
            "│     > [ /opt/contoso/exampleapp                                    ] │",
            "path",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "│   Installation Progress:                                             │",
            "text",
        ),
        (
            "│   [██████████████████████████░░░░░░░░] 78% (Extracting .cab)         │",
            "progress",
        ),
        (
            "│   Action: InstallFiles -> /opt/contoso/exampleapp/bin/server         │",
            "action",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "│   [ < Back ]            [> I Agree ]            [ Cancel ]           │",
            "nav",
        ),
        (
            "│                                                                      │",
            "border",
        ),
        (
            "└──────────────────────────────────────────────────────────────────────┘",
            "border",
        ),
    ]

    y = 44
    for line, kind in lines:
        if kind == "border":
            draw.text((28, y), line, fill=(90, 110, 140, 255), font=fonts["mono"])
        elif kind == "progress":
            # Highlight progress bar
            draw.text((28, y), line, fill=(120, 220, 140, 255), font=fonts["mono"])
        elif kind == "accent":
            draw.text((28, y), line, fill=(122, 162, 247, 255), font=fonts["mono_bold"])
        elif kind == "nav":
            draw.text((28, y), line, fill=(187, 154, 247, 255), font=fonts["mono_bold"])
        elif kind == "path":
            draw.text((28, y), line, fill=(224, 175, 104, 255), font=fonts["mono"])
        elif kind == "action":
            draw.text((28, y), line, fill=(158, 206, 106, 255), font=fonts["mono"])
        elif kind == "dim":
            draw.text((28, y), line, fill=(140, 145, 160, 255), font=fonts["mono"])
        else:
            draw.text((28, y), line, fill=(192, 202, 245, 255), font=fonts["mono"])
        y += 16

    return im


def generate_cli_workflow(fonts: dict) -> Image.Image:
    """Generates Command-Line Installation Execution screenshot (`cli_install_output.png`)."""
    w, h = 680, 360
    im = Image.new("RGBA", (w, h), (24, 25, 32, 255))
    draw = ImageDraw.Draw(im)

    # Window title bar
    draw_window_titlebar(
        draw,
        w,
        h,
        "zsh — msi-cli install ./ExampleApp.msi /qb /lvx ./install.log",
        fonts,
        dark=True,
    )

    cli_output = [
        (
            "$ msi-cli install ./ExampleApp.msi /qb /lvx ./install.log",
            (255, 255, 255, 255),
            True,
        ),
        (
            "[INFO] Validating Compound File Binary Format container...",
            (122, 162, 247, 255),
            False,
        ),
        (
            "[INFO] Loaded database catalogs: 48 tables, 312 columns",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[INFO] Executing CostInitialize -> FileCost -> CostFinalize",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[INFO] Volume '/': Required: 68.4 MB | Free: 142.1 GB (Available)",
            (120, 220, 140, 255),
            False,
        ),
        (
            "[INFO] Preparing 2-phase transaction script (42 operations)",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[WORKER] Transitioning to privileged worker context over IPC...",
            (224, 175, 104, 255),
            False,
        ),
        (
            "[WORKER] Unpacking cabinet '#cab1.cab' with LZX decompression...",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[WORKER] Writing atomic files with quarantine rollback protection (.rbf)",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[WORKER] Translating POSIX permissions: mode 0755, owner root:wheel",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[WORKER] Generating launchd daemon plist: /Library/LaunchDaemons/...",
            (170, 175, 190, 255),
            False,
        ),
        (
            r"[WORKER] Registering HKLM\Software\Contoso\ExampleApp in SQLite store",
            (170, 175, 190, 255),
            False,
        ),
        (
            "[SUCCESS] Installation completed successfully (Exit Code: 0)",
            (120, 235, 140, 255),
            True,
        ),
        ("", (0, 0, 0, 0), False),
        ("$ msi-cli info ./ExampleApp.msi", (255, 255, 255, 255), True),
        (
            "Package: ExampleApp 1.2.0 | GUID: {9A1F4C3B-2D6E-4F8A-9C7B-1E3D5F7A9B0C}",
            (190, 195, 210, 255),
            False,
        ),
        (
            "Platform: x64;1033;POSIX | Schema Version: 500 | Compressed: Yes",
            (190, 195, 210, 255),
            False,
        ),
    ]

    y = 42
    for text, color, is_bold in cli_output:
        font = fonts["mono_bold"] if is_bold else fonts["mono"]
        if text.startswith("$"):
            # Prompt accent
            draw.text((24, y), "$ ", fill=(122, 162, 247, 255), font=fonts["mono_bold"])
            draw.text(
                (38, y), text[2:], fill=(255, 255, 255, 255), font=fonts["mono_bold"]
            )
        elif text.startswith("[SUCCESS]"):
            draw.text((24, y), text, fill=(120, 235, 140, 255), font=fonts["mono_bold"])
        elif text.startswith("[WORKER]"):
            draw.text(
                (24, y), "[WORKER]", fill=(224, 175, 104, 255), font=fonts["mono_bold"]
            )
            draw.text(
                (24 + 68, y), text[8:], fill=(170, 175, 190, 255), font=fonts["mono"]
            )
        elif text.startswith("[INFO]"):
            draw.text(
                (24, y), "[INFO]", fill=(122, 162, 247, 255), font=fonts["mono_bold"]
            )
            if "Available" in text:
                draw.text(
                    (24 + 52, y),
                    text[6:],
                    fill=(120, 220, 140, 255),
                    font=fonts["mono"],
                )
            else:
                draw.text(
                    (24 + 52, y),
                    text[6:],
                    fill=(170, 175, 190, 255),
                    font=fonts["mono"],
                )
        else:
            draw.text((24, y), text, fill=color, font=font)
        y += 18

    return im


def main():
    """Generates all screenshots and saves them to target locations."""
    fonts = get_fonts()

    images = {
        "gui_welcome_dialog.png": generate_gui_welcome(fonts),
        "gui_feature_tree_dialog.png": generate_gui_feature_tree(fonts),
        "tui_wizard.png": generate_tui_wizard(fonts),
        "cli_install_output.png": generate_cli_workflow(fonts),
    }

    # Output targets
    repo_root = Path(__file__).resolve().parent.parent
    target_dirs = [
        repo_root.parent / "cc0-assets" / "msi-rs" / "screenshots",
    ]

    for target_dir in target_dirs:
        target_dir.mkdir(parents=True, exist_ok=True)
        for name, img in images.items():
            out_path = target_dir / name
            img.save(out_path, format="PNG", optimize=True)
            print(f"Generated {out_path} ({img.width}x{img.height})")


if __name__ == "__main__":
    main()
