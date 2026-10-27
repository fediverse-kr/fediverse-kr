// Public, read-only check: demo chrome must never behave as an app.
// PLAYWRIGHT_MODULE may point at an existing local Playwright installation.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const base = process.env.BASE_URL || 'http://127.0.0.1:12239';
const browser = await chromium.launch({headless: true, ...(process.env.CHROMIUM_PATH ? {executablePath: process.env.CHROMIUM_PATH} : {})});
const checks = [];
const errors = [];
const check = (name, passed) => { assert.ok(passed, name); checks.push(name); };
try {
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', e => { if (['error', 'warning'].includes(e.type())) errors.push(e.text()); });
  const open = async path => {
    await page.goto(base + path);
    await page.waitForSelector('.portal-root[data-ready="true"]');
  };
  const inspect = async label => {
    const state = await page.locator('.social-window').evaluateAll(windows => ({
      count: windows.length,
      inert: windows.every(w => w.inert),
      controls: windows.flatMap(w => [...w.querySelectorAll('button,input,textarea,select,a[href],[contenteditable="true"],[role="button"],[role="textbox"]')].map(e => e.outerHTML.slice(0,180))),
      scroll: windows.every(w => getComputedStyle(w.querySelector('.social-feed')).overflowY === 'hidden')
    }));
    check(label + ': mock windows present', state.count > 0);
    check(label + ': inert mock windows', state.inert);
    check(label + ': no fake controls: ' + state.controls.join(', '), state.controls.length === 0);
    check(label + ': no manual inner scrolling', state.scroll);
  };
  await open('/');
  await inspect('home');
  for (let scene = 0; scene < 3; scene++) {
    await page.waitForFunction(scene => document.querySelector('.landing-demo')?.dataset.scene === String(scene) && document.querySelector('.landing-demo')?.dataset.sceneTransition === 'idle', scene);
    // Observe the compose/photo/editor stage, then pause with the real control.
    await page.waitForFunction(() => Number(document.querySelector('.landing-demo')?.dataset.tick) >= 15);
    await page.getByRole('button', {name:'애니메이션 일시 정지', exact:true}).click();
    await inspect('home scene ' + scene);
    const before = await page.locator('.landing-demo').getAttribute('data-tick');
    const window = page.locator('.social-window[data-visible="true"]');
    const markup = await window.innerHTML();
    await window.click({force:true});
    await page.mouse.wheel(0, 300);
    await page.keyboard.press('Enter');
    await page.keyboard.press('Space');
    await page.waitForTimeout(250);
    check('scene ' + scene + ': chrome does not mutate', await window.innerHTML() === markup);
    check('scene ' + scene + ': playback pause works', await page.locator('.landing-demo').getAttribute('data-tick') === before);
    const focusEscapes = await page.locator('.social-window').evaluateAll(windows => windows.every(w => {
      w.querySelectorAll('*').forEach(e => e.focus?.());
      return !w.contains(document.activeElement);
    }));
    check('scene ' + scene + ': cannot focus mock descendants', focusEscapes);
    await page.getByRole('button', {name:'애니메이션 재생', exact:true}).click();
    await page.waitForFunction(t => Number(document.querySelector('.landing-demo')?.dataset.tick) > Number(t), before);
    await page.getByRole('button', {name:'다음 이용 장면', exact:true}).click();
  }
  for (const topic of ['', 'delivery', 'visibility', 'boundaries', 'find', 'moving']) {
    await open('/start' + (topic ? '/' + topic : ''));
    await page.waitForFunction(() => !!window.__fedkrExplainerScroll);
    const controls = page.getByRole('group', {name:'장면 바로 보기'}).getByRole('button');
    for (let step = 0; step < await controls.count(); step++) {
      await controls.nth(step).click();
      await page.waitForFunction(step => document.querySelector('#ex-scrolly')?.dataset.step === String(step), step);
      await inspect((topic || 'basics') + ' step ' + step);
    }
  }
  for (const width of [1440,390]) {
    await page.setViewportSize({width,height:1000});
    await open('/');
    await page.waitForFunction(() => Number(document.querySelector('.landing-demo')?.dataset.tick) >= 15);
    await page.getByRole('button',{name:'애니메이션 일시 정지',exact:true}).click();
    await inspect('home width ' + width);
    check('no overflow ' + width, await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    if (width === 390) {
      await page.getByRole('group', {name:'모바일에서 볼 사이트'}).getByRole('button').nth(1).click();
      check('mobile view selection works', await page.locator('.social-window[data-visible="true"]').getAttribute('data-remote') === 'true');
    }
    if (process.env.SCREENSHOT_DIR) {
      await fs.mkdir(process.env.SCREENSHOT_DIR,{recursive:true});
      await page.screenshot({path:`${process.env.SCREENSHOT_DIR}/home-${width}.png`,fullPage:true});
    }
  }
  const noJs = await browser.newContext({javaScriptEnabled:false});
  try {
    const p = await noJs.newPage();
    await p.goto(base + '/');
    check('SSR is already inert before hydration', await p.locator('.social-window').evaluateAll(ws => ws.length > 0 && ws.every(w => w.inert)));
  } finally { await noJs.close(); }
  check('no browser warnings or errors', errors.length === 0);
  console.log(JSON.stringify({passed:checks.length, checks, errors}));
} finally { await browser.close(); }
