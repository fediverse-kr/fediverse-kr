// playwright-cli --session fedkr-review run-code --filename scripts/check-content-browser.js
// Local unconfigured preview only. No real account or remote federation requests.
async (page) => {
    const base = await page.evaluate(() => location.origin);
    if (!/^http:\/\/127\.0\.0\.1:(12239|12241)$/.test(base)) throw new Error('Local preview origin required');
    const checks = [];
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const check = (name, passed) => { if (!passed) throw new Error(name); checks.push(name); };
    const open = async path => {
        await page.goto(base + path);
        await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
    };
    const overflow = () => page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1);
    const navLabels = ['연합우주 이해하기', '소프트웨어', '서버 찾기', '사람 찾기', '운영', '개발', '내 계정'];
    const req = page.context().request;
    check('unconfigured preview has explicit data source', (await (await req.get(base+'/api/public/catalog')).json()).preview === true);
    const previewComments=await req.get(base+'/api/public/comments?domain=light.example&page=0');
    check('reserved preview domain has explicit example comments',previewComments.status()===200 && (await previewComments.json()).preview===true);
    const previewAccess=await req.get(base+'/api/member/comment-access?domain=light.example&ids=');
    check('preview comment controls do not claim real account access',previewAccess.status()===200 && (await previewAccess.json()).available===false);
    check('unknown preview domain is not an invented comment thread',(await req.get(base+'/api/public/comments?domain=missing.example&page=0')).status()===404);
    const previewReplies=await req.get(base+'/api/public/comment-replies?domain=light.example&parent=eeeeeeee-0000-4000-8000-000000000001&page=0');
    check('reserved preview domain retains its example reply',previewReplies.status()===200 && (await previewReplies.json()).replies.length===1);
    for (const [old, next] of [['/software','/platforms'],['/my','/account'],['/recommend','/servers'],['/auth/verify?code=synthetic','/login']]) {
        const response = await req.get(base+old,{maxRedirects:0});
        check('Phoenix URL '+old, response.status()===308 && response.headers().location===next);
    }
    check('liveness is 200', (await req.get(base+'/healthz')).status()===200);
    check('preview is not database-ready', (await req.get(base+'/readyz')).status()===503);
    for (const [width,height] of [[1440,1000],[390,844],[320,667]]) {
        await page.setViewportSize({width,height});
        for (const path of ['/platforms','/software/pixelfed','/servers','/servers/light.example','/people','/operate','/develop','/apps','/about','/guides/self-hosting']) {
            await open(path);
            check(`${width} ${path} no horizontal overflow`, !await overflow());
            check(`${width} ${path} one main heading`, await page.locator('main h1').count()===1);
            if (width<=1100) {
                const toggle=page.locator('.site-nav').getByRole('button',{name:'주요 메뉴 열기',exact:true});
                check(`${width} ${path} menu toggle`,await toggle.isVisible() && await toggle.getAttribute('aria-expanded')==='false');
                await toggle.click();
                const drawer=page.getByRole('dialog',{name:'주요 메뉴',exact:true});
                await drawer.waitFor({state:'visible'});
                for (const name of navLabels) check(`${width} ${path} drawer menu ${name}`,await drawer.getByRole('link',{name,exact:true}).isVisible());
                await drawer.getByRole('button',{name:'주요 메뉴 닫기',exact:true}).click();
                await drawer.waitFor({state:'hidden'});
                await page.waitForFunction(()=>document.activeElement?.id==='mobile-menu-toggle');
            } else {
                const desktopNavigation=page.locator('.site-nav');
                for (const name of navLabels) check(`${width} ${path} visible menu ${name}`,await desktopNavigation.getByRole('link',{name,exact:true}).isVisible());
            }
            if(path==='/servers/light.example') check(`${width} example comments are visible without an error`,await page.locator('.community-thread').count()===1 && await page.locator('.community-section [role="alert"]').count()===0);
        }
        await open('/platforms');
        await page.getByRole('link',{name:'▧ 사진',exact:true}).click();
        await page.getByRole('status').filter({hasText:'소프트웨어 1개'}).waitFor();
        check(`${width} kind filter`, await page.locator('.software-card').count()===1);
        await page.locator('a[href="/software/pixelfed"]').click();
        await page.getByRole('link',{name:'이 소프트웨어의 서버 찾기'}).click();
        await page.waitForURL('**/software/pixelfed/servers');
        await page.getByRole('heading',{name:'빛 모으는 곳',exact:true}).waitFor();
        check(`${width} software-to-sites`, await page.locator('.server-card').count()===1);
        check(`${width} software filter matches the displayed result`, await page.getByLabel('소프트웨어',{exact:true}).inputValue()==='pixelfed');
        for(const name of ['종류','계열','태그']) check(`${width} unset filter ${name} stays all`,await page.getByLabel(name,{exact:true}).inputValue()==='');
        await page.getByRole('searchbox',{name:'서버 이름, 주소, 소프트웨어 검색'}).fill('no-such-synthetic-site');
        const beforeSearch = await page.evaluate(() => performance.timeOrigin);
        await page.getByRole('button',{name:'검색',exact:true}).click();
        await page.getByRole('heading',{name:'조건에 맞는 서버가 없어요.'}).waitFor();
        check(`${width} search empty state`, await page.locator('.server-card').count()===0);
        check(`${width} search does not reload the document`, (await page.evaluate(() => performance.timeOrigin))===beforeSearch);
        await open('/people');
        await page.getByRole('button',{name:/목록에서 발견되고 싶어요/}).click();
        check(`${width} listing opt-in preview`, await page.locator('.identity-preview').getAttribute('data-mode')==='2');
        await page.getByRole('button',{name:/인증만 하고 싶어요/}).click();
        check(`${width} private preview has no public handles`, !(await page.locator('.identity-preview').innerText()).includes('@me'));
        for (const topic of ['','delivery','visibility','boundaries','find','moving']) {
            await open('/start'+(topic?'/'+topic:''));
            await page.waitForFunction(()=>!!window.__fedkrExplainerScroll);
            const steps = topic ? 3 : 5;
            for(let step=0;step<steps;step++) {
                await page.getByRole('group',{name:'장면 바로 보기'}).getByRole('button').nth(step).click();
                await page.waitForFunction(i=>document.querySelector('#ex-scrolly')?.dataset.step===String(i),step);
                // Allow the CSS resize/fade to settle before geometry checks.
                await page.waitForTimeout(950);
                check(`${width} ${topic||'basics'} ${step} scroll state`, await page.getByRole('group',{name:'장면 바로 보기'}).getByRole('button').nth(step).getAttribute('aria-pressed')==='true');
                check(`${width} ${topic||'basics'} ${step} no overflow`, !await overflow());
                if(width<761) {
                    check(`${width} ${topic||'basics'} ${step} caption on screen`, await page.locator('.ex-mobile-caption').evaluate(el=>{const r=el.getBoundingClientRect();return r.top>=0 && r.bottom<=innerHeight+1;}));
                    if(topic==='find') check(`${width} find ${step} only active app`, await page.locator('.ex-find-scene .ex-perspective:visible').count()===1);
                }
                if(!topic&&step===3) check(`${width} basics soup avatar loads`,await page.locator('img.social-avatar[src*="soup-"]').evaluateAll(images=>images.some(image=>image.complete&&image.naturalWidth>0)));
                if(topic==='delivery'&&step===1) check(`${width} delivery film avatar loads`,await page.locator('img.social-avatar[src*="film-"]').evaluateAll(images=>images.some(image=>image.complete&&image.naturalWidth>0)));
            }
            if(topic==='find'||topic==='moving') await page.screenshot({path:`output/playwright/${topic}-${width}.png`});
        }
    }
    const noJs=await page.context().browser().newContext({javaScriptEnabled:false});
    try {
        const noJsPage=await noJs.newPage();
        for(const path of ['/start','/start/moving','/platforms','/servers','/people','/operate','/develop']) {
            await noJsPage.goto(base+path);
            check('SSR text '+path, await noJsPage.locator('main h1').count()===1 && (await noJsPage.locator('main').innerText()).length>100);
        }
    } finally { await noJs.close(); }
    await page.setViewportSize({width:1440,height:1000});
    await page.emulateMedia({reducedMotion:'reduce'});
    await open('/');
    await page.waitForFunction(()=>Number(document.querySelector('.landing-demo')?.dataset.tick||0)>=12);
    const reducedStartTick=Number(await page.locator('.landing-demo').getAttribute('data-tick'));
    await page.waitForTimeout(350);
    const reducedEndTick=Number(await page.locator('.landing-demo').getAttribute('data-tick'));
    check('reduced-motion keeps landing autoplay',reducedEndTick>reducedStartTick);
    check('reduced-motion keeps a CSS animation running',await page.evaluate(()=>document.getAnimations().some(animation=>animation.playState==='running')));
    const catTitle=page.locator('.social-site-title',{hasText:'냥냥.타워'});
    await catTitle.waitFor();
    check('cat site mark is the dedicated cat-tower SVG',await catTitle.locator('.social-brand-mark svg[data-icon-pack="cat-tower"]').count()===1);
    await page.waitForFunction(()=>{
        const avatars=[...document.querySelectorAll('.landing-demo img.social-avatar')];
        return avatars.length>=3&&avatars.every(image=>image.complete&&image.naturalWidth>0);
    });
    const avatars=await page.evaluate(()=>{const images=[...document.querySelectorAll('.landing-demo img.social-avatar')];return {sources:new Set(images.map(image=>image.currentSrc||image.src)).size,loaded:images.every(image=>image.complete&&image.naturalWidth>0),legacy:document.querySelectorAll('.landing-demo .social-character').length};});
    check('visible social people use loaded image avatars',avatars.sources>=3&&avatars.loaded&&avatars.legacy===0);
    await open('/start/find');
    await page.getByRole('group',{name:'장면 바로 보기'}).getByRole('button').nth(2).click();
    await page.waitForFunction(()=>document.querySelector('#ex-scrolly')?.dataset.step==='2');
    check('reduced-motion scrolling remains usable', await page.locator('.ex-find-scene .social-post[data-highlighted=true]').count()===2);
    await page.getByRole('navigation',{name:'주요 메뉴'}).getByRole('link',{name:'소프트웨어',exact:true}).click();
    await page.waitForFunction(()=>!window.__fedkrExplainerScroll);
    check('scroll observer disposed on navigation', true);
    await page.emulateMedia({reducedMotion:'no-preference'});
    check('no browser runtime exceptions', errors.length===0);
    return {passed:checks.length, errors};
}
