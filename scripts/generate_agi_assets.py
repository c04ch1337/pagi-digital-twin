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

# Pillow is expected to be available in the runtime environment used to generate assets.
# Some editors/typecheckers may not have it configured in their analysis environment.
from PIL import Image, ImageDraw, ImageFont  # pyright: ignore[reportMissingImports]


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

    # Propane / natural gas inspired flame icon.
    # Outer flame (amber -> orange)
    flame_box = (
        int(size * 0.22),
        int(size * 0.16),
        int(size * 0.78),
        int(size * 0.88),
    )
    left, top, right, bottom = flame_box
    w = right - left
    h = bottom - top
    cx = (left + right) // 2

    outer_color = (251, 146, 60, 235)  # orange-400
    outer_highlight = (252, 211, 77, 225)  # amber-300
    inner_color = (255, 255, 255, 210)

    # Outer flame: combine a base ellipse with an upper polygon.
    base_h = int(h * 0.32)
    base_box = (
        int(left + w * 0.20),
        bottom - base_h,
        int(right - w * 0.20),
        bottom,
    )

    tip = (cx, top)
    poly = [
        (int(left + w * 0.20), bottom - base_h // 2),
        (int(left + w * 0.10), int(top + h * 0.46)),
        (int(left + w * 0.28), int(top + h * 0.30)),
        tip,
        (int(right - w * 0.28), int(top + h * 0.30)),
        (int(right - w * 0.10), int(top + h * 0.46)),
        (int(right - w * 0.20), bottom - base_h // 2),
    ]

    draw.polygon(poly, fill=outer_color)
    draw.ellipse(base_box, fill=outer_color)

    # Outer highlight (subtle)
    highlight_poly = [
        (cx, int(top + h * 0.10)),
        (int(left + w * 0.40), int(top + h * 0.48)),
        (cx, int(bottom - base_h * 0.65)),
        (int(right - w * 0.40), int(top + h * 0.48)),
    ]
    draw.polygon(highlight_poly, fill=outer_highlight)

    # Inner flame
    inner = (
        int(left + w * 0.25),
        int(top + h * 0.28),
        int(right - w * 0.25),
        int(bottom - h * 0.18),
    )
    il, it, ir, ib = inner
    iw = ir - il
    ih = ib - it
    icx = (il + ir) // 2

    inner_tip = (icx, it)
    inner_base_h = int(ih * 0.34)
    inner_base_box = (
        int(il + iw * 0.26),
        ib - inner_base_h,
        int(ir - iw * 0.26),
        ib,
    )
    inner_poly = [
        (int(il + iw * 0.26), ib - inner_base_h // 2),
        (int(il + iw * 0.18), int(it + ih * 0.55)),
        inner_tip,
        (int(ir - iw * 0.18), int(it + ih * 0.55)),
        (int(ir - iw * 0.26), ib - inner_base_h // 2),
    ]
    draw.polygon(inner_poly, fill=inner_color)
    draw.ellipse(inner_base_box, fill=inner_color)

    img.save(path)
    return img


def write_svg(path: Path) -> None:
    svg = """<svg xmlns='http://www.w3.org/2000/svg' width='512' height='512' viewBox='0 0 512 512'>
  <defs>
    <radialGradient id='bg' cx='40%' cy='35%' r='70%'>
      <stop offset='0%' stop-color='#6366f1'/>
      <stop offset='100%' stop-color='#1e1b4b'/>
    </radialGradient>
    <linearGradient id='flame' x1='0' y1='0' x2='0' y2='1'>
      <stop offset='0%' stop-color='#fcd34d'/>
      <stop offset='55%' stop-color='#fb923c'/>
      <stop offset='100%' stop-color='#ef4444'/>
    </linearGradient>
  </defs>

  <circle cx='256' cy='256' r='236' fill='url(#bg)'/>
  <circle cx='256' cy='256' r='212' fill='none' stroke='rgba(255,255,255,0.25)' stroke-width='18'/>

  <!-- Propane / natural gas flame -->
  <path
    d='M256 96
       C214 154 176 198 186 270
       C198 355 238 396 256 420
       C274 396 314 355 326 270
       C336 198 298 154 256 96 Z'
    fill='url(#flame)' opacity='0.95'/>

  <path
    d='M256 164
       C236 200 216 222 220 272
       C224 322 244 350 256 366
       C268 350 288 322 292 272
       C296 222 276 200 256 164 Z'
    fill='rgba(255,255,255,0.85)'/>
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

