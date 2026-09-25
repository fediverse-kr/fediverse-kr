// Playwright CLI scenario, real PG/admin fixture and OpenDAL; no mocked replies.
// Run after check-catalog-moderation-browser with the same synthetic member.
async page => {
    const base=await page.evaluate(()=>location.origin), ctx=page.context(), req=ctx.request, checks=[], errors=[];
    if(!['http://127.0.0.1:12239','http://127.0.0.1:12241'].includes(base))throw Error('Local fixture origin only');
    const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
    const get=async path=>await(await req.get(base+path)).json();
    const post=(path,data,origin=base)=>req.post(base+path,{data,headers:{origin}});
    const state=await get('/api/member/session');
    if(state.member?.display_name!=='Browser test fixture'||!await get('/api/member/moderation/access'))throw Error('Synthetic catalog admin required');
    const name='browser-'+state.member.id, itemPath='/api/member/moderation/software/item?name='+name;
    const logoPath='/api/member/moderation/software/logo', publicPath='/api/public/software-logo/'+name;
    const open=async()=>{await page.goto(base+'/account/moderation/catalog/software/'+name);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    const xml=(tag,color)=>`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><title>synthetic-${tag}</title><rect width="64" height="64" rx="12" fill="${color}"/><path d="M16 32h32M32 16v32" stroke="white" stroke-width="6"/></svg>`;
    // Use actual file-input uploads from checked-in, synthetic SVG fixtures.
    const encoded=async text=>page.evaluate(text=>btoa(text),text);
    const write=async tag=>{
        await page.getByLabel('새 로고 선택',{exact:true}).setInputFiles('scripts/fixtures/logo-'+tag+'.svg');
        await page.getByText(/선택: logo-/).waitFor();
        await page.getByLabel('로고 관리 사유 · 관리자에게만 공개',{exact:true}).fill('비공개 로고 테스트 사유');
    };
    const save=async label=>{const result=page.waitForResponse(r=>r.url().endsWith(logoPath)&&r.request().method()==='POST');await page.getByRole('button',{name:label,exact:true}).click();return result;};
    const anonymous=await ctx.browser().newContext();
    page.on('pageerror',e=>errors.push(e.message));
    try {
        for(const[path,id]of[['/account/software/new','software-slug'],['/account/moderation/catalog/categories/new','category-name']]) {
            await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
            const input=page.locator('#'+id);
            await input.fill('invalid!');check('HTML rejects invalid identifier '+id,await input.evaluate(el=>el.validity.patternMismatch));
            await input.fill('valid-name_2');check('HTML accepts hyphen and underscore '+id,await input.evaluate(el=>el.checkValidity()));
        }
        await open();
        let current=await get(itemPath);
        const first=xml('first','#375fd0');
        const input={request:{name,revision:current.revision,note:'검사',data:await encoded(first)}};
        check('anonymous logo upload denied',(await anonymous.request.post(base+logoPath,{data:input,headers:{origin:base}})).status()===401);
        check('cross-origin logo upload denied',(await post(logoPath,input,'https://evil.example')).status()===403);
        // The standalone server can close an oversized upload while its client
        // is still sending; the development proxy instead delivers HTTP 413.
        // Accept only that specific transport refusal, and require live health.
        const oversized=await post(logoPath,{padding:'x'.repeat(700001)}).catch(e=>{
            if(!String(e.message).includes('ECONNRESET'))throw Error('Unexpected oversized-upload transport failure');
            return null;
        });
        check('upload body capped without stopping server',(!oversized||oversized.status()===413)&&(await req.get(base+'/healthz')).status()===200);
        check('fake raster rejected',(await post(logoPath,{request:{...input.request,data:await encoded('not an image')}})).status()===400);
        check('no logo before explicit save',!current.logo_available&&(await req.get(base+publicPath)).status()===404);
        await write('first');
        check('file selection alone is private',(await req.get(base+publicPath)).status()===404);
        check('UI uploads through real API',(await save('확인하고 로고 저장')).status()===200);
        await page.getByText('로고 설정을 저장했습니다.',{exact:true}).waitFor();
        let result=await req.get(base+publicPath);
        const original=await result.text();
        check('OpenDAL serves original SVG',result.status()===200&&original.trim()===first);
        check('SVG stays isolated',result.headers()['content-security-policy'].includes('sandbox')&&result.headers()['x-content-type-options']==='nosniff');
        check('real image rendered',await page.getByRole('region',{name:'제품 로고 관리'}).locator('img').evaluate(img=>img.complete&&img.naturalWidth===64));
        current=await get(itemPath);
        check('unchanged bytes do not add revision',(await post(logoPath,{request:{...input.request,revision:current.revision,data:await encoded(original)}})).status()===200&&(await get(itemPath)).revision===current.revision);
        const second=xml('second','#b84261');await write('second');
        check('parallel setting changes',(await post('/api/member/moderation/software/act',{request:{name,revision:current.revision,note:'다른 창',action:{kind:'display_order',value:current.display_order+1}}})).status()===200);
        check('stale upload rejected',(await save('확인하고 로고 저장')).status()===409);
        check('conflict keeps file and reason',await page.getByText(/선택: logo-second.svg/).isVisible()&&await page.getByLabel('로고 관리 사유 · 관리자에게만 공개',{exact:true}).inputValue()==='비공개 로고 테스트 사유');
        check('old logo stays visible',(await(await req.get(base+publicPath)).text())===original);
        await page.getByRole('button',{name:'최신 설정 확인',exact:true}).click();await page.getByRole('button',{name:'최신 설정을 확인하고 계속',exact:true}).click();
        check('confirmed replacement succeeds',(await save('확인하고 로고 저장')).status()===200);
        check('replacement bytes served',(await(await req.get(base+publicPath)).text()).trim()===second);
        const publicHistory=JSON.stringify(await get('/api/public/software-history?name='+name+'&page=0'));
        check('public history hides keys and private reason',!publicHistory.includes('software-logos/')&&!publicHistory.includes('비공개 로고'));
        for(const width of[1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            check('logo editor no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            await page.getByRole('region',{name:'제품 로고 관리'}).scrollIntoViewIfNeeded();
            await page.screenshot({path:`output/playwright/catalog-logo-${width}.png`,fullPage:true});
        }
        await page.getByLabel('기존 로고를 제거합니다',{exact:true}).check();
        await page.getByLabel('로고 관리 사유 · 관리자에게만 공개',{exact:true}).fill('검사 파일 제거');
        check('explicit UI removal succeeds',(await save('확인하고 로고 제거')).status()===200);
        await page.getByText('아직 등록된 로고가 없습니다.',{exact:true}).waitFor();
        check('removed logo inaccessible',(await req.get(base+publicPath)).status()===404&&!(await get(itemPath)).logo_available);
        check('no runtime exceptions',errors.length===0);
        return {passed:checks.length,checks,runtimeErrors:errors};
    } finally { await anonymous.close(); }
}
