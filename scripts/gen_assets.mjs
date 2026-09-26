// Renders the editor's static bitmaps from the mockup's CSS gradients, so they match
// docs/mockup/Main.dc.html exactly. Uses a headless Chromium browser (Edge or Chrome).
//
//   node scripts/gen_assets.mjs [path-to-browser]
//
// Output: assets/images/{plate,knob_s,knob_m,knob_l,jog}_{silver,black}@{1,2}x.png

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

const root = resolve(import.meta.dirname, "..");
const out = join(root, "assets", "images");
mkdirSync(out, { recursive: true });

const candidates = [
  process.argv[2],
  process.env.CHROME,
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/chromium",
  "/usr/bin/google-chrome",
].filter(Boolean);
const browser = candidates.find((p) => existsSync(p));
if (!browser) throw new Error("No Chromium browser found; pass its path as an argument");

const brush =
  "repeating-linear-gradient(90deg, rgba(255,255,255,0.05) 0px, rgba(255,255,255,0.05) 1px, rgba(0,0,0,0.03) 1px, rgba(0,0,0,0.03) 3px)";

// Theme tokens, copied from the mockup.
const themes = {
  silver: {
    plate: brush + ", linear-gradient(180deg, #e8eaeb 0%, #d4d7d9 55%, #c5c8cb 100%)",
    panel: brush + ", linear-gradient(180deg, #dde0e2 0%, #d1d4d6 100%)",
    panelShadow: "inset 0 1px 2px rgba(0,0,0,0.12), 0 1px 0 #f4f5f6",
    grooveDark: "#9ea3a7",
    grooveLight: "#f6f7f7",
    bezel: "linear-gradient(180deg, #2a2c2f, #3c3f42)",
    knob: "radial-gradient(circle at 50% 35%, #ffffff 0%, #e3e5e7 30%, #b9bdc0 58%, #9da1a5 60.5%, transparent 61%), repeating-conic-gradient(#8c9195 0deg 5deg, #d5d8da 5deg 10deg)",
    jog: "radial-gradient(circle at 50% 35%, #ffffff 0%, #e2e4e6 38%, #bfc3c6 76%, #9ea2a6 78.5%, transparent 79%), repeating-conic-gradient(#868b8f 0deg 3deg, #d8dbdd 3deg 6deg)",
    dimple: "radial-gradient(circle at 50% 60%, #c9ccce, #a9adb0)",
  },
  black: {
    plate: brush + ", linear-gradient(180deg, #2b2c2f 0%, #1e1f21 60%, #161719 100%)",
    panel: brush + ", linear-gradient(180deg, #242528 0%, #1d1e20 100%)",
    panelShadow: "inset 0 1px 0 rgba(255,255,255,0.05), 0 1px 0 #34353a",
    grooveDark: "#08090a",
    grooveLight: "#393a3f",
    bezel: "linear-gradient(180deg, #0d0d0e, #1a1b1d)",
    knob: "radial-gradient(circle at 50% 35%, #4b4c50 0%, #2d2e31 32%, #1c1d1f 58%, #111214 60.5%, transparent 61%), repeating-conic-gradient(#08090a 0deg 5deg, #2f3034 5deg 10deg)",
    jog: "radial-gradient(circle at 50% 35%, #45464a 0%, #2a2b2e 40%, #1b1c1e 76%, #0f1011 78.5%, transparent 79%), repeating-conic-gradient(#060607 0deg 3deg, #2c2d31 3deg 6deg)",
    dimple: "radial-gradient(circle at 50% 60%, #26272a, #141516)",
  },
};

// Section panels and the VFD bezel (layout from the design doc, section 7).
const panels = [
  [16, 166, 360, 330],
  [386, 166, 400, 330],
  [796, 166, 468, 330],
  [16, 506, 470, 220],
  [496, 506, 400, 220],
  [906, 506, 358, 220],
];

function page(w, h, body) {
  return `<!doctype html><html><head><style>html,body{margin:0;padding:0;background:transparent;width:${w}px;height:${h}px;overflow:hidden}</style></head><body>${body}</body></html>`;
}

function plate(t) {
  const secs = panels
    .map(
      ([x, y, w, h]) =>
        `<div style="position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px;box-sizing:border-box;border-radius:8px;background:${t.panel};border:1px solid ${t.grooveDark};box-shadow:${t.panelShadow}"></div>`
    )
    .join("");
  const bezel = `<div style="position:absolute;left:326px;top:16px;width:620px;height:140px;box-sizing:border-box;border-radius:10px;background:${t.bezel};box-shadow:inset 0 1px 0 rgba(255,255,255,0.08), 0 1px 0 ${t.grooveLight}, 0 0 0 1px ${t.grooveDark}"></div>`;
  return page(1280, 800, `<div style="position:relative;width:1280px;height:800px;background:${t.plate}">${bezel}${secs}</div>`);
}

// Knob body with room for its drop shadow.
const PAD = 5;
function knob(t, ring) {
  const inset = Math.round(ring * 0.13) + 1;
  const d = ring - 2 * inset;
  const s = d + 2 * PAD;
  return [
    s,
    page(
      s,
      s,
      `<div style="position:absolute;left:${PAD}px;top:${PAD - 1}px;width:${d}px;height:${d}px;border-radius:50%;background:${t.knob};box-shadow:0 2px 4px rgba(0,0,0,0.35), 0 0 0 1px rgba(0,0,0,0.25)"></div>`
    ),
  ];
}

const JPAD = 8;
function jog(t, dimple = true) {
  const d = 104;
  const s = d + 2 * JPAD;
  return [
    s,
    page(
      s,
      s,
      `<div style="position:absolute;left:${JPAD}px;top:${JPAD - 3}px;width:${d}px;height:${d}px;border-radius:50%;background:${t.jog};box-shadow:0 3px 6px rgba(0,0,0,0.35), 0 0 0 1px ${t.grooveDark}">${dimple ? `<div style="position:absolute;left:62px;top:22px;width:18px;height:18px;border-radius:50%;background:${t.dimple};box-shadow:inset 0 2px 3px rgba(0,0,0,0.45), 0 1px 0 rgba(255,255,255,0.35)"></div>` : ""}</div>`
    ),
  ];
}

const tmp = join(tmpdir(), "rs92-assets");
mkdirSync(tmp, { recursive: true });

function shoot(name, w, h, html, scale) {
  const file = join(tmp, name + ".html");
  writeFileSync(file, html);
  const png = join(out, `${name}@${scale}x.png`);
  execFileSync(browser, [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    "--default-background-color=00000000",
    `--force-device-scale-factor=${scale}`,
    `--window-size=${w},${h}`,
    `--screenshot=${png}`,
    "file:///" + file.replace(/\\/g, "/"),
  ], { stdio: "ignore" });
  console.log(png);
}

for (const [tn, t] of Object.entries(themes)) {
  for (const scale of [1, 2]) {
    shoot(`plate_${tn}`, 1280, 800, plate(t), scale);
    for (const [kn, ring] of [["s", 38], ["m", 48], ["l", 60]]) {
      const [s, html] = knob(t, ring);
      shoot(`knob_${kn}_${tn}`, s, s, html, scale);
    }
    const [s, html] = jog(t);
    shoot(`jog_${tn}`, s, s, html, scale);
    const [sb, body] = jog(t, false);
    shoot(`jogbody_${tn}`, sb, sb, body, scale);
  }
}
rmSync(tmp, { recursive: true, force: true });
