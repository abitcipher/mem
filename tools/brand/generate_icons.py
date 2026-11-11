#!/usr/bin/env python3
"""Generates the app icons from assets/brand/logo-512x512.png (export of assets/brand/logo.svg).

Run from the repo root with Pillow installed:  python3 -I tools/brand/generate_icons.py
Outputs are committed; re-run after changing the logo.

The logo is an orange disc with the "m!" cut out (transparent). Derived images:
- logo-filled: the "m!" filled white, so it reads the same on dark backgrounds;
- glyph-white: the "m!" alone, for the adaptive-icon foreground and the themed (monochrome) icon.
"""
import os

from PIL import Image, ImageChops, ImageDraw

SRC = 'assets/brand/logo-512x512.png'
RES = 'app/android/app/src/main/res'
DENSITIES = {'mdpi': 1, 'hdpi': 1.5, 'xhdpi': 2, 'xxhdpi': 3, 'xxxhdpi': 4}


def disc(inset):
    """Anti-aliased mask of the logo's disc (opaque box 16..496 of 512), inset by `inset` px."""
    m = Image.new('L', (2048, 2048), 0)
    lo, hi = 4 * (16 + inset), 4 * (496 - inset) - 1
    ImageDraw.Draw(m).ellipse((lo, lo, hi, hi), fill=255)
    return m.resize((512, 512), Image.LANCZOS)


def fit(img, size, frac):
    """`img` scaled so its 480 px disc spans `frac` of a size×size transparent canvas, centred."""
    s = round(size * frac * 512 / 480)
    canvas = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    canvas.alpha_composite(img.resize((s, s), Image.LANCZOS), ((size - s) // 2, (size - s) // 2))
    return canvas


def main():
    logo = Image.open(SRC).convert('RGBA')
    filled = Image.new('RGBA', (512, 512), (255, 255, 255, 0))
    filled.putalpha(disc(3))  # inset so no white fringe shows at the anti-aliased rim
    filled.alpha_composite(logo)
    glyph_alpha = ImageChops.multiply(Image.eval(logo.getchannel('A'), lambda v: 255 - v), disc(6))
    glyph = Image.new('RGBA', (512, 512), (255, 255, 255, 255))
    glyph.putalpha(glyph_alpha)

    os.makedirs('assets/brand/derived', exist_ok=True)
    filled.save('assets/brand/derived/logo-filled-512.png')
    glyph.save('assets/brand/derived/glyph-white-512.png')
    for name, f in DENSITIES.items():
        mip = f'{RES}/mipmap-{name}'
        os.makedirs(mip, exist_ok=True)
        # Classic icon (Android 7.0-7.1): the round logo with a small margin.
        fit(filled, round(48 * f), 44 / 48).save(f'{mip}/ic_launcher.png')
        fit(filled, round(48 * f), 44 / 48).save(f'{mip}/ic_launcher_round.png')
        # Adaptive layers (Android 8+): 108 dp canvas, 72 dp visible; the disc spans the visible area,
        # so any mask shape shows the logo at its own proportions on the orange background.
        fit(glyph, round(108 * f), 72 / 108).save(f'{mip}/ic_launcher_foreground.png')
        fit(glyph, round(108 * f), 72 / 108).save(f'{mip}/ic_launcher_monochrome.png')
        draw = f'{RES}/drawable-{name}'
        os.makedirs(draw, exist_ok=True)
        fit(filled, round(128 * f), 1.0).save(f'{draw}/launch_logo.png')  # launch screen before Android 12
    os.makedirs('app/assets/brand', exist_ok=True)
    filled.resize((256, 256), Image.LANCZOS).save('app/assets/brand/logo.png')


if __name__ == '__main__':
    main()
