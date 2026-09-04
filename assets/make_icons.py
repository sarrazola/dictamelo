#!/usr/bin/env python3
"""Genera el ícono de la app (1024x1024) y los íconos de la barra de menú.

Uso:  python3 assets/make_icons.py
Luego: npx tauri icon assets/app-icon-1024.png
"""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
TRAY_DIR = ROOT / "src-tauri" / "icons" / "tray"
TRAY_DIR.mkdir(parents=True, exist_ok=True)


def mic_glyph(draw: ImageDraw.ImageDraw, cx: float, cy: float, size: float, fill):
    """Dibuja un micrófono centrado en (cx, cy); `size` es la altura total."""
    w = size * 0.34
    body_h = size * 0.52
    top = cy - size * 0.48
    # Cápsula del micrófono
    draw.rounded_rectangle(
        [cx - w / 2, top, cx + w / 2, top + body_h], radius=w / 2, fill=fill
    )
    # Arco de soporte
    arc_w = size * 0.62
    arc_top = top + body_h * 0.42
    arc_bottom = top + body_h + size * 0.16
    thick = max(1, int(size * 0.075))
    draw.arc(
        [cx - arc_w / 2, arc_top, cx + arc_w / 2, arc_bottom],
        start=0, end=180, fill=fill, width=thick,
    )
    # Mástil y base
    stem_top = arc_bottom - size * 0.02
    stem_bottom = cy + size * 0.42
    draw.rectangle([cx - thick / 2, stem_top, cx + thick / 2, stem_bottom], fill=fill)
    base_w = size * 0.36
    draw.rounded_rectangle(
        [cx - base_w / 2, stem_bottom - thick / 2, cx + base_w / 2, stem_bottom + thick / 2],
        radius=thick / 2, fill=fill,
    )


def app_icon(path: Path, px: int = 1024):
    scale = 4
    big = px * scale
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    margin = big * 0.06
    radius = big * 0.22
    # Fondo con degradado vertical (azul-violeta)
    grad = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    gd = ImageDraw.Draw(grad)
    top_c = (78, 88, 255)
    bot_c = (140, 60, 220)
    for y in range(big):
        t = y / (big - 1)
        c = tuple(int(top_c[i] * (1 - t) + bot_c[i] * t) for i in range(3)) + (255,)
        gd.line([(0, y), (big, y)], fill=c)
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [margin, margin, big - margin, big - margin], radius=radius, fill=255
    )
    img.paste(grad, (0, 0), mask)
    # Micrófono blanco
    mic_glyph(d, big / 2, big / 2 + big * 0.02, big * 0.52, (255, 255, 255, 255))
    img = img.resize((px, px), Image.LANCZOS)
    img.save(path)


def tray_icon(name: str, px: int, fg, badge=None, template=True):
    """Ícono de barra de menú. `px` es el tamaño en píxeles (usar 2x para retina)."""
    scale = 4
    big = px * scale
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if badge is not None:
        # Disco de color con micrófono blanco encima (íconos de estado, no template)
        d.ellipse([0, 0, big - 1, big - 1], fill=badge)
        mic_glyph(d, big / 2, big / 2, big * 0.62, (255, 255, 255, 255))
    else:
        mic_glyph(d, big / 2, big / 2, big * 0.92, fg)
    img = img.resize((px, px), Image.LANCZOS)
    img.save(TRAY_DIR / f"{name}.png")


if __name__ == "__main__":
    out = ROOT / "assets" / "app-icon-1024.png"
    app_icon(out)
    print("app icon ->", out)
    black = (0, 0, 0, 255)
    # 36 px = 18 pt @2x (tray-icon escala a 18 pt de alto en macOS)
    tray_icon("idle", 36, black)
    tray_icon("recording", 36, black, badge=(228, 52, 52, 255))
    tray_icon("transcribing", 36, black, badge=(56, 120, 250, 255))
    tray_icon("pasting", 36, black, badge=(40, 170, 90, 255))
    tray_icon("error", 36, black, badge=(240, 140, 20, 255))
    print("tray icons ->", TRAY_DIR)
