// Generates raster brand assets from the SVG master brand/icon.svg (spec F-UI-9, plan WP7):
//   brand/generated/icon-1024.png      input for `pnpm tauri icon` (all app icon sizes and the .ico)
//   brand/generated/preview-<n>.png    the icon at 16, 32, 64 and 256 px, for the AC-UI-6 check
//   src-tauri/icons/installer/header.bmp   150 x 57, Windows installer header
//   src-tauri/icons/installer/sidebar.bmp  164 x 314, Windows installer welcome and finish pages
//   public/icon.svg                     the mark used inside the app
// Usage: node scripts/brand-assets.mjs   (uses Playwright's Chromium; no network)
import { chromium } from '@playwright/test';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const FRONTEND = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BRAND = path.resolve(FRONTEND, '..', 'brand');
const GENERATED = path.join(BRAND, 'generated');
const INSTALLER = path.join(FRONTEND, 'src-tauri', 'icons', 'installer');
const svg = readFileSync(path.join(BRAND, 'icon.svg'), 'utf8');
const markOnly = svg.replace(/<rect[^>]*\/>/, ''); // the rings and tail without the cream tile

const PAPER = '#FAF6EE';
const LINE = '#E4DACA';

/** 24-bit bottom-up BMP from RGBA pixels (what NSIS expects). */
export function encodeBmp(width, height, rgba) {
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const size = 54 + rowSize * height;
  const buf = Buffer.alloc(size);
  buf.write('BM', 0);
  buf.writeUInt32LE(size, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(rowSize * height, 34);
  for (let y = 0; y < height; y += 1) {
    const row = 54 + (height - 1 - y) * rowSize;
    for (let x = 0; x < width; x += 1) {
      const i = (y * width + x) * 4;
      buf[row + x * 3] = rgba[i + 2];
      buf[row + x * 3 + 1] = rgba[i + 1];
      buf[row + x * 3 + 2] = rgba[i];
    }
  }
  return buf;
}

async function render(page, html, width, height) {
  await page.setViewportSize({ width, height });
  await page.setContent(`<html><body style="margin:0;width:${width}px;height:${height}px;overflow:hidden">${html}</body></html>`);
  return page;
}

async function pixels(page, width, height) {
  const png = await page.screenshot({ clip: { x: 0, y: 0, width, height } });
  return page.evaluate(async ({ b64, width, height }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);
    return Array.from(ctx.getImageData(0, 0, width, height).data);
  }, { b64: png.toString('base64'), width, height });
}

const sized = (s, w, h) => s.replace('<svg ', `<svg width="${w}" height="${h}" `);

async function main() {
  mkdirSync(GENERATED, { recursive: true });
  mkdirSync(INSTALLER, { recursive: true });
  const browser = await chromium.launch();
  const page = await browser.newPage();

  await render(page, sized(svg, 1024, 1024), 1024, 1024);
  await page.screenshot({ path: path.join(GENERATED, 'icon-1024.png'), omitBackground: true });

  for (const n of [16, 32, 64, 256]) {
    await render(page, sized(svg, n, n), n, n);
    await page.screenshot({ path: path.join(GENERATED, `preview-${n}.png`), omitBackground: true });
  }

  // Header: cream with the mark at the right, no text (NSIS draws its own title)
  await render(page, `<div style="width:150px;height:57px;background:${PAPER};position:relative">
    <div style="position:absolute;right:8px;top:6px">${sized(markOnly, 45, 45)}</div></div>`, 150, 57);
  writeFileSync(path.join(INSTALLER, 'header.bmp'), encodeBmp(150, 57, await pixels(page, 150, 57)));

  // Sidebar: cream, one large pale sand circle partly off the edge, the mark near the bottom
  await render(page, `<div style="width:164px;height:314px;background:${PAPER};position:relative;overflow:hidden">
    <div style="position:absolute;left:-70px;top:-40px;width:230px;height:230px;border-radius:50%;background:${LINE};opacity:0.55"></div>
    <div style="position:absolute;left:46px;bottom:34px">${sized(markOnly, 72, 72)}</div></div>`, 164, 314);
  writeFileSync(path.join(INSTALLER, 'sidebar.bmp'), encodeBmp(164, 314, await pixels(page, 164, 314)));

  await browser.close();
  copyFileSync(path.join(BRAND, 'icon.svg'), path.join(FRONTEND, 'public', 'icon.svg'));
  console.log('brand assets written: icon-1024.png, previews, installer header and sidebar, public/icon.svg');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) await main();
