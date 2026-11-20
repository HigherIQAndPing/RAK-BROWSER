#!/usr/bin/env python3
"""Fügt das gemeinsame Design (design/rak-design.css) und die benötigten SVG-Icons in die lokalen
Oberflächen-Dateien ein. Nur die markierten Bereiche werden ersetzt, JavaScript bleibt unberührt:

  /* RAK-DESIGN:BEGIN */ ... /* RAK-DESIGN:END */     (im <style>)
  <!--RAK-SPRITE:BEGIN--> ... <!--RAK-SPRITE:END-->   (direkt nach <body ...>)

Aufruf:  python3 tools/apply-design.py        (aus dem Projektordner)
"""
import re, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PAGES = ["home", "settings", "downloads", "history", "bookmarks", "extensions", "passwords"]

# Stroke-Icons im 24×24-Raster (Linienstärke/-enden kommen aus .ds-i)
ICONS = {
 "search": '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
 "arrow-right": '<path d="M5 12h14M13 6l6 6-6 6"/>',
 "arrow-up-right": '<path d="M7 17L17 7M8 7h9v9"/>',
 "play": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M10 9.3v5.4l4.6-2.7z"/>',
 "terminal": '<rect x="3" y="4.5" width="18" height="15" rx="2"/><path d="M7 10l3 2.5L7 15M12.5 15H17"/>',
 "music": '<path d="M9 18V6l10-2v12"/><circle cx="7" cy="18" r="2"/><circle cx="17" cy="16" r="2"/>',
 "compass": '<circle cx="12" cy="12" r="8.5"/><path d="M14.8 9.2l-2.1 4.4-4.4 2.1 2.1-4.4z"/>',
 "plus": '<path d="M12 5v14M5 12h14"/>',
 "mask": '<path d="M3 11.5c0-3.4 3-5.5 9-5.5s9 2.1 9 5.5c0 3-2 5-4.5 5-1.6 0-2.3-1.2-4.5-1.2s-2.9 1.2-4.5 1.2C5 16.5 3 14.5 3 11.5z"/><path d="M8 11.2h2M14 11.2h2"/>',
 "sliders": '<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
 "download": '<path d="M12 4v11M7 10.5l5 5 5-5M5 19.5h14"/>',
 "folder": '<path d="M3.5 7.5a2 2 0 012-2H10l2 2.5h6.5a2 2 0 012 2V17a2 2 0 01-2 2h-13a2 2 0 01-2-2z"/>',
 "folder-open": '<path d="M3.5 18.5V7a2 2 0 012-2H10l2 2.5h6a2 2 0 012 2V10"/><path d="M3.5 18.5l2.6-7.2a1.5 1.5 0 011.4-1h13.2a1 1 0 011 1.3l-2.2 6.4a1.5 1.5 0 01-1.4 1z"/>',
 "trash": '<path d="M4.5 7h15M10 4h4M6.5 7l.8 12a1.5 1.5 0 001.5 1.4h6.4a1.5 1.5 0 001.5-1.4l.8-12M10 11v6M14 11v6"/>',
 "close": '<path d="M6 6l12 12M18 6L6 18"/>',
 "external": '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1.5 1.5 0 01-1.5 1.5h-11A1.5 1.5 0 014 19V8.5A1.5 1.5 0 015.5 7H10"/>',
 "shield": '<path d="M12 3l7.5 3v5.5c0 4.6-3.1 8.2-7.5 9.5-4.4-1.3-7.5-4.9-7.5-9.5V6z"/><path d="M9 12l2.2 2.2L15.5 10"/>',
 "key": '<circle cx="8" cy="15" r="3.5"/><path d="M10.5 12.5L19 4M16 7l2.5 2.5M14 9l2 2"/>',
 "lock": '<rect x="5" y="10.5" width="14" height="9.5" rx="2"/><path d="M8 10.5V8a4 4 0 018 0v2.5"/>',
 "puzzle": '<path d="M10 4.5a2 2 0 114 0V6h3.5A1.5 1.5 0 0119 7.5V11h-1.5a2 2 0 100 4H19v3.5a1.5 1.5 0 01-1.5 1.5H14v-1.5a2 2 0 10-4 0V20H6.5A1.5 1.5 0 015 18.5V15h1.5a2 2 0 100-4H5V7.5A1.5 1.5 0 016.5 6H10z"/>',
 "bookmark": '<path d="M7 4h10a1 1 0 011 1v15l-6-4-6 4V5a1 1 0 011-1z"/>',
 "star": '<path d="M12 3.5l2.6 5.6 6.1.7-4.5 4.2 1.2 6L12 17l-5.4 3 1.2-6L3.3 9.8l6.1-.7z"/>',
 "clock": '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>',
 "refresh": '<path d="M20 12a8 8 0 10-2.4 5.7M20 5v5h-5"/>',
 "check": '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
 "cpu": '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9.5" y="9.5" width="5" height="5" rx="1"/><path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3"/>',
 "volume": '<path d="M4 10v4h3.5L12 18V6L7.5 10z"/><path d="M15.5 9a4 4 0 010 6M18 6.5a8 8 0 010 11"/>',
 "palette": '<path d="M12 3.5a8.5 8.5 0 100 17c1.2 0 1.8-.8 1.8-1.7 0-1.4-1-1.7-1-2.7 0-.9.7-1.6 1.6-1.6H17a3.5 3.5 0 003.5-3.5C20.5 6.7 16.7 3.5 12 3.5z"/><circle cx="8" cy="11" r=".9"/><circle cx="11" cy="7.8" r=".9"/><circle cx="15.2" cy="8.4" r=".9"/>',
 "image": '<rect x="3.5" y="5" width="17" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M4 17l5-4.5 4 3.5 3-2.5 4.5 4"/>',
 "globe": '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.5 2.6 3.7 5.4 3.7 8.5s-1.2 5.9-3.7 8.5c-2.5-2.6-3.7-5.4-3.7-8.5S9.5 6.1 12 3.5z"/>',
 "translate": '<path d="M4 6h9M8.5 4v2M6 6c.5 3 2.7 5.5 5.5 6.8M12 6c-.6 3-3 5.8-6.5 7.2M13.5 20l4-9 4 9M15 17h5"/>',
 "search-engine": '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5M8.5 11h5M11 8.5v5"/>',
 "logo": '<path d="M12 2.8l7.6 4.4v9.6L12 21.2l-7.6-4.4V7.2z"/><path d="M9.6 16V8.4h3a2.2 2.2 0 010 4.4h-3M12.6 12.8l2.6 3.2"/>',
 "gauge": '<path d="M4.5 17a8.5 8.5 0 1115 0"/><path d="M12 13.5l4-4"/><circle cx="12" cy="14" r="1.2"/>',
 "warning": '<path d="M12 4l9 15.5H3z"/><path d="M12 10v4.2M12 17h.01"/>',
}

