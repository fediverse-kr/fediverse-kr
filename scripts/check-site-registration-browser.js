// Actual local PG/HTTP boundary checks plus explicitly mocked UI-only success.
// No real server is registered and no network/auth guard is bypassed in the app.
async (page) => {
    const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request;
    const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    const post=(path,data)=>req.post(base+path,{data,headers:{origin:base}});
    const prefix='/api/member/sites/registration';
    const anon=await ctx.browser().newContext();
    try {
        check('member eligibility real PG',(await req.get(base+prefix)).status()===200);
        check('anonymous read denied',(await anon.request.get(base+prefix)).status()===401);
        for(const action of ['preview','create']) {
            check('anonymous '+action,(await anon.request.post(base+prefix+'/'+action,{data:{domain:'example.org'},headers:{origin:base}})).status()===401);
            check('Origin '+action,(await req.post(base+prefix+'/'+action,{data:{domain:'example.org'},headers:{origin:'https://elsewhere.example.org'}})).status()===403);
            check('body '+action,(await post(prefix+'/'+action,{domain:'x'.repeat(9000)})).status()===413);
            check('internal target '+action,(await post(prefix+'/'+action,{domain:'http://127.0.0.1/'})).status()===400);
        }
        const owned=await (await req.get(base+'/api/member/sites?page=0')).json(),site=owned.sites[0];
        check('fixture owned site exists',!!site);
        for(const action of ['preview','create'])check('duplicate '+action,(await post(prefix+'/'+action,{domain:site.domain})).status()===409);
        const unchanged=await (await req.get(base+'/api/member/sites?page=0')).json();
        check('duplicate did not modify owner data',JSON.stringify(owned)===JSON.stringify(unchanged));
        const ssr=await req.get(base+'/account/sites/new'),html=await ssr.text();
        check('private SSR',ssr.headers()['cache-control'].includes('no-store'));
        check('private account not serialized',!html.includes('browser-fixture.example/users/'));
        await page.goto(base+'/account/sites/new');
        const input=page.getByRole('textbox',{name:'서버 주소'}),inspect=page.getByRole('button',{name:'서버 정보 확인',exact:true});
        await input.fill('http://127.0.0.1/');await inspect.click();
        await page.getByRole('alert').waitFor();
        check('invalid draft preserved',await input.inputValue()==='http://127.0.0.1/');
        await input.fill(site.domain);await inspect.click();
        await page.getByText('이미 등록되어 있거나 새로 등록할 수 없는 주소예요. 기존 정보는 변경하지 않았습니다.',{exact:true}).waitFor();
        check('real duplicate message visible',true);
        // UI-only remote data. This is not counted as actual HTTP/PG registration.
        const sample={domain:'ui-registration.example.org',name:'가상의 사진 서버',description:'긴 소개 '.repeat(600)+'<script>window.registrationInjected=true</script>',software:'photo-demo',users:0,registration_open:false};
        let releasePreview,confirmCalls=0;
        await page.route(base+prefix+'/preview',async route=>{
            await new Promise(resolve=>releasePreview=resolve);
            await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(sample)});
        });
        await page.route(base+prefix+'/create',async route=>{
            confirmCalls++;
            check('confirmation sends domain only',JSON.stringify(route.request().postDataJSON())===JSON.stringify({domain:sample.domain}));
            await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(sample.domain)});
        });
        await input.fill('https://ui-registration.example.org/');
        await inspect.click();
        await page.getByRole('button',{name:'서버 확인 중…',exact:true}).waitFor();
        check('pending locks input and submit',await input.isDisabled() && await page.getByRole('button',{name:'서버 확인 중…'}).isDisabled());
        while(!releasePreview)await page.waitForTimeout(20);
        releasePreview();
        await page.getByRole('heading',{name:sample.name}).waitFor();
        check('zero and closed are not unknown',(await page.locator('.registration-facts').innerText()).includes('0') && (await page.locator('.registration-facts').innerText()).includes('닫힘'));
        check('remote markup not executed',await page.evaluate(()=>window.registrationInjected!==true));
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            check('no horizontal overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth));
            check('bounded long description '+width,await page.locator('.registration-description').evaluate(e=>e.scrollHeight>e.clientHeight && e.clientHeight<=220));
            await page.screenshot({path:`output/playwright/registration-${width}.png`,fullPage:true});
        }
        await input.fill('changed.example.org');
        check('editing invalidates old preview',await page.getByRole('heading',{name:sample.name}).count()===0);
        releasePreview=undefined;await input.fill(sample.domain);await inspect.click();
        while(!releasePreview)await page.waitForTimeout(20);releasePreview();
        await page.getByRole('button',{name:'이 서버 등록',exact:true}).click();
        await page.getByRole('heading',{name:'목록에 등록했어요.'}).waitFor();
        check('UI completion has next links',await page.getByRole('link',{name:'서버 정보 보기'}).getAttribute('href')==='/servers/'+sample.domain && await page.getByRole('link',{name:'운영자 인증',exact:true}).count()===1);
        check('single confirmation',confirmCalls===1);
        await page.unroute(base+prefix+'/preview');await page.unroute(base+prefix+'/create');
        check('simulation created no server',(await req.get(base+'/servers/'+sample.domain)).status()===404);
        check('runtime errors',errors.length===0);
        return {passed:checks.length,checks,runtimeErrors:errors,scope:'Real API rejection/private SSR + simulated preview/confirmation UI, not live remote registration'};
    } finally {await anon.close();}
}
