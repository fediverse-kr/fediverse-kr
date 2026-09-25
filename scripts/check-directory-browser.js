// playwright-cli -s=fedkr-review run-code --filename scripts/check-directory-browser.js
// Explicit fictional preview. Read-only HTTP and local browser state only.
async (page) => {
    const base='http://127.0.0.1:12239', checks=[], errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    const open=async(path)=>{await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    const req=page.context().request;
    check('preview only',(await(await req.get(base+'/api/public/catalog')).json()).preview===true);
    for(const [width,height] of [[1440,1000],[390,844],[320,667]]) {
        await page.setViewportSize({width,height});
        await open('/servers');
        check(width+' all nine controls visible',await page.getByRole('combobox').count()===9);
        check(width+' no horizontal overflow',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
        check(width+' unknown source status retained',(await page.locator('.server-card').allTextContents()).every(t=>t.includes('아직 확인 전')));
        await page.getByLabel('가입 방식',{exact:true}).selectOption('invite_only');
        const origin=await page.evaluate(()=>performance.timeOrigin);
        await page.getByRole('button',{name:'조건 적용',exact:true}).click();
        await page.waitForURL('**/servers?reg=invite_only');
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='1곳');
        check(width+' invite-only result',await page.locator('.server-card h2').innerText()==='냥냥.타워');
        check(width+' SPA retains document',await page.evaluate(()=>performance.timeOrigin)===origin);
        check(width+' excluded count',(await page.locator('.directory-filter-excluded').innerText()).includes('3곳'));
        await page.reload();await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        check(width+' reload restores filter',await page.getByLabel('가입 방식',{exact:true}).inputValue()==='invite_only');
        await page.getByRole('link',{name:'가입 방식 모두 보기',exact:true}).click();
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='4곳');
        await page.goBack();
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='1곳');
        check(width+' history restores selected control',await page.getByLabel('가입 방식',{exact:true}).inputValue()==='invite_only');
        await page.goForward();
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='4곳');
        await page.getByLabel('종류',{exact:true}).selectOption('image');
        await page.getByLabel('계열',{exact:true}).selectOption('pixelfed');
        await page.getByLabel('소프트웨어',{exact:true}).selectOption('pixelfed');
        await page.getByLabel('태그',{exact:true}).selectOption('사진');
        await page.getByRole('button',{name:'조건 적용',exact:true}).click();
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='1곳');
        check(width+' compound kind/family/software/tag result',await page.locator('.server-card h2').innerText()==='빛 모으는 곳');
        check(width+' Korean tag survives URL',await page.evaluate(()=>new URL(location.href).searchParams.get('tag'))==='사진');
        await page.reload();await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        check(width+' SSR selected kind retained',await page.getByLabel('종류',{exact:true}).inputValue()==='image');
        await page.getByRole('link',{name:'초기화',exact:true}).click();
        await page.waitForFunction(()=>document.querySelector('.directory-result-count')?.textContent==='4곳');
        await page.getByLabel('정렬 기준',{exact:true}).selectOption('users');
        check(width+' numeric default descending',await page.getByLabel('정렬 방향',{exact:true}).inputValue()==='desc');
        await page.getByLabel('정렬 기준',{exact:true}).selectOption('domain');
        await page.getByRole('button',{name:'조건 적용',exact:true}).click();
        await page.waitForURL('**/servers?sort=domain');
        check(width+' domain sort',await page.locator('.server-domain').allTextContents().then(a=>a.join(',')===[...a].sort().join(',')));
        await page.screenshot({path:`output/playwright/server-search-${width}.png`,fullPage:true});
        await open('/servers?page=2');
        check(width+' empty page has previous link',await page.getByRole('navigation',{name:'서버 목록 페이지'}).getByRole('link',{name:'이전'}).isVisible());
        await page.getByRole('link',{name:'이전',exact:true}).click();
        await page.waitForFunction(()=>document.querySelectorAll('.server-card').length===4);
    }
    for(const query of ['page=0','reg=invalid','q=a&q=b','sort=untrusted','page_size=201']) {
        const r=await req.get(base+'/servers?'+query);
        check('invalid SSR '+query,r.status()===400 && (await r.text()).includes('검색 주소의 조건이 올바르지 않아요.'));
        check('invalid API '+query,(await req.get(base+'/api/public/server-search?filters='+encodeURIComponent(query))).status()===400);
    }
    const nojs=await page.context().browser().newContext({javaScriptEnabled:false});
    try {
        const p=await nojs.newPage();await p.goto(base+'/servers?reg=invite_only');
        check('filtered SSR without JavaScript',await p.locator('.server-card h2').innerText()==='냥냥.타워');
        check('SSR count without JavaScript',await p.locator('.directory-result-count').innerText()==='1곳');
    } finally {await nojs.close();}
    check('direct software API',(await(await req.get(base+'/api/public/software/pixelfed')).json()).software.name==='pixelfed');
    check('no runtime exceptions',errors.length===0);
    return {passed:checks.length,errors};
}
