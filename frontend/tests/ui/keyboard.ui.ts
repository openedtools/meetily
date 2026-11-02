import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';

// AC-UI-4: every control is reachable with Tab, shows a visible focus indicator,
// and is at least 24 by 24 px (WCAG 2.2 target size, minimum).
const MOCK = path.join(__dirname, 'mock-tauri.js');
const SCREENS = ['/', '/meeting-details?id=m1', '/people', '/settings'];
const MIN = 24;

type Stop = { key: string; label: string; w: number; h: number; focusVisible: boolean };

async function open(page: Page, url: string) {
  await page.addInitScript({ path: MOCK });
  await page.goto(url);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(800);
}

/** The element Tab landed on, with its size and whether any focus styling is visible. */
async function current(page: Page): Promise<Stop | null> {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return null;
    el.dataset.tabbed = '1';
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const outline = cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0;
    const ring = cs.boxShadow !== 'none' && cs.boxShadow !== '';
    const label = (el.getAttribute('aria-label') || el.textContent || el.getAttribute('placeholder') || el.tagName).trim().slice(0, 40);
    const key = `${el.tagName}|${label}|${Math.round(r.x)},${Math.round(r.y)}`;
    return { key, label: `${el.tagName.toLowerCase()} "${label}"`, w: r.width, h: r.height, focusVisible: outline || ring };
  });
}

for (const url of SCREENS) {
  test(`keyboard: ${url}`, async ({ page }) => {
    await open(page, url);
    const seen = new Map<string, Stop>();
    for (let i = 0; i < 120; i += 1) {
      await page.keyboard.press('Tab');
      const stop = await current(page);
      if (!stop) continue;
      if (seen.has(stop.key)) break; // wrapped around
      seen.set(stop.key, stop);
    }
    const stops = [...seen.values()];
    const noFocus = stops.filter((s) => !s.focusVisible).map((s) => s.label);
    const small = stops.filter((s) => s.w < MIN || s.h < MIN).map((s) => `${s.label} ${Math.round(s.w)}x${Math.round(s.h)}`);

    // Visible, tabbable controls that Tab never reached
    const reachable = await page.evaluate(() => {
      // Roving-focus widgets (tab lists, radio groups) keep inactive items at tabindex -1 on purpose:
      // they are reached with the arrow keys, and the container hands focus to the active item,
      // so neither is counted here.
      const sel = 'button:not([disabled]), a[href], input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]';
      return Array.from(document.querySelectorAll<HTMLElement>(sel))
        .filter((el) => {
          const r = el.getBoundingClientRect();
          const container = ['tablist', 'radiogroup', 'menubar', 'toolbar'].includes(el.getAttribute('role') ?? '');
          return !container && el.tabIndex >= 0 && !el.dataset.tabbed && r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
        })
        .map((el) => `${el.tagName.toLowerCase()} "${(el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 30)}"`);
    });

    const missing = reachable;
    console.log(`${url}: ${stops.length} tab stops`);
    if (missing.length) console.log(`  not reached by Tab: ${missing.join('; ')}`);
    if (noFocus.length) console.log(`  no visible focus: ${noFocus.join('; ')}`);
    if (small.length) console.log(`  smaller than ${MIN}px: ${small.join('; ')}`);
    expect(noFocus, 'every control shows a focus indicator').toEqual([]);
    expect(small, `every control is at least ${MIN} by ${MIN} px`).toEqual([]);
    expect(missing, 'Tab reaches every visible control').toEqual([]);
  });
}
