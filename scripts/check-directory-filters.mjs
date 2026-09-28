async (page) => {
  const base = new URL(page.url()).origin;
  const checks = [];
  const assert = (ok, message) => { if (!ok) throw new Error(message); checks.push(message); };
  await page.goto(base + '/servers?software=mastodon&reg=open&sort=name&dir=desc&page_size=25');
  await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
  const panel = page.locator('.directory-filter-panel');
  assert(!(await panel.getAttribute('open')), 'Advanced filters start collapsed');
  assert(await page.getByRole('link', {name:'소프트웨어: Mastodon 해제',exact:true}).isVisible(), 'Applied software is visible and removable outside the collapsed panel');
  await page.getByRole('link', {name:'소프트웨어: Mastodon 해제',exact:true}).click();
  await page.waitForURL(url => !url.searchParams.has('software'));
  const url = new URL(page.url());
  assert(url.searchParams.get('reg')==='open' && url.searchParams.get('sort')==='name' && url.searchParams.get('dir')==='desc' && url.searchParams.get('page_size')==='25', 'Removing one filter preserves unrelated filters and sort');
  await page.locator('.directory-filter-panel > summary').click();
  assert(await page.getByLabel('가입 방식',{exact:true}).isVisible(), 'Expanding filters exposes native labelled controls');
  await page.getByLabel('가입 방식',{exact:true}).selectOption('invite_only');
  assert(await page.getByText('변경한 조건을 적용해 주세요.',{exact:true}).isVisible(), 'Unsaved filter changes have explicit pending feedback');
  await page.getByRole('button',{name:'조건 적용',exact:true}).click();
  await page.waitForURL(url => url.searchParams.get('reg')==='invite_only');
  await page.getByRole('link',{name:'가입 방식: 초대 필요 해제',exact:true}).waitFor({state:'visible'});
  assert(await page.getByRole('link',{name:'가입 방식: 초대 필요 해제',exact:true}).isVisible(), 'Applied choice is reflected after route content resolves');
  await page.reload();await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
  assert(await page.getByRole('link',{name:'가입 방식: 초대 필요 해제',exact:true}).isVisible(), 'Applied filters survive reload');
  await page.getByRole('link',{name:'모두 해제',exact:true}).click();
  await page.waitForURL(url => url.pathname==='/servers' && !url.search);
  await page.waitForFunction(()=>!document.querySelector('.active-filter-chips') && !!document.querySelector('.directory-result-count'));
  assert(await page.locator('.active-filter-chips a').count()===0, 'Reset restores the canonical unfiltered route');
  return {checks};
}
