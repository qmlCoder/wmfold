"""Subset the Noto Serif/Sans SC variable fonts to the characters used on the page."""
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(r"E:\code\wmfold")
HTML = (ROOT / "site" / "index.html").read_text(encoding="utf-8")
FONTS = Path(r"C:\Windows\Fonts")
OUT = ROOT / "site" / "assets" / "fonts"
OUT.mkdir(parents=True, exist_ok=True)

# every unique character that appears anywhere in the page (text, attributes, css, js)
chars = sorted(set(HTML))
# always include a little safety margin for typographic characters and ASCII
chars += list("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
              " .,:;!?()[]{}<>/\\|+-=*&%$#@'\"`~^_·—–…「」『』、。，；：？！“”‘’《》〈〉％＃")
text = "".join(dict.fromkeys(chars))
print(f"unique chars: {len(text)}")
(ROOT / ".tools" / "chars.txt").write_text(text, encoding="utf-8")


def do(src_name: str, dst_name: str) -> None:
    src = FONTS / src_name
    font = TTFont(src)
    axes = {a.axisTag: (a.minValue, a.maxValue) for a in font["fvar"].axes} if "fvar" in font else {}
    print(f"{src_name}: axes={axes}")
    opts = subset.Options()
    opts.flavor = "woff2"
    opts.desubroutinize = False
    opts.layout_features = ["*"]
    opts.name_IDs = [1, 2, 3, 4, 6]
    opts.notdef_outline = True
    subsetter = subset.Subsetter(options=opts)
    subsetter.populate(text=text)
    subsetter.subset(font)
    dst = OUT / dst_name
    font.flavor = "woff2"
    font.save(dst)
    print(f"  -> {dst.name}: {dst.stat().st_size/1024:.0f} KB")


do("NotoSerifSC-VF.ttf", "serif.woff2")
do("NotoSansSC-VF.ttf", "sans.woff2")

# sanity check: axes survived, and the glyph for a few chars exists
for name in ("serif.woff2", "sans.woff2"):
    f = TTFont(OUT / name)
    has_fvar = "fvar" in f
    cmap = f.getBestCmap()
    sample = [ord(c) for c in "化学式能量文件"]
    missing = [chr(c) for c in sample if c not in cmap]
    print(f"{name}: fvar={has_fvar} glyphs={len(cmap)} missing={missing}")