CSS_RE = re.compile(r"/\* RAK-DESIGN:BEGIN.*?/\* RAK-DESIGN:END \*/", re.S)
SPR_RE = re.compile(r"<!--RAK-SPRITE:BEGIN-->.*?<!--RAK-SPRITE:END-->", re.S)

def sprite_for(html: str) -> str:
    used = sorted(set(re.findall(r'href="#i-([a-z0-9-]+)"', SPR_RE.sub("", html))))
    unknown = [u for u in used if u not in ICONS]
    if unknown:
        sys.exit(f"Unbekannte Icons: {unknown}")
    syms = "".join(f'<symbol id="i-{n}" viewBox="0 0 24 24">{ICONS[n]}</symbol>' for n in used)
    return f'<!--RAK-SPRITE:BEGIN--><svg class="sprite" aria-hidden="true" focusable="false"><defs>{syms}</defs></svg><!--RAK-SPRITE:END-->'

def main():
    css = (ROOT / "design" / "rak-design.css").read_text(encoding="utf-8")
    block = "/* RAK-DESIGN:BEGIN – generiert aus design/rak-design.css (tools/apply-design.py), nicht von Hand ändern */\n" + css + "\n/* RAK-DESIGN:END */"
    for name in PAGES:
        p = ROOT / "src" / f"{name}.html"
        html = p.read_text(encoding="utf-8")
        if not CSS_RE.search(html):
            sys.exit(f"{p.name}: Marker /* RAK-DESIGN:BEGIN */ … /* RAK-DESIGN:END */ fehlt im <style>")
        html = CSS_RE.sub(lambda m: block, html, count=1)
        if not SPR_RE.search(html):
            sys.exit(f"{p.name}: Marker <!--RAK-SPRITE:BEGIN--><!--RAK-SPRITE:END--> fehlt im <body>")
        html = SPR_RE.sub(lambda m: sprite_for(html), html, count=1)
        p.write_text(html, encoding="utf-8")
        print("ok", p.name)

if __name__ == "__main__":
    main()
