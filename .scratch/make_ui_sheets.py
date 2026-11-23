from pathlib import Path
import re
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent / "ui-validation"
images = {}
for raw in sorted(root.glob("*.rgba")):
    match = re.search(r"-(\d+)x(\d+)$", raw.stem)
    if not match:
        raise RuntimeError(f"No physical dimensions in {raw.name}")
    width, height = map(int, match.groups())
    data = raw.read_bytes()
    if len(data) != width * height * 4:
        raise RuntimeError(f"Unexpected byte count in {raw.name}")
    image = Image.frombytes("RGBA", (width, height), data).convert("RGB")
    output = raw.with_suffix(".png")
    image.save(output, optimize=True)
    images[raw.stem] = output
    raw.unlink()

groups = {
    "overview-light": [p for stem, p in images.items() if stem.startswith("overview-") and ".light-" in stem],
    "devices-light": [p for stem, p in images.items() if stem.startswith("devices-") and ".light-" in stem],
    "equalizer-light": [p for stem, p in images.items() if stem.startswith("equalizer-") and ".light-" in stem],
    "presets-light": [p for stem, p in images.items() if stem.startswith("presets-") and ".light-" in stem],
    "controls-light": [p for stem, p in images.items() if stem.startswith("controls-") and ".light-" in stem],
    "comfort-light": [p for stem, p in images.items() if stem.startswith("comfort-") and ".light-" in stem],
    "power-light": [p for stem, p in images.items() if stem.startswith("power-") and ".light-" in stem],
    "connections-light": [p for stem, p in images.items() if stem.startswith("connections-") and ".light-" in stem],
    "about-light": [p for stem, p in images.items() if stem.startswith("about-") and ".light-" in stem],
    "sound-effects-light": [p for stem, p in images.items() if stem.startswith("sound-effects-") and ".light-" in stem],
    "settings-light": [p for stem, p in images.items() if stem.startswith("settings-") and ".light-" in stem],
    "fields-dialogs-light": [p for stem, p in images.items() if stem.startswith(("add-device-dialog-", "save-profile-dialog-", "create-preset-dialog-"))],
    "diagnostics-device-light": [p for stem, p in images.items() if stem.startswith("diagnostics-device-") and ".light-" in stem],
    "sound-mode-light": [p for stem, p in images.items() if stem.startswith("sound-mode-") and ".light-" in stem],
    "a3954-scale-light": [p for stem, p in images.items() if stem.startswith("a3954-scale-") and ".light-" in stem],
    "a3954-overview-light": [p for stem, p in images.items() if stem.startswith("a3954-overview-") and ".light-" in stem],
    "dark-states": [p for stem, p in images.items() if ".dark-" in stem],
    "diagnostics-review-light": [p for stem, p in images.items() if stem.startswith("diagnostics-review-") and ".light-" in stem],
    "dialogs-light": [p for stem, p in images.items() if (stem.startswith("layout-dialog-") or stem.startswith("toggles-dialog-"))],
}

for name, paths in groups.items():
    paths.sort()
    columns = 3
    cell_width = 420
    cell_height = 355
    label_height = 32
    rows = (len(paths) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * cell_width, rows * (cell_height + label_height)), "#d8d8d8")
    draw = ImageDraw.Draw(sheet)
    for index, path in enumerate(paths):
        image = Image.open(path)
        image.thumbnail((cell_width - 12, cell_height - 12))
        x = (index % columns) * cell_width
        y = (index // columns) * (cell_height + label_height)
        draw.text((x + 6, y + 8), path.stem.rsplit("-", 1)[0], fill="#111111")
        sheet.paste(image, (x + (cell_width - image.width) // 2, y + label_height + (cell_height - image.height) // 2))
    output = root / f"sheet-{name}.jpg"
    sheet.save(output, quality=90, optimize=True)
    if output.stat().st_size >= 3_000_000:
        raise RuntimeError(f"Contact sheet too large: {output}")
    print(f"{output.name}: {output.stat().st_size} bytes, {len(paths)} captures")

png_sizes = [p.stat().st_size for p in images.values()]
print(f"PNG captures: {len(images)}; largest: {max(png_sizes)} bytes")
