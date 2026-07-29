# utils/convert_map.py
import sys
from pathlib import Path
from PIL import Image

def convert_pgm_to_png(pgm_path: Path, output_path: Path):
    if not pgm_path.exists():
        print(f"Error: {pgm_path} does not exist.")
        sys.exit(1)

    with Image.open(pgm_path) as img:
        img.save(output_path, "PNG")
        print(f"Successfully converted {pgm_path.name} -> {output_path.name}")

if __name__ == "__main__":
    base_dir = Path(__file__).resolve().parent.parent / "core" / "static" / "assets"
    pgm_file = base_dir / "warehouse.pgm"
    png_file = base_dir / "warehouse.png"
    
    convert_pgm_to_png(pgm_file, png_file)