#!/usr/bin/env python3
"""
Generate docs/wiring.png for the ESP32 D1 R32 + TDS V1.0 + DS18B20 project.

Block-style wiring diagram (Fritzing-like). Run:
    python3 docs/draw_wiring.py
"""
from pathlib import Path
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

OUT = Path(__file__).parent / "wiring.png"

# ---------- canvas ----------
fig, ax = plt.subplots(figsize=(13, 8), dpi=180)
ax.set_xlim(0, 13)
ax.set_ylim(0, 8)
ax.set_aspect("equal")
ax.axis("off")
ax.set_facecolor("#fafafa")

WIRE = dict(linewidth=2.4, solid_capstyle="round", zorder=2)
COL_RED   = "#d62728"
COL_BLACK = "#222222"
COL_BLUE  = "#1f77b4"
COL_GREEN = "#2ca02c"
COL_BG_BOARD  = "#3a6ea5"
COL_BG_SENSOR = "#2e7d32"

# ---------- ESP32 D1 R32 board ----------
esp_x, esp_y, esp_w, esp_h = 0.5, 1.5, 5.5, 5.5
esp = mpatches.FancyBboxPatch(
    (esp_x, esp_y), esp_w, esp_h,
    boxstyle="round,pad=0.05,rounding_size=0.25",
    linewidth=1.4, edgecolor="#1c3d5a", facecolor=COL_BG_BOARD, zorder=1,
)
ax.add_patch(esp)
ax.text(esp_x + esp_w/2, esp_y + esp_h - 0.35,
        "Wemos D1 R32 (ESP32)", color="white",
        ha="center", va="center", fontsize=13, fontweight="bold")
ax.text(esp_x + esp_w/2, esp_y + esp_h - 0.85,
        "USB CH340  →  /dev/tty.usbserial-*  @ 115200", color="#cfd8dc",
        ha="center", va="center", fontsize=9, fontstyle="italic")

# Pin headers we'll draw on the right edge of the ESP board
pins = [
    # (label,        gpio,        color,          y_offset_from_top)
    ("3V3",          "+3.3 V",    COL_RED,        1.6),
    ("GND",          "GND",       COL_BLACK,      2.2),
    ("D7 / GPIO14",  "1-Wire DQ", COL_BLUE,       2.8),
    ("SVP / GPIO34", "ADC1_6",    COL_GREEN,      3.4),
]
pin_x = esp_x + esp_w  # right edge of board
pin_box_w, pin_box_h = 1.7, 0.42

esp_pin_coords = {}
for label, role, col, off in pins:
    py = esp_y + esp_h - off
    # gold-ish pad
    pad = mpatches.FancyBboxPatch(
        (pin_x - 0.05, py - pin_box_h/2), pin_box_w, pin_box_h,
        boxstyle="round,pad=0.02,rounding_size=0.08",
        linewidth=1.0, edgecolor="#8a6d3b", facecolor="#ffd24d", zorder=2,
    )
    ax.add_patch(pad)
    ax.text(pin_x + pin_box_w/2 - 0.05, py,
            label, color=COL_BLACK, ha="center", va="center",
            fontsize=8.5, fontweight="bold")
    ax.text(pin_x + pin_box_w + 0.05, py,
            role, color=col, ha="left", va="center", fontsize=8)
    esp_pin_coords[label] = (pin_x + pin_box_w - 0.05, py)

# ---------- TDS V1.0 module ----------
tds_x, tds_y, tds_w, tds_h = 9.0, 4.2, 3.3, 2.5
tds = mpatches.FancyBboxPatch(
    (tds_x, tds_y), tds_w, tds_h,
    boxstyle="round,pad=0.05,rounding_size=0.2",
    linewidth=1.4, edgecolor="#1b4d1f", facecolor=COL_BG_SENSOR, zorder=1,
)
ax.add_patch(tds)
ax.text(tds_x + tds_w/2, tds_y + tds_h - 0.3,
        "TDS Sensor V1.0", color="white",
        ha="center", va="center", fontsize=12, fontweight="bold")
ax.text(tds_x + tds_w/2, tds_y + tds_h - 0.7,
        "(DFRobot SEN0244-class\n+ on-board DS18B20)",
        color="#dcedc8", ha="center", va="center", fontsize=8, fontstyle="italic")

