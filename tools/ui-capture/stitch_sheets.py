#!/usr/bin/env python3
"""Build contact sheets and an HTML index from capture-ui.ps1's manifest.

The capture script writes raw PNGs plus manifest.json; this script never talks to
the application. It groups the same captures several ways so visual review can
start from the grouping that fits the question.
"""

from __future__ import annotations

import json
import sys
from collections import OrderedDict
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageOps

CELL_W = 420
CELL_H = 330
LABEL_H = 34
MARGIN = 12
COLUMNS = 3
BG = "#eeeeee"
CARD = "#ffffff"
INK = "#222222"
MUTED = "#666666"


def font(size: int, bold: bool = False):
    names = ["segoeuib.ttf" if bold else "segoeui.ttf", "arialbd.ttf" if bold else "arial.ttf"]
    for name in names:
        path = Path("C:/Windows/Fonts") / name
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.load_default()


def read_json(path: Path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


class Capture:
    def __init__(self, raw: dict, run_dir: Path, labels: dict):
        self.raw = raw
        self.path = run_dir / raw["file"]
        self.id = raw["id"]
        self.kind = raw.get("kind", "page")
        self.area = raw.get("area", "app")
        self.flow = raw.get("flow", 0)
        self.theme = raw["theme"]
        self.scale = int(raw["scale"])
        self.label = raw.get("label") or self.id
        self.theme_label = labels.get("themeLabels", {}).get(self.theme, self.theme)

    @property
    def exists(self) -> bool:
        return self.raw.get("status") == "ok" and self.path.exists()


def tile(capture: Capture) -> Image.Image:
    image = Image.open(capture.path).convert("RGB")
    image = ImageOps.contain(image, (CELL_W - 16, CELL_H - 16), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (CELL_W, CELL_H + LABEL_H), CARD)
    canvas.paste(image, ((CELL_W - image.width) // 2, LABEL_H + (CELL_H - image.height) // 2))
    draw = ImageDraw.Draw(canvas)
    draw.text((10, 8), capture.label, fill=INK, font=font(15, bold=True))
    meta = f"{capture.theme_label} · {capture.scale}%"
    meta_font = font(12)
    meta_w = draw.textlength(meta, font=meta_font)
    draw.text((CELL_W - meta_w - 10, 10), meta, fill=MUTED, font=meta_font)
    return canvas


def make_sheet(captures: list[Capture], title: str, path: Path, columns: int = COLUMNS):
    captures = [capture for capture in captures if capture.exists]
    if not captures:
        return None
    columns = max(1, min(columns, len(captures)))
    rows = (len(captures) + columns - 1) // columns
    sheet = Image.new(
        "RGB",
        (columns * (CELL_W + MARGIN) + MARGIN, 50 + rows * (CELL_H + LABEL_H + MARGIN) + MARGIN),
        BG,
    )
    draw = ImageDraw.Draw(sheet)
    draw.text((MARGIN + 4, 12), title, fill=INK, font=font(18, bold=True))
    for index, capture in enumerate(captures):
        x = MARGIN + (index % columns) * (CELL_W + MARGIN)
        y = 50 + (index // columns) * (CELL_H + LABEL_H + MARGIN)
        sheet.paste(tile(capture), (x, y))
    path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(path, quality=85, optimize=True)
    print(f"sheet {path.relative_to(path.parent.parent)}: {len(captures)} captures, {path.stat().st_size} bytes")
    return path


def main(run_dir: Path) -> int:
    manifest = read_json(run_dir / "manifest.json")
    labels_path = Path(__file__).with_name("labels.ru.json")
    labels = read_json(labels_path)
    captures = [Capture(raw, run_dir, labels) for raw in manifest.get("captures", [])]
    pages = [capture for capture in captures if capture.kind == "page"]
    dialogs = [capture for capture in captures if capture.kind == "dialog"]
    pages_by_id = {page["id"]: page for page in labels["pages"]}
    dialog_by_id = {dialog["id"]: dialog for dialog in labels["dialogs"]}
    page_order = [page["id"] for page in labels["pages"]]
    flow = {page["id"]: page.get("flow", 99) for page in labels["pages"]}
    flow.update({dialog["id"]: dialog.get("flow", 99) for dialog in labels["dialogs"]})

    sheets_dir = run_dir / "sheets"
    sheets_dir.mkdir(parents=True, exist_ok=True)
    generated: list[OrderedDict] = []

    # 1. One sheet per theme + content scale: the quick visual-QA overview.
    for theme in manifest.get("themes", []):
        for scale in [int(value) for value in manifest.get("scales", [])]:
            state_pages = [capture for capture in pages if capture.theme == theme and capture.scale == scale]
            state_pages.sort(key=lambda capture: (flow.get(capture.id, 99), page_order.index(capture.id) if capture.id in page_order else 999))
            path = sheets_dir / "by-theme-scale" / f"{theme}-{scale}.jpg"
            made = make_sheet(state_pages, f"OpenSCL4 · {labels['themeLabels'].get(theme, theme)} · {scale}%", path, columns=3)
            if made:
                generated.append(OrderedDict(kind="theme-scale", theme=theme, scale=scale, file=str(made.relative_to(run_dir)).replace("\\", "/"), count=len(state_pages)))
            state_dialogs = [capture for capture in dialogs if capture.theme == theme and capture.scale == scale]
            state_dialogs.sort(key=lambda capture: flow.get(capture.id, 99))
            made = make_sheet(state_dialogs, f"OpenSCL4 · диалоги · {labels['themeLabels'].get(theme, theme)} · {scale}%", sheets_dir / "by-theme-scale" / f"dialogs-{theme}-{scale}.jpg", columns=3)
            if made:
                generated.append(OrderedDict(kind="theme-scale-dialogs", theme=theme, scale=scale, file=str(made.relative_to(run_dir)).replace("\\", "/"), count=len(state_dialogs)))

    # 2. One sheet per page, all theme/scale states: best for regressions.
    theme_order = list(manifest.get("themes", []))
    scale_order = [int(value) for value in manifest.get("scales", [])]
    for page_id in page_order:
        state_entries = sorted(
            [capture for capture in pages if capture.id == page_id],
            key=lambda capture: (theme_order.index(capture.theme) if capture.theme in theme_order else 99, scale_order.index(capture.scale) if capture.scale in scale_order else 99),
        )
        if not state_entries:
            continue
        path = sheets_dir / "by-page" / f"{page_id}.jpg"
        made = make_sheet(state_entries, f"{pages_by_id[page_id]['label']} · все темы и масштабы", path, columns=4)
        if made:
            generated.append(OrderedDict(kind="page", page=page_id, file=str(made.relative_to(run_dir)).replace("\\", "/"), count=len(state_entries)))

    # 3. One sheet per functional area: best for design review.
    areas: OrderedDict[str, list[Capture]] = OrderedDict()
    for capture in pages:
        areas.setdefault(capture.area, []).append(capture)
    for dialog in dialogs:
        areas.setdefault("dialogs", []).append(dialog)
    for area, area_captures in areas.items():
        area_captures.sort(key=lambda capture: (flow.get(capture.id, 99), capture.theme, scale_order.index(capture.scale) if capture.scale in scale_order else 99))
        path = sheets_dir / "by-area" / f"{area}.jpg"
        made = make_sheet(area_captures, f"OpenSCL4 · {labels.get('areas', {}).get(area, area)}", path, columns=3)
        if made:
            generated.append(OrderedDict(kind="area", area=area, file=str(made.relative_to(run_dir)).replace("\\", "/"), count=len(area_captures)))

    # 4. Optional flow sheet per theme/scale: reading order for a UX walkthrough.
    for theme in manifest.get("themes", []):
        for scale in [int(value) for value in manifest.get("scales", [])]:
            state = [capture for capture in pages if capture.theme == theme and capture.scale == scale]
            state.sort(key=lambda capture: flow.get(capture.id, 99))
            path = sheets_dir / "by-flow" / f"{theme}-{scale}.jpg"
            made = make_sheet(state, f"OpenSCL4 · сценарий · {labels['themeLabels'].get(theme, theme)} · {scale}%", path, columns=4)
            if made:
                generated.append(OrderedDict(kind="flow", theme=theme, scale=scale, file=str(made.relative_to(run_dir)).replace("\\", "/"), count=len(state)))

    # 5. HTML index with thumbnails and relative links.
    def html_group(title: str, items: list[OrderedDict]):
        if not items:
            return ""
        cards = []
        for item in items:
            rel = item["file"]
            cards.append(
                f'<figure><a href="{rel}"><img loading="lazy" src="{rel}" alt="{title}"></a>'
                f'<figcaption>{rel.split("/")[-1]} · {item.get("count", 0)} снимков</figcaption></figure>'
            )
        return f'<section><h2>{title}</h2><div class="grid">{"".join(cards)}</div></section>'

    groups = OrderedDict([
        ("Сначала: тема × масштаб", [item for item in generated if item["kind"] in ("theme-scale", "theme-scale-dialogs")]),
        ("Одна страница во всех состояниях", [item for item in generated if item["kind"] == "page"]),
        ("Функциональные разделы", [item for item in generated if item["kind"] == "area"]),
        ("Сценарий", [item for item in generated if item["kind"] == "flow"]),
    ])
    error_items = manifest.get("errors", [])
    html = f"""<!doctype html>
<html lang="ru">
<meta charset="utf-8">
<title>OpenSCL4 · скриншоты UI</title>
<style>
:root {{ color-scheme: light; }}
body {{ font: 14px/1.4 "Segoe UI", Arial, sans-serif; margin: 24px; background: #faf8f5; color: #25222a; }}
h1 {{ margin: 0 0 4px; }}
.meta {{ color: #6b6470; margin-bottom: 18px; }}
section {{ margin: 28px 0; }}
h2 {{ font-size: 18px; border-bottom: 1px solid #ddd6e0; padding-bottom: 6px; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 14px; }}
figure {{ margin: 0; background: #fff; border: 1px solid #ddd6e0; border-radius: 12px; padding: 8px; }}
figure img {{ width: 100%; display: block; border-radius: 6px; }}
figcaption {{ color: #6b6470; font-size: 12px; margin-top: 6px; }}
.errors {{ background: #fff1f0; border: 1px solid #f0c0c0; border-radius: 10px; padding: 10px 14px; }}
code {{ font: 12px Consolas, monospace; }}
</style>
<h1>OpenSCL4 · скриншоты UI</h1>
<div class="meta">
Язык: русский · темы: {', '.join(manifest.get('themes', []))} · масштабы: {', '.join(str(value) for value in manifest.get('scales', []))}%<br>
Снимков: {len(captures)} · ошибок: {len(error_items)} · запуск: <code>{manifest.get('run', '')}</code>
</div>
{html_group('Сначала: тема × масштаб', groups['Сначала: тема × масштаб'])}
{html_group('Одна страница во всех состояниях', groups['Одна страница во всех состояниях'])}
{html_group('Функциональные разделы', groups['Функциональные разделы'])}
{html_group('Сценарий', groups['Сценарий'])}
{('<div class="errors"><h2>Ошибки</h2><ul>' + ''.join(f'<li>{item}</li>' for item in error_items) + '</ul></div>') if error_items else ''}
</html>
"""
    (run_dir / "index.html").write_text(html, encoding="utf-8")
    (run_dir / "sheets" / "sheets.json").write_text(json.dumps(generated, ensure_ascii=False, indent=2), encoding="utf-8")
    readme = run_dir / "README.txt"
    if not readme.exists():
        readme.write_text(
            "OpenSCL4 UI screenshots\n\n"
            "index.html — начните здесь. Группировки:\n"
            "- by-theme-scale — все страницы для одной темы и масштаба (быстрый визуальный контроль);\n"
            "- by-page — одна страница во всех темах/масштабах (регрессии);\n"
            "- by-area — функциональные разделы (дизайн-ревью);\n"
            "- by-flow — порядок пользовательского сценария.\n"
            "raw/ — исходные PNG; manifest.json — метаданные и ошибки.\n",
            encoding="utf-8",
        )
    print(f"index.html: {len(generated)} sheets, {len(captures)} captures")
    if not generated:
        return 1
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("usage: stitch_sheets.py <capture-run-directory>", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(Path(sys.argv[1]).resolve()))
