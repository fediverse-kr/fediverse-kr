// Run against a built release with the public catalog or its disposable preview.
async (page) => {
  const base = new URL(page.url()).origin;
  const checks = [];
  const assert = (ok, message) => { if (!ok) throw new Error(message); checks.push(message); };
  const ready = () => page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
  for (const width of [1920, 390, 320]) {
    await page.setViewportSize({width, height:1080});
    await page.goto(base + '/software/mastodon');
    await ready();
    assert(await page.locator('main h1').count() === 1, 'One software title at ' + width);
    const cta = page.locator('.software-primary-actions').getByRole('link', {name:'이 소프트웨어의 서버 찾기', exact:true});
    assert(await cta.isVisible(), 'Primary server action is visible at ' + width);
    const r = await cta.boundingBox();
    assert(r.y + r.height < 1080, 'Primary next action appears in the first viewport at ' + width);
    assert(await page.getByRole('heading', {name:'특징 한눈에', exact:true}).isVisible(), 'Registered features render at ' + width);
    await page.waitForFunction(() => Array.from(document.querySelectorAll('.software-product-hero img')).every(i => i.complete && i.naturalWidth > 0));
    assert(await page.locator('.software-product-hero img').count() >= 2, 'Official mark and product image load at ' + width);
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'No software detail horizontal overflow at ' + width);
    await page.screenshot({path:`software-detail-${width}.png`, fullPage:true});
  }
  await page.getByRole('link', {name:'이 소프트웨어의 서버 찾기', exact:true}).click();
  await page.waitForURL(base + '/software/mastodon/servers');
  await page.locator('.server-grid').waitFor({state:'visible'});
  assert(await page.locator('.server-grid').isVisible(), 'Primary action opens matching server directory');
  await page.goto(base + '/platforms');
  await ready();
  assert(await page.locator('.software-example').count() === 3, 'Three editorial examples supplement the full directory');
  assert(await page.locator('.software-family-row').count() >= 3, 'Registered families are discoverable');
  // Search is authoritative: do not assume a product appears on the first page.
  await page.getByRole('searchbox', {name:'소프트웨어 이름 검색'}).fill('mastodon');
  await page.getByRole('button', {name:'검색', exact:true}).click();
  await page.waitForURL(url => url.searchParams.get('q') === 'mastodon');
  const mastodon = page.locator('a.software-directory-item[href="/software/mastodon"]');
  await mastodon.waitFor({state:'visible'});
  await mastodon.scrollIntoViewIfNeeded();
  await page.waitForFunction(() => document.querySelector('a.software-directory-item[href="/software/mastodon"] .software-emblem img')?.naturalWidth > 0);
  assert(await mastodon.locator('.software-emblem img').count() === 1, 'Catalog mark loads after hydrated search');
  await mastodon.click();
  await page.waitForURL(base + '/software/mastodon');
  await page.locator('main h1').filter({hasText:'Mastodon'}).waitFor();
  assert(await page.locator('.software-family-index').isVisible(), 'Detail retains registered family navigation');
  await page.getByRole('link', {name:'Glitch-soc', exact:true}).click();
  await page.waitForURL(base + '/software/glitch-soc');
  await page.locator('main h1').filter({hasText:'Glitch-soc'}).waitFor();
  assert(await page.locator('.software-artwork-fallback').isVisible(), 'Unbundled fork has its own generated artwork, not the parent brand');
  assert(!(await page.locator('main').innerText()).includes('자동 생성한 표지'), 'Decorative covers do not expose generation details');
  assert(!(await page.locator('main').innerText()).includes('제품 이미지 미등록'), 'Decorative covers do not expose editorial missing-asset state');
  await page.reload(); await ready();
  assert((await page.locator('main h1').innerText()).includes('Glitch-soc'), 'Same-route product navigation survives reload');
  await page.goto(base + '/platforms?q=wildebeest'); await ready();
  const retired = page.locator('a.software-directory-item[href="/software/wildebeest"]');
  assert(await retired.getAttribute('data-inactive') === 'true', 'Discontinued software is marked in search');
  assert(await retired.locator('.software-status').innerText() === '개발 중단', 'Discontinued label is explicit, not just color');
  assert((await retired.locator('h3').evaluate(el => getComputedStyle(el).textDecorationLine)).includes('line-through'), 'Discontinued title is visibly struck through');
  await retired.click(); await page.waitForURL(base + '/software/wildebeest');
  await page.locator('main h1').filter({hasText:'Wildebeest'}).waitFor();
  assert(await page.locator('.software-heading .software-status').innerText() === '개발 중단', 'Detail retains development status');
  assert(await page.locator('.software-family-anchor[data-inactive="true"]').count() === 1, 'Family anchor retains development status');
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Status badges do not overflow mobile layout');
  return {checks};
}
