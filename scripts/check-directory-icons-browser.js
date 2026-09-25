// Requires isolated member-browser-fixture.ps1 -IncludeSite -IncludeIcon,
// optionally -IncludeSvgIcon to check recognized SVG and direct-document CSP.
// database-backed localhost preview, and state-load .local/browser-state.json.
// No remote reads. Changes visibility of that generated owner fixture only.
async (page) => {
    const base='http://127.0.0.1:12239', req=page.context().request, checks=[], errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    let item=(await(await req.get(base+'/api/member/sites?page=0')).json()).sites?.[0];
    if(!item || item.edit.name!=='Browser owner fixture' || !item.closed || !/^browser-[a-f0-9-]+\.example\.org$/.test(item.domain)) throw Error('Generated fixture required');
    const icon=base+'/api/public/server-icon/'+item.domain;
    const unexpected=[];
    const intercept=async route=>{unexpected.push(true);await route.abort();};
    await page.context().route('https://icon-fixture.invalid/**',intercept);
    await page.context().route(base+'/__icon_probe',intercept);
    await page.unroute(icon); // Clear only this test's route after an interrupted run.
    const image=await req.get(icon);
    const svg=image.headers()['content-type']==='image/svg+xml';
    const bytes=await image.body(), width=svg?64:1;
    check('recognized image served from local DB',image.status()===200 && (svg?bytes.includes('window.__icon_executed'):image.headers()['content-type']==='image/png' && bytes.length===68));
    check('untrusted image security headers',image.headers()['x-content-type-options']==='nosniff' && image.headers()['cache-control']==='no-store' && image.headers()['content-security-policy'].includes('sandbox') && image.headers()['cross-origin-resource-policy']==='same-origin');
    const head=await req.head(icon);
    check('HEAD has original byte length, no body',head.status()===200 && Number(head.headers()['content-length'])===bytes.length && (await head.body()).length===0);
    const wrongMethod=await req.post(icon,{data:{}});
    check('read-only icon endpoint',wrongMethod.status()===405 && wrongMethod.headers().allow==='GET, HEAD');
    check('unregistered site icon returns 404',(await req.get(base+'/api/public/server-icon/unregistered.example.org')).status()===404);
    for(const bad of ['127.0.0.1','localhost','foo.example.org/extra','%2Fetc%2Fpasswd']) check('bad icon path rejected '+bad,(await req.get(base+'/api/public/server-icon/'+bad)).status()===400);
    const open=async path=>{await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    await open('/servers?q='+encodeURIComponent(item.domain));
    await page.locator('.server-mark img').scrollIntoViewIfNeeded();
    await page.waitForFunction(width=>document.querySelector('.server-mark img')?.naturalWidth===width,width);
    check('cached icon actually renders in list',await page.locator('.server-mark img').count()===1);
    await page.locator('.server-card').click();
    await page.locator('.page-heading .server-mark').scrollIntoViewIfNeeded();
    await page.waitForFunction(width=>document.querySelector('.page-heading .server-mark img')?.naturalWidth===width,width);
    check('cached icon renders in detail',true);
    await page.screenshot({path:'output/playwright/site-icon-'+(svg?'svg':'png')+'.png'});
    if(svg){
        const documentPage=await page.context().newPage();
        try {
            await documentPage.goto(icon);
            await documentPage.waitForLoadState('networkidle');
            check('direct SVG scripts stay sandboxed',await documentPage.evaluate(()=>typeof window.__icon_executed==='undefined'));
            check('SVG inline styles remain visible',await documentPage.evaluate(()=>getComputedStyle(document.querySelector('circle')).fill==='rgb(36, 122, 81)'));
            check('SVG cannot request remote or same-origin resources',unexpected.length===0);
        }finally{await documentPage.close();}
    }
    await page.route(icon,route=>route.fulfill({status:200,contentType:'image/png',body:'not an image'}));
    try {
        await open('/servers/'+item.domain);
        await page.locator('.page-heading .server-mark').scrollIntoViewIfNeeded();
        await page.waitForFunction(()=>!document.querySelector('.page-heading .server-mark img') && document.querySelector('.page-heading .server-mark')?.textContent==='B');
        check('invalid image decoding falls back to initial',true);
    } finally {await page.unroute(icon);}
    const save=async(hidden)=>{
        item=(await(await req.get(base+'/api/member/sites?page=0')).json()).sites[0];
        const changes={...item.edit,hidden};
        const result=await req.post(base+'/api/member/sites/edit',{data:{domain:item.domain,revision:item.revision,changes},headers:{origin:base}});
        check('fixture visibility saved '+hidden,result.status()===200);
    };
    try {
        await save(true);
        const hidden=await req.get(icon);
        check('icon disappears on next read after hiding',hidden.status()===404 && hidden.headers()['cache-control']==='no-store');
        const search=await req.get(base+'/api/public/server-search?filters='+encodeURIComponent('q='+item.domain));
        check('hidden site not in new search',(await search.json()).total===0);
    } finally {await save(false);}
    check('visible icon restored',(await req.get(icon)).status()===200);
    check('no browser exceptions',errors.length===0);
    await page.context().unroute('https://icon-fixture.invalid/**',intercept);
    await page.context().unroute(base+'/__icon_probe',intercept);
    return {passed:checks.length,errors};
}
