async (page) => {
  const base = new URL(page.url()).origin;
  const checks = [];
  const assert = (ok, message) => { if (!ok) throw new Error(message); checks.push(message); };
  for (const [width, height, columns] of [[1920,1080,4],[820,1000,2],[390,844,1]]) {
    await page.setViewportSize({width,height});
    for (const [route, selector] of [['/platforms','.software-grid'],['/servers','.server-grid']]) {
      await page.goto(base + route);
      await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
      const layout = await page.locator(selector).evaluate(e => ({
        columns: getComputedStyle(e).gridTemplateColumns.split(' ').length,
        widths: [...e.children].map(c=>c.getBoundingClientRect().width),
        overflow: document.documentElement.scrollWidth - innerWidth,
        width: innerWidth
      }));
      await page.screenshot({path:`directory-${route.slice(1)}-${width}.png`,fullPage:false});
      assert(layout.columns === columns, `${route} at ${width}px: ${columns} columns, got ${layout.columns}`);
      assert(layout.width === width && layout.overflow === 0, `${route} at ${width}px: exact viewport and no horizontal overflow`);
      if (layout.widths.length > 1) assert(Math.max(...layout.widths)-Math.min(...layout.widths)<1, `${route} at ${width}px: equal card widths`);
    }
  }
  return {checks};
}
