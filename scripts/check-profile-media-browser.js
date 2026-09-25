// Run with media-browser-fixture.ps1 Seed and an authenticated CLI context.
// Actual HTTP/PG failure preservation + rendering. No mocked successful AP call.
async page => {
    const context=page.context(), req=context.request, base=await page.evaluate(()=>location.origin);
    const checks=[],errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
    const state=await (await req.get(base+'/api/member/session')).json();
    if(state.member?.display_name!=='Browser test fixture :wave:' || state.linked_accounts.length!==1 || !state.linked_accounts[0].handle.endsWith('@browser-fixture.example')) throw Error('Expected only synthetic media fixture');
    const account=state.linked_accounts[0].id, route=base+'/api/member/profile-media/refresh';
    const avatar=await(await req.get(base+'/api/member/avatar')).text();
    const before=await(await req.get(base+'/api/member/profile-media')).json();
    check('legacy avatar and emoji present',before.avatar_available && before.emojis.includes('wave'));
    const anon=await context.browser().newContext();
    try {
        check('anonymous refresh rejected',(await anon.request.post(route,{headers:{origin:base},data:{account_id:account}})).status()===401);
        check('cross origin refresh rejected',(await req.post(route,{headers:{origin:'https://other.example.org'},data:{account_id:account}})).status()===403);
        check('invalid linked account rejected',(await req.post(route,{headers:{origin:base},data:{account_id:'00000000-0000-0000-0000-000000000000'}})).status()===400);
        await page.goto(base+'/account');
        await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        await page.locator('body').ariaSnapshot();
        const form=page.getByRole('form',{name:'프로필 사진과 이모지'});
        check('profile source defaults to linked account',await form.getByLabel('가져올 계정').inputValue()===account);
        const response=page.waitForResponse(r=>r.url()===route && r.request().method()==='POST');
        await form.getByRole('button',{name:'사진·이모지 가져오기'}).click();
        check('invalid remote domain fails through actual service',(await response).status()===502);
        await form.getByText('연합 계정을 불러오지 못했어요. 기존 사진은 유지합니다.',{exact:true}).waitFor();
        check('useful error displayed',true);
        check('image bytes retained on refresh failure',await(await req.get(base+'/api/member/avatar')).text()===avatar);
        const after=await(await req.get(base+'/api/member/profile-media')).json();
        check('failure persisted and pending cleared',after.refresh_failed && !after.refreshing && after.avatar_available);
        check('no storage paths exposed',!JSON.stringify(after).includes('avatars/') && !JSON.stringify(after).includes('sha256'));
        check('rate limited before another remote request',(await req.post(route,{headers:{origin:base},data:{account_id:account}})).status()===429);
        const current=await(await req.get(base+'/api/member/session')).json();
        check('manual name and private linking preserved',current.member.display_name===state.member.display_name && !current.linked_accounts[0].is_public);
        check('anonymous media remains private',(await anon.request.get(base+'/api/member/avatar')).status()===401);
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            await page.goto(base+'/account');
            await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
            await page.locator('body').ariaSnapshot();
            await page.waitForFunction(()=>document.querySelector('.profile-avatar img')?.naturalWidth===64 && document.querySelector('.profile-emoji img')?.naturalWidth===1);
            check('private avatar/emoji decode '+width,true);
            check('no horizontal overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            await page.getByRole('form',{name:'프로필 사진과 이모지'}).scrollIntoViewIfNeeded();
            check('persisted failure visible '+width,await page.getByText('마지막 갱신을 모두 마치지 못했어요.',{exact:false}).isVisible());
            await page.screenshot({path:'output/playwright/profile-refresh-'+width+'.png',fullPage:true});
        }
        check('no runtime exceptions',errors.length===0);
        return {passed:checks.length,checks,errors,scope:'Actual HTTP/PG and fixture images; no live ActivityPub success or real member backup'};
    } finally { await anon.close(); }
}
