async (page) => {
  const base = new URL(page.url()).origin;
  const checks = [];
  const assert = (ok, message) => { if (!ok) throw new Error(message); checks.push(message); };
  for (const width of [1920,390]) {
    await page.setViewportSize({width,height:1080});
    await page.goto(base+'/software/mastodon');
    await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    assert(await page.locator('main h1').count()===1,'One software title at '+width);
    const cta = page.locator('main header').getByRole('link',{name:'이 소프트웨어의 서버 찾기',exact:true});
    assert(await cta.isVisible(),'Primary server action belongs to the software overview at '+width);
    const r=await cta.boundingBox(); assert(r.y+r.height<1080,'Primary next action appears in the first viewport at '+width);
    assert(await page.getByRole('heading',{name:'주요 기능',exact:true}).isVisible(),'Features can be scanned at '+width);
    assert(await page.evaluate(()=>document.documentElement.scrollWidth===innerWidth),'No software detail horizontal overflow at '+width);
    await page.screenshot({path:`software-detail-${width}.png`,fullPage:true});
  }
  await page.getByRole('link',{name:'이 소프트웨어의 서버 찾기',exact:true}).click();
  await page.waitForURL(base+'/software/mastodon/servers');
  await page.locator('.server-grid').waitFor({state:'visible'});
  assert(await page.locator('.server-grid').isVisible(),'The primary action opens the matching server directory');
  return {checks};
}
