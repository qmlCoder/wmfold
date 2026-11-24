"""Prepare website image assets from the real wmfold window screenshots."""
from pathlib import Path
from PIL import Image

ROOT = Path(r"E:\code\wmfold")
SRC = ROOT / ".tools"
OUT = ROOT / "site" / "assets"
OUT.mkdir(parents=True, exist_ok=True)


def save_webp(img: Image.Image, name: str, quality: int = 92) -> None:
    path = OUT / name
    img.save(path, "WEBP", quality=quality, method=6)
    print(f"{name}: {img.size[0]}x{img.size[1]}  {path.stat().st_size/1024:.0f} KB")


def resize_width(img: Image.Image, width: int) -> Image.Image:
    if img.width == width:
        return img
    h = round(img.height * width / img.width)
    return img.resize((width, h), Image.LANCZOS)


# --- main screenshots (captured at deviceScaleFactor 2) -------------------
hero = Image.open(SRC / "hero3.png").convert("RGB")            # 3600x2000 (1800x1000 css)
overview = Image.open(SRC / "shot-all-1400.png").convert("RGB")  # 2800x1800 (1400x900 css)
settings = Image.open(SRC / "shot-settings2.png").convert("RGB")  # 2800x1800 (1400x900 css)

save_webp(resize_width(hero, 2400), "app-log.webp", 93)
save_webp(resize_width(overview, 2000), "app-files.webp", 93)
save_webp(resize_width(settings, 1700), "app-settings.webp", 92)

# --- detail crops (from the 2x hero shot, so they stay crisp) ------------
# CSS coords x2.  Filter bar chips row: css y 158..200, full width
save_webp(hero.crop((0, 300, 3600, 404)), "detail-filter.webp", 93)
# Chem columns header + first rows: css x 800..1450, y 175..700
save_webp(hero.crop((1600, 350, 2900, 1400)), "detail-columns.webp", 93)
# Preview panel: css x 1446..1800, y 155..760
save_webp(hero.crop((2892, 310, 3600, 1520)), "detail-preview.webp", 93)

# --- terminal screenshot from the teammate (if present) ------------------
tui = SRC.parent / "site" / "assets" / "tui.png"
if tui.exists():
    img = Image.open(tui).convert("RGB")
    save_webp(resize_width(img, min(img.width, 2000)), "tui.webp", 93)

# --- app icon / favicon --------------------------------------------------
icon = Image.open(ROOT / "src-tauri" / "icons" / "icon.png").convert("RGBA")
icon.resize((192, 192), Image.LANCZOS).save(OUT / "icon-192.png", "PNG", optimize=True)
icon.resize((64, 64), Image.LANCZOS).save(OUT / "favicon.png", "PNG", optimize=True)
print("icon-192.png / favicon.png written")

for f in sorted(OUT.glob("*")):
    print(f.name, f"{f.stat().st_size/1024:.0f} KB")
