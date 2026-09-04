#!/usr/bin/env python3
"""Genera todos los íconos a partir del logo original (assets/logo-original.png).

- Ícono de app (1024×1024) al estilo macOS: cuadrado redondeado con márgenes.
- Íconos de la barra de menú: el mismo dibujo (perfil + líneas de sonido) en monocromo
  (plantilla, se adapta al modo claro/oscuro) y tintado por estado.
- ui/icon.png para la barra lateral y «Acerca de».

Uso:  python3 assets/make_icons.py && npx tauri icon assets/app-icon-1024.png
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter
import numpy as np

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "assets" / "logo-original.png"
TRAY_DIR = ROOT / "src-tauri" / "icons" / "tray"
TRAY_DIR.mkdir(parents=True, exist_ok=True)


def app_icon(path: Path, px: int = 1024):
    """Cuadrado redondeado (la forma que usa macOS) con el logo a sangre y margen del 10 %."""
    logo = Image.open(LOGO).convert("RGB")
    canvas = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    margin = round(px * 0.098)          # cuadrícula de Apple: 824 px de 1024
    size = px - 2 * margin
    radius = round(size * 0.2237)       # radio de esquina de los íconos de macOS
    content = logo.resize((size, size), Image.LANCZOS)
    # Máscara con antialiasing: se dibuja a 4× y se reduce.
    scale = 4
    mask = Image.new("L", (size * scale, size * scale), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, size * scale - 1, size * scale - 1], radius=radius * scale, fill=255)
    mask = mask.resize((size, size), Image.LANCZOS)
    canvas.paste(content, (margin, margin), mask)
    canvas.save(path)


def glyph_alpha() -> Image.Image:
    """Máscara (canal alfa) del trazo negro del logo: perfil + líneas, recortado a la parte central."""
    im = Image.open(LOGO).convert("RGB")
    lum = np.asarray(im).astype(np.float32).mean(axis=2)
    # Transición suave alrededor del umbral para conservar el antialiasing del trazo.
    alpha = np.clip((120.0 - lum) / 60.0, 0.0, 1.0) * 255.0
    mask = Image.fromarray(alpha.astype(np.uint8), "L")
    # Recorte: de la punta de la nariz hasta el final de las líneas de sonido.
    crop = mask.crop((270, 290, 1170, 1070))
    return crop


def tray_icon(name: str, color, height_px: int = 36):
    """PNG con el dibujo en `color` y el alfa del trazo, de `height_px` de alto (18 pt @2x)."""
    alpha = glyph_alpha()
    w, h = alpha.size
    width_px = round(height_px * w / h)
    alpha = alpha.resize((width_px, height_px), Image.LANCZOS)
    rgba = Image.new("RGBA", (width_px, height_px), color + (0,))
    rgba.putalpha(alpha)
    rgba.save(TRAY_DIR / f"{name}.png")
    return width_px


if __name__ == "__main__":
    out = ROOT / "assets" / "app-icon-1024.png"
    app_icon(out)
    print("app icon ->", out)
    (ROOT / "ui").mkdir(exist_ok=True)
    Image.open(out).resize((256, 256), Image.LANCZOS).save(ROOT / "ui" / "icon.png")
    print("ui/icon.png listo")
    w = tray_icon("idle", (0, 0, 0))               # plantilla: macOS la tiñe según el modo
    tray_icon("recording", (228, 52, 52))
    tray_icon("transcribing", (56, 120, 250))
    tray_icon("pasting", (40, 170, 90))
    tray_icon("error", (240, 140, 20))
    print(f"tray icons -> {TRAY_DIR} ({w}×36 px)")