# 4 pins down the LEFT edge of the TDS module
tds_pins = [
    # (label, role,        color,      y_within_module)
    ("+",    "VCC 3V3",     COL_RED,    1.85),
    ("-",    "GND",         COL_BLACK,  1.40),
    ("A0",   "TDS analog",  COL_GREEN,  0.95),
    ("T1",   "DS18B20 DQ",  COL_BLUE,   0.50),
]
tds_pin_coords = {}
pin_box_w2, pin_box_h2 = 0.5, 0.36
for label, role, col, dy in tds_pins:
    py = tds_y + dy
    pad = mpatches.FancyBboxPatch(
        (tds_x - 0.45, py - pin_box_h2/2), pin_box_w2, pin_box_h2,
        boxstyle="round,pad=0.02,rounding_size=0.06",
        linewidth=1.0, edgecolor="#8a6d3b", facecolor="#ffd24d", zorder=2,
    )
    ax.add_patch(pad)
    ax.text(tds_x - 0.45 + pin_box_w2/2, py,
            label, color=COL_BLACK, ha="center", va="center",
            fontsize=10, fontweight="bold")
    ax.text(tds_x + 0.15, py, role, color="white",
            ha="left", va="center", fontsize=9)
    tds_pin_coords[label] = (tds_x - 0.45, py)

# ---------- probe (waterproof + 2 electrodes + DS18B20 cable) ----------
probe_x, probe_y = 11.6, 0.4
ax.plot([tds_x + tds_w, probe_x], [tds_y + 0.3, probe_y + 0.4],
        color="#666", linewidth=2, zorder=1)
ax.add_patch(mpatches.FancyBboxPatch(
    (probe_x, probe_y), 1.0, 1.6,
    boxstyle="round,pad=0.03,rounding_size=0.12",
    linewidth=1.0, edgecolor="#333", facecolor="#bdbdbd", zorder=1,
))
ax.text(probe_x + 0.5, probe_y + 1.3, "Probe", ha="center", va="center",
        fontsize=8, fontweight="bold")
# 2 electrodes
for ex in (probe_x + 0.3, probe_x + 0.7):
    ax.plot([ex, ex], [probe_y, probe_y - 0.55],
            color="#888", linewidth=2, zorder=1)
ax.text(probe_x + 0.5, probe_y - 0.85, "2 conductivity\nelectrodes",
        ha="center", va="center", fontsize=7.5, color="#555")

# ---------- wires (Manhattan-routed) ----------
def manhattan(p1, p2, midx, color, **kw):
    x1, y1 = p1; x2, y2 = p2
    ax.plot([x1, midx, midx, x2], [y1, y1, y2, y2],
            color=color, **WIRE, **kw)

# vertical channels in different x to keep wires readable
manhattan(esp_pin_coords["3V3"],         tds_pin_coords["+"],  midx=8.05, color=COL_RED)
manhattan(esp_pin_coords["GND"],         tds_pin_coords["-"],  midx=8.30, color=COL_BLACK)
manhattan(esp_pin_coords["SVP / GPIO34"],tds_pin_coords["A0"], midx=8.55, color=COL_GREEN)
manhattan(esp_pin_coords["D7 / GPIO14"], tds_pin_coords["T1"], midx=8.80, color=COL_BLUE)

# Channel legend at the very top of the wires
ax.text(8.05, 7.20, "+3V3",   color=COL_RED,   fontsize=8.5, fontweight="bold", ha="center")
ax.text(8.30, 7.45, "GND",    color=COL_BLACK, fontsize=8.5, fontweight="bold", ha="center")
ax.text(8.55, 7.70, "Analog", color=COL_GREEN, fontsize=8.5, fontweight="bold", ha="center")
ax.text(8.80, 7.95, "1-Wire", color=COL_BLUE,  fontsize=8.5, fontweight="bold", ha="center")

# ---------- legend / notes ----------
notes = (
    "Notes:\n"
    "• Power TDS board from 3V3 (NOT 5V) so analog out (max ~2.3 V) stays inside ESP32 ADC range.\n"
    "• 4.7 kΩ pull-up between T1 and 3V3 — usually already on the TDS V1.0 board.\n"
    "• GPIO34 is input-only and on ADC1 (must be ADC1; ADC2 is blocked when WiFi is active).\n"
    "• Firmware: docs/main.rs prints  temp + tds_raw + v + tds_ppm  every 1 s on UART0 @115200."
)
ax.text(0.6, 0.95, notes, fontsize=8.4, color="#222",
        ha="left", va="top", family="monospace",
        bbox=dict(boxstyle="round,pad=0.5", facecolor="#fff8d6",
                  edgecolor="#bba14f", linewidth=1.0))

ax.set_title(
    "ESP32 D1 R32 + TDS V1.0 + DS18B20 — wiring",
    fontsize=14, fontweight="bold", pad=8,
)

plt.tight_layout()
fig.savefig(OUT, bbox_inches="tight", facecolor=fig.get_facecolor())
print(f"wrote {OUT}")
