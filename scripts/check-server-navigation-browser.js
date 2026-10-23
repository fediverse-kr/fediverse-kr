async (page) => {
  const origin = new URL(page.url()).origin;
  const errors = [];
  const passed = [];
  const onConsole = (message) => {
    const text = message.text();
    // Rust tracing can emit errors through console.info rather than console.error.
    if (message.type() === 'error' || /cannot reclaim|panicked at|\bERROR\b/.test(text)) {
      errors.push({ route: new URL(page.url()).pathname, text });
    }
  };
  const onError = (error) => errors.push({ route: new URL(page.url()).pathname, text: error.message });
  page.on('console', onConsole);
  page.on('pageerror', onError);
  const check = (condition, label) => {
    if (!condition) throw new Error(label);
    passed.push(label);
  };
  const listing = async () => {
    await page.getByRole('searchbox', { name: '서버 이름, 주소, 소프트웨어 검색' }).waitFor();
    await page.locator('.server-card').first().waitFor();
  };
  const detail = async (name) => {
    await page.getByRole('heading', { name, exact: true, level: 1 }).waitFor();
    await page.locator('.server-health-summary').waitFor();
    await page.locator('.community-section').waitFor();
  };
  try {
    await page.goto(origin + '/servers');
    await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
    await listing();
    await page.evaluate(() => { window.__navigationRegressionDocument = true; });
    const cards = await page.locator('.server-card').evaluateAll(nodes => nodes.slice(0, 2).map(node => ({
      href: node.getAttribute('href'), name: node.querySelector('h2').textContent,
    })));
    check(cards.length === 2, 'Two server destinations available');
    // Hold real auxiliary responses: the primary detail must remain usable
    // before either panel resolves, and one resolving must not reset the other.
    let releaseHealth, releaseComments;
    let sawHealth, sawComments;
    const healthSeen = new Promise(resolve => { sawHealth = resolve; });
    const commentsSeen = new Promise(resolve => { sawComments = resolve; });
    const healthGate = new Promise(resolve => { releaseHealth = resolve; });
    const commentsGate = new Promise(resolve => { releaseComments = resolve; });
    const healthRoute = async route => {
      const response = await route.fetch(); sawHealth();
      await healthGate; await route.fulfill({ response });
    };
    const commentsRoute = async route => {
      const response = await route.fetch(); sawComments();
      await commentsGate; await route.fulfill({ response });
    };
    await page.route('**/api/public/server-health?*', healthRoute);
    await page.route('**/api/public/comments?*', commentsRoute);
    try {
      await page.locator('.server-card').first().click();
      await Promise.all([healthSeen, commentsSeen]);
      await page.getByRole('heading', { name: cards[0].name, exact: true, level: 1 }).waitFor({ timeout: 5000 });
      check(await page.locator('.route-loading-content').count() === 0, 'Slow panels do not hide the primary detail');
      releaseHealth();
      await page.locator('.server-health-summary').waitFor();
      check(await page.getByRole('heading', { name: cards[0].name, exact: true, level: 1 }).isVisible(), 'Health completion retains detail while comments remain pending');
      check(errors.length === 0, 'No reclamation error during staggered panel completion');
    } finally {
      releaseHealth(); releaseComments();
      await page.unroute('**/api/public/server-health?*', healthRoute);
      await page.unroute('**/api/public/comments?*', commentsRoute);
    }
    await page.locator('.community-section').waitFor();
    await page.locator('a.back-link').click();
    await listing();
    for (const card of cards) {
      await page.locator('.server-card').filter({ has: page.getByRole('heading', { name: card.name, exact: true }) }).first().click();
      await detail(card.name);
      check(new URL(page.url()).pathname === card.href, 'Native detail navigation: ' + card.href);
      await page.goBack();
      await listing();
      check(new URL(page.url()).pathname === '/servers', 'Back restores directory');
      await page.goForward();
      await detail(card.name);
      check(await page.evaluate(() => window.__navigationRegressionDocument === true), 'Navigation did not reload the document');
      await page.locator('a.back-link').click();
      await listing();
    }
    await page.reload();
    await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
    await listing();
    await page.locator('.server-card').first().click();
    await detail(cards[0].name);
    check(errors.length === 0, 'No browser/WASM errors across navigation: ' + JSON.stringify(errors));
    return { passed, errors };
  } finally {
    page.off('console', onConsole);
    page.off('pageerror', onError);
  }
}
