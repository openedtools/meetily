import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';

const OUT = path.join(__dirname, 'screenshots');
const MOCK = path.join(__dirname, 'mock-tauri.js');

async function open(page: Page, url: string) {
  await page.addInitScript({ path: MOCK });
  await page.goto(url);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(800);
}

async function shot(page: Page, name: string) {
  const theme = test.info().project.name;
  await page.screenshot({ path: path.join(OUT, `${name}-${theme}.png`) });
}

test('record screen', async ({ page }) => {
  await open(page, '/');
  await shot(page, '01-record');
});

test('meeting: transcript and summary', async ({ page }) => {
  await open(page, '/meeting-details?id=m1');
  await shot(page, '02-meeting');
});

test('people', async ({ page }) => {
  await open(page, '/people');
  await shot(page, '03-people');
  const first = page.getByText('Alex Morgan').first();
  if (await first.isVisible()) {
    await first.click();
    await page.waitForTimeout(500);
    await shot(page, '04-person');
  }
});

for (const tab of ['general', 'recording', 'Transcriptionmodels', 'summaryModels', 'beta']) {
  test(`settings: ${tab}`, async ({ page }) => {
    await open(page, '/settings');
    const trigger = page.locator(`[role="tab"][data-value="${tab}"], [role="tab"][id$="-trigger-${tab}"]`).first();
    if (await trigger.count()) await trigger.click();
    await page.waitForTimeout(500);
    await shot(page, `05-settings-${tab}`);
  });
}

test('onboarding', async ({ page }) => {
  await open(page, '/?onboarding');
  await shot(page, '06-onboarding-1');
  const next = page.getByRole('button', { name: /get started|continue|next/i }).first();
  if (await next.isVisible()) {
    await next.click();
    await page.waitForTimeout(500);
    await shot(page, '06-onboarding-2');
  }
});

test('dialog: delete meeting', async ({ page }) => {
  await open(page, '/');
  const item = page.getByText('Course design sync').first();
  await item.hover();
  const del = page.getByRole('button', { name: /delete/i }).first();
  if (await del.isVisible()) {
    await del.click();
    await page.waitForTimeout(400);
    await shot(page, '07-dialog-delete');
  }
});

test('no page errors on the main screens', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  for (const url of ['/', '/meeting-details?id=m1', '/people', '/settings']) await open(page, url);
  expect(errors).toEqual([]);
});
