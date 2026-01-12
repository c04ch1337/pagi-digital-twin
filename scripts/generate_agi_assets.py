"""Generate high-res Ferrellgas AGI badge + favicon.

Outputs:
- frontend-digital-twin/public/ferrellgas-agi-badge.png (512x512)
- frontend-digital-twin/public/ferrellgas-agi-badge.svg (vector fallback)
- frontend-digital-twin/public/favicon.ico (multi-size)
- frontend-digital-twin/public/favicon-32.png (32x32)

Requires: Pillow
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parent.parent
PUBLIC_DIR = ROOT / "frontend-digital-twin" / "public"


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def lerp_rgb(c1: tuple[int, int, int], c2: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return (
        int(lerp(c1[0], c2[0], t)),
        int(lerp(c1[1], c2[1], t)),
        int(lerp(c1[2], c2[2], t)),
    )


def radial_gradient(size: int, inner: tuple[int, int, int], outer: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = img.load()
    cx = (size - 1) / 2
    cy = (size - 1) / 2
    max_r = math.sqrt(cx * cx + cy * cy)

    for y in range(size):
        for x in range(size):
            r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2) / max_r
            r = max(0.0, min(1.0, r))
            col = lerp_rgb(inner, outer, r)
            px[x, y] = (*col, 255)

    return img


def make_badge_png(path: Path, size: int = 512) -> Image.Image:
    # Indigo badge with subtle radial gradient + ring + monogram.
    bg = radial_gradient(size, (99, 102, 241), (30, 27, 75))

    mask = Image.new("L", (size, size), 0)
    mask_draw = ImageDraw.Draw(mask)
    pad = int(size * 0.04)
    mask_draw.ellipse((pad, pad, size - pad, size - pad), fill=255)

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    img.paste(bg, (0, 0), mask)

    draw = ImageDraw.Draw(img)

    # Outer ring
    ring_w = max(4, int(size * 0.035))
    ring_pad = pad + ring_w // 2
    draw.ellipse(
        (ring_pad, ring_pad, size - ring_pad, size - ring_pad),
        outline=(255, 255, 255, 60),
        width=ring_w,
    )

    # Inner glow
    glow = radial_gradient(size, (255, 255, 255), (99, 102, 241))
    glow_mask = Image.new("L", (size, size), 0)
    glow_draw = ImageDraw.Draw(glow_mask)
    glow_pad = int(size * 0.22)
    glow_draw.ellipse((glow_pad, glow_pad, size - glow_pad, size - glow_pad), fill=60)
    # Pillow's alpha_composite doesn't accept a separate mask; use paste with mask instead.
    img.paste(glow, (0, 0), glow_mask)

    # Monogram: "F"
    try:
        # Best effort: use a system font if available.
        font = ImageFont.truetype("arialbd.ttf", int(size * 0.52))
    except Exception:
        font = ImageFont.load_default()

    text = "F"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (size - tw) // 2
    ty = (size - th) // 2 - int(size * 0.03)

    # Shadow
    draw.text((tx + int(size * 0.01), ty + int(size * 0.02)), text, font=font, fill=(0, 0, 0, 90))
    # Foreground
    draw.text((tx, ty), text, font=font, fill=(255, 255, 255, 235))

    img.save(path)
    return img


def write_svg(path: Path) -> None:
    svg = """<svg xmlns='http://www.w3.org/2000/svg' width='512' height='512' viewBox='0 0 512 512'>
  <defs>
    <radialGradient id='g' cx='40%' cy='35%' r='70%'>
      <stop offset='0%' stop-color='#6366f1'/>
      <stop offset='100%' stop-color='#1e1b4b'/>
    </radialGradient>
  </defs>
  <circle cx='256' cy='256' r='236' fill='url(#g)'/>
  <circle cx='256' cy='256' r='212' fill='none' stroke='rgba(255,255,255,0.25)' stroke-width='18'/>
  <text x='256' y='312' text-anchor='middle' font-family='Inter, Space Grotesk, Arial Black, sans-serif' font-size='280' font-weight='800' fill='rgba(255,255,255,0.92)'>F</text>
</svg>"""
    path.write_text(svg, encoding="utf-8")


def write_favicon(large: Image.Image, ico_path: Path, png32_path: Path) -> None:
    sizes = [16, 24, 32, 48, 64, 128, 256]
    images = [large.resize((s, s), Image.Resampling.LANCZOS) for s in sizes]
    images[2].save(png32_path)
    images[0].save(ico_path, format="ICO", sizes=[(s, s) for s in sizes])


def main() -> None:
    PUBLIC_DIR.mkdir(parents=True, exist_ok=True)

    badge_png = PUBLIC_DIR / "ferrellgas-agi-badge.png"
    badge_svg = PUBLIC_DIR / "ferrellgas-agi-badge.svg"
    favicon_ico = PUBLIC_DIR / "favicon.ico"
    favicon_32 = PUBLIC_DIR / "favicon-32.png"

    large = make_badge_png(badge_png, size=512)
    write_svg(badge_svg)
    write_favicon(large, favicon_ico, favicon_32)

    print("Wrote:")
    for p in [badge_png, badge_svg, favicon_ico, favicon_32]:
        print("-", p)


if __name__ == "__main__":
    main()

