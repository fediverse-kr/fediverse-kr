// Use media-browser-fixture.ps1 and state-load .local/browser-state.json.
// Only synthetic local files and account are touched. No AP/network proof bypass.
async (page) => {
    const base='http://127.0.0.1:12239', context=page.context(), req=context.request;
    const checks=[],errors=[],unexpected=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(label,value)=>{if(!value)throw Error(label);checks.push(label);};
    const item=(await(await req.get(base+'/api/member/sites?page=0')).json()).sites?.[0];
    if(!item || item.edit.name!=='Browser owner fixture' || !item.closed || !/^browser-[a-f0-9-]+\.example\.org$/.test(item.domain)) throw Error('Synthetic owner fixture required');
    const id=item.domain.slice(8,-12), software='browser-media-'+id;
    const logo=base+'/api/public/software-logo/'+software, avatar=base+'/api/member/avatar',emoji=base+'/api/member/emoji/wave';
    const intercept=async route=>{unexpected.push(new URL(route.request().url()).pathname);await route.abort();};
    await context.route('https://media-fixture.invalid/**',intercept);
    await context.route(base+'/__media_probe',intercept);
    const anonymous=await context.browser().newContext();
    try {
        const metadata=await req.get(base+'/api/member/profile-media');
        const profile=await metadata.json();
        const compatibility=profile.emojis.includes('blob-cat.@/한국어');
        const names=compatibility?['wave','blob-cat.@/한국어','.','..','a+b','%2F']:['wave'];
        check('own media metadata',metadata.status()===200 && profile.avatar_available && profile.emojis.length===names.length && names.every(name=>profile.emojis.includes(name)));
        check('metadata contains no storage key or hash',!JSON.stringify(profile).includes('avatars/') && !JSON.stringify(profile).includes('sha256'));
        check('private metadata is not cached',metadata.headers()['cache-control'].includes('no-store'));
        for(const [path,mime,length] of [[logo,'image/svg+xml',null],[avatar,'image/svg+xml',null],[emoji,compatibility?'image/bmp':'image/png',compatibility?58:68]]) {
            const response=await req.get(path), bytes=await response.body(),headers=response.headers();
            check('real image bytes '+mime+' '+path.split('/').at(-2),response.status()===200 && headers['content-type']===mime && (length===null || bytes.length===length));
            check('image security headers '+path.split('/').at(-2),headers['cache-control'].includes('no-store') && headers['x-content-type-options']==='nosniff' && headers['content-security-policy'].includes('sandbox') && headers['cross-origin-resource-policy']==='same-origin');
            const head=await req.head(path);
            check('HEAD preserves length without bytes '+path.split('/').at(-2),head.status()===200 && Number(head.headers()['content-length'])===bytes.length && (await head.body()).length===0);
            const post=await req.post(path,{data:{},headers:{origin:base}});
            check('media endpoint is read-only '+path.split('/').at(-2),post.status()===405 && post.headers().allow==='GET, HEAD');
        }
        for(const name of names) {
            const path=base+'/api/member/emoji?name='+encodeURIComponent(name);
            const response=await req.get(path);
            check('canonical logical emoji lookup '+name,response.status()===200 && (await response.body()).equals(await (await req.get(emoji)).body()));
            check('canonical emoji stays private '+name,(await anonymous.request.get(path)).status()===401);
            check('canonical emoji blocks cross-site reads '+name,(await req.get(path,{headers:{'sec-fetch-site':'cross-site'}})).status()===403);
        }
        for(const path of [avatar,emoji,base+'/api/member/profile-media']) check('anonymous cannot read '+path.split('/').at(-1),(await anonymous.request.get(path)).status()===401);
        check('cross-site private image rejected',(await req.get(avatar,{headers:{'sec-fetch-site':'cross-site'}})).status()===403);
        check('unknown emoji stays missing',(await req.get(base+'/api/member/emoji/missing')).status()===404);
        check('software path traversal rejected',(await req.get(base+'/api/public/software-logo/%2Fetc%2Fpasswd')).status()===400);
        check('public logo available without login',(await anonymous.request.get(logo)).status()===200);
        const detail=await req.get(base+'/api/public/software/'+software);
        check('catalog projection exposes availability only',detail.status()===200 && (await detail.json()).software.logo_available && !(await detail.text()).includes('software-logos/'));
        const ownHtml=await req.get(base+'/account');
        check('own SSR excludes stored paths',ownHtml.status()===200 && ownHtml.headers()['cache-control'].includes('no-store') && !/avatars\/media-|emojis\/media-|software-logos\/media-/.test(await ownHtml.text()));
        const anonymousHtml=await anonymous.request.get(base+'/account');
        check('anonymous SSR has no legacy identity',!(await anonymousHtml.text()).includes('Browser test fixture') && !(await anonymousHtml.text()).includes('profile-avatar'));
        const open=async path=>{
            await page.goto(base+path);
            await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
            await page.locator('body').ariaSnapshot();
        };
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            for(const [path,selector,w,label] of [
                ['/account','.profile-avatar img',64,'account'],
                ['/platforms','.software-mark img',64,'catalog'],
                ['/software/'+software,'.page-heading .software-mark img',64,'software'],
                ['/servers/'+item.domain,'.page-heading .server-mark img',1,'site']
            ]) {
                await open(path);
                await page.locator(selector).scrollIntoViewIfNeeded();
                await page.waitForFunction(({selector,w})=>document.querySelector(selector)?.naturalWidth===w,{selector,w});
                check(label+' actually decodes image '+width,true);
                if(label==='account') {
                    const shown=compatibility?['wave','blob-cat.@/한국어','..']:['wave'];
                    await page.waitForFunction(count=>{const images=[...document.querySelectorAll('.profile-emoji img')];return images.length===count && images.every(img=>img.naturalWidth===1);},shown.length);
                    check('custom emoji accessible labels '+width,JSON.stringify(await page.locator('.profile-emoji img').evaluateAll(images=>images.map(img=>img.alt)))===JSON.stringify(shown.map(name=>':'+name+':')));
                    check('common UI preserves each logical name '+width,await page.locator('.profile-emoji img').evaluateAll((images,names)=>images.every((img,i)=>new URL(img.src).searchParams.get('name')===names[i]),shown));
                    check('name stays beside avatar '+width,await page.evaluate(()=>document.querySelector('.profile-avatar').getBoundingClientRect().right<=document.querySelector('.profile-summary').getBoundingClientRect().left));
                }
                if(label==='catalog' || label==='software') {
                    check('wrapped title stays beside logo '+width,await page.evaluate(()=>document.querySelector('.software-mark').getBoundingClientRect().right<=document.querySelector('.software-title > span:not(.media-mark)').getBoundingClientRect().left));
                }
                check(label+' no horizontal overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
                await page.evaluate(()=>window.scrollTo(0,0));
                await page.screenshot({path:`output/playwright/media-${label}-${width}.png`});
            }
        }
        await page.route(logo,route=>route.fulfill({status:200,contentType:'image/svg+xml',body:'invalid'}));
        try {
            await open('/software/'+software);
            await page.locator('.software-mark').scrollIntoViewIfNeeded();
            await page.waitForFunction(()=>!document.querySelector('.software-mark img') && document.querySelector('.software-mark')?.textContent==='M');
            check('broken logo falls back to initial',true);
        } finally {await page.unroute(logo);}
        const svgPage=await context.newPage();
        try {
            await svgPage.goto(logo);
            await svgPage.waitForLoadState('networkidle');
            check('direct SVG script is sandboxed',await svgPage.evaluate(()=>typeof window.__media_executed==='undefined'));
            check('CSP blocks SVG external image and fetch',unexpected.length===0);
        } finally {await svgPage.close();}
        await open('/account');
        const logout=await req.post(base+'/api/member/logout',{data:{},headers:{origin:base}});
        check('logout succeeds',logout.status()===200);
        check('private avatar unavailable after session revocation',(await req.get(avatar)).status()===401);
        check('canonical emoji unavailable after session revocation',(await req.get(base+'/api/member/emoji?name=..')).status()===401);
        check('no runtime errors',errors.length===0);
        return {passed:checks.length,errors,unexpectedRequests:unexpected.length};
    } finally {
        await anonymous.close();
        await context.unroute('https://media-fixture.invalid/**',intercept);
        await context.unroute(base+'/__media_probe',intercept);
    }
}
