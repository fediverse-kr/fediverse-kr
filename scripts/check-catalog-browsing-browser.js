// playwright-cli run-code after catalog-browser-fixture.ps1 -Mode Seed.
// Local actual PG and anonymous HTTP, no request interception or remote IO.
async (page) => {
    const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    const location=async()=>page.evaluate(()=>({pathname:window.location.pathname,params:Object.fromEntries(new URLSearchParams(window.location.search))}));
    const search=async filters=>await(await req.get(base+'/api/public/software-search?filters='+encodeURIComponent(filters))).json();
    const seed=await search('q=browse-');
    const sample=seed.software.find(s=>/^browse-[a-f0-9]{32}-tool-/.test(s.name));
    if(!sample || seed.preview)throw Error('Isolated synthetic catalog required');
    const prefix=sample.name.replace(/-tool-\d+$/,''),q='q='+prefix;
    const first=await search(q),second=await search(q+'&page=2');
    check('actual whole-database pagination',first.total===27 && first.software.length===24 && second.software.length===3 && !second.has_next);
    check('stable pages have no duplicates',new Set([...first.software,...second.software].map(s=>s.name)).size===27);
    check('literal Korean percent underscore plus search',(await search('q='+encodeURIComponent('사진%_+'))).total===1);
    const filtered=await search(q+'&cat='+prefix+'-kind-14');
    check('off-page category filters all software',filtered.total===13 && filtered.labels.some(k=>k.label==='검토 종류 14'));
    for(const bad of ['page=0','page=10002','kind_page=-1','q=a&q=b','q=%00']) {
        check('bad API criteria are 400 '+bad,(await req.get(base+'/api/public/software-search?filters='+encodeURIComponent(bad))).status()===400);
        check('bad SSR criteria are 400 '+bad,(await req.get(base+'/platforms?'+bad)).status()===400);
    }
    const open=async path=>{await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    const count=async n=>{await page.waitForFunction(n=>document.querySelectorAll('.software-card').length===n,n);};
    for(const [width,height] of [[1440,1000],[390,844],[320,667]]) {
        await page.setViewportSize({width,height});
        await open('/platforms?'+q);await count(24);
        const epoch=await page.evaluate(()=>performance.timeOrigin);
        await page.getByRole('navigation',{name:'소프트웨어 목록 페이지',exact:true}).getByRole('link',{name:'다음',exact:true}).click();
        await page.waitForURL('**/*page=2');await count(3);
        check(width+' pagination uses SPA and keeps search',(await page.evaluate(()=>performance.timeOrigin))===epoch && (await location()).params.q===prefix);
        await page.goBack();await count(24);
        check(width+' back restores criteria',await page.getByRole('searchbox',{name:'소프트웨어 이름 검색'}).inputValue()===prefix);
        await page.getByRole('navigation',{name:'종류 목록 페이지',exact:true}).getByRole('link',{name:'다음',exact:true}).click();
        await page.getByRole('link',{name:'◇ 검토 종류 14',exact:true}).waitFor();
        await page.getByRole('link',{name:'◇ 검토 종류 14',exact:true}).click();await count(13);
        check(width+' selected category is visible',await page.getByRole('link',{name:'◇ 검토 종류 14',exact:true}).getAttribute('aria-current')==='true');
        await page.reload();await count(13);
        check(width+' reload retains both category and query',(await location()).params.cat===prefix+'-kind-14');
        await page.getByRole('link',{name:'✳ 모든 종류',exact:true}).click();await count(24);
        await page.getByRole('searchbox',{name:'소프트웨어 이름 검색'}).fill('사진%_+');
        await page.getByRole('button',{name:'검색',exact:true}).click();await count(1);
        check(width+' Korean query remains exact',(await location()).params.q==='사진%_+');
        check(width+' layout fits viewport',!await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1));
        check(width+' main menu stays visible',await page.getByRole('navigation',{name:'주요 메뉴'}).getByRole('link',{name:'소프트웨어',exact:true}).isVisible());
        await page.screenshot({path:`output/playwright/catalog-browse-${width}.png`,fullPage:true});
        await page.locator('.software-card').click();
        await page.getByRole('heading',{name:'검토 사진%_+ 도구',exact:true}).waitFor();
        check(width+' card opens independent detail',(await location()).pathname==='/software/'+prefix+'-tool-27');
        await open('/platforms?'+q+'&page=9999');
        await page.getByRole('link',{name:'첫 페이지로',exact:true}).click();await count(24);
        check(width+' empty page has a working recovery link',true);
    }
    const noJs=await ctx.browser().newContext({javaScriptEnabled:false});
    try {
        const p=await noJs.newPage();await p.goto(base+'/platforms?'+q);
        check('SSR already includes cards and real pagination links',await p.locator('.software-card').count()===24);
        await p.getByRole('navigation',{name:'소프트웨어 목록 페이지',exact:true}).getByRole('link',{name:'다음',exact:true}).click();
        check('pagination works without JavaScript',await p.locator('.software-card').count()===3);
    } finally {await noJs.close();}
    check('no browser runtime exceptions',errors.length===0);
    return {passed:checks.length,errors};
}
