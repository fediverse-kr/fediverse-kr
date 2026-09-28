async (page) => {
 const base=new URL(page.url()).origin,checks=[];
 const check=(ok,message)=>{if(!ok)throw new Error(message);checks.push(message)};
 for(const width of [1920,820,390]){
  await page.setViewportSize({width,height:1080});await page.goto(base+'/servers');
  await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
  const cards=page.locator('.server-card');await cards.first().waitFor();
  check(await cards.locator('.server-identity').count()===await cards.count(),'Every server retains a separate identity row');
  const both=cards.filter({has:page.locator('.server-cover')}).filter({has:page.locator('.site-icon img')}).first();
  await both.locator('.server-cover img').waitFor();
  for(const el of [both.locator('.server-cover img'),both.locator('.site-icon img')])await el.evaluate(e=>e.decode());
  const data=await both.evaluate(el=>{const cover=el.querySelector('.server-cover img'),icon=el.querySelector('.site-icon img');return {cw:cover.getBoundingClientRect().width,ch:cover.getBoundingClientRect().height,iw:icon.getBoundingClientRect().width,ih:icon.getBoundingClientRect().height,cover:cover.src,icon:icon.src}});
  check(data.cw>=180&&data.ch>=100,`Header remains recognizable at ${width}px`);
  check(data.iw<=32&&data.ih<=32&&data.cover!==data.icon,`Independent small identity icon at ${width}px`);
  check(await cards.locator('.site-icon').count()===await cards.count(),'Even image-less servers have an identity mark');
  const noHeader=cards.filter({hasNot:page.locator('.server-cover')});
  check(await noHeader.count()>0,'Header-less servers do not reserve a blank image frame');
  check(await noHeader.first().evaluate(e=>e.querySelector('.server-identity').getBoundingClientRect().top-e.getBoundingClientRect().top<40),'Header-less card begins with identity, not empty image space');
  const tiny=cards.locator('.site-icon img');
  await tiny.evaluateAll(es=>{es.forEach(e=>e.loading='eager');return Promise.all(es.map(e=>e.decode()))});
  const images=await tiny.evaluateAll(es=>es.map(e=>({natural:e.naturalWidth,display:e.getBoundingClientRect().width})));
  check(images.some(i=>i.natural===16&&i.display<=16),'A real 16px favicon is not enlarged');
  check(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`No card overflow at ${width}px`);
  await page.screenshot({path:`server-images-${width}.png`,fullPage:true});
 }
 for(const [role,domain,status] of [['header','fixture-1.example.com',404],['header','fixture-4.example.com',200],['icon','fixture-4.example.com',404],['header','fixture-5.example.com',404]]){
  const response=await page.request.get(`${base}/api/public/server-${role}/${domain}`);
  check(response.status()===status,`${role} endpoint preserves its role for ${domain}`);
  check(response.headers()['cache-control']==='no-store','Image endpoints recheck visibility instead of retaining readable caches');
 }
 const head=await page.request.head(base+'/api/public/server-header/fixture-0.example.com');
 check(head.status()===200&&(await head.body()).length===0&&Number(head.headers()['content-length'])>0,'Header endpoint supports correct HEAD semantics');
 const post=await page.request.post(base+'/api/public/server-header/fixture-0.example.com');
 check(post.status()===405,'Header endpoint rejects mutation methods');
 const coverCard=page.locator('.server-card').filter({has:page.locator('.server-cover')}).first();await coverCard.click();
 await page.locator('.page-heading .server-cover img').waitFor();
 check(await page.locator('.page-heading .site-icon').count()===1,'Detail keeps icon alongside optional header');
 await page.screenshot({path:'server-image-detail-390.png',fullPage:true});
 await page.route('**/api/public/server-header/fixture-0.example.com',route=>route.fulfill({status:404,body:''}));
 await page.goto(base+'/servers?q=fixture-0.example.com');
 await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true'&&document.querySelector('.server-card .site-icon img')&&!document.querySelector('.server-card .server-cover'));
 check(await page.locator('.server-card .site-icon img').count()===1,'Failed header collapses cleanly without losing the identity icon');
 await page.screenshot({path:'server-image-unavailable-390.png',fullPage:false});
 return {status:'PASS',checks};
}