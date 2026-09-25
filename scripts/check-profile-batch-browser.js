// media-browser-fixture.ps1 -IncludeProfileBatch; actual HTTP/PG/worker, no API mocks.
async page => {
    const context=page.context(), req=context.request, base=await page.evaluate(()=>location.origin);
    const endpoint=base+'/api/member/moderation/profile-batch', checks=[], errors=[];
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    page.on('pageerror',e=>errors.push(e.message));
    const member=await(await req.get(base+'/api/member/session')).json();
    if(member.member?.display_name!=='Browser test fixture :wave:') throw Error('Synthetic profile batch fixture required');
    const avatar=await(await req.get(base+'/api/member/avatar')).text();
    const anon=await context.browser().newContext();
    try {
        check('anonymous progress private',(await anon.request.get(endpoint)).status()===401);
        check('anonymous start rejected',(await anon.request.post(endpoint+'/start',{headers:{origin:base},data:{confirmed:true}})).status()===401);
        check('cross origin start rejected',(await req.post(endpoint+'/start',{headers:{origin:'https://other.example.org'},data:{confirmed:true}})).status()===403);
        check('confirmation required',(await req.post(endpoint+'/start',{headers:{origin:base},data:{confirmed:false}})).status()===400);
        const initial=await req.get(endpoint);
        check('private no-store response',(initial.headers()['cache-control']||'').includes('no-store'));
        check('fresh fixture has no prior batch',await initial.json()===null);
        await page.goto(base+'/account');
        await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        await page.locator('body').ariaSnapshot();
        await page.getByRole('link',{name:'백오피스 열기',exact:true}).click();
        await page.getByRole('heading',{name:'운영 개요',exact:true}).waitFor();
        await page.locator('.backoffice-nav-rail').getByRole('link',{name:/^작업·유지보수/}).click();
        await page.getByRole('link',{name:'프로필 유지보수',exact:true}).click();
        await page.waitForURL(base+'/account/moderation/profiles');
        await page.locator('body').ariaSnapshot();
        const area=page.getByRole('region',{name:'전체 프로필 갱신'});
        check('maintenance scope and unchanged identity explained',await area.getByText('회원 사진과 이모지만 갱신합니다. 이름과 공개 범위는 바꾸지 않습니다.',{exact:true}).isVisible());
        check('batch persistence and source skipping explained',await area.getByText('기존에 선택한 출처를 사용합니다. 출처가 없거나 최근 갱신한 회원은 건너뛰며, 창을 닫아도 작업은 계속됩니다.',{exact:true}).isVisible());
        check('unchecked button disabled',await area.getByRole('button',{name:'전체 갱신 시작'}).isDisabled());
        await area.getByRole('checkbox').check();
        await area.getByRole('button',{name:'전체 갱신 시작'}).click();
        await area.getByRole('progressbar',{name:'프로필 갱신 진행'}).waitFor();
        check('real running progress displayed',true);
        const active=await(await req.get(endpoint)).json();
        check('fixed real target snapshot',active.total===31);
        const same=await(await req.post(endpoint+'/start',{headers:{origin:base},data:{confirmed:true}})).json();
        check('repeated start reuses active batch',same.id===active.id);
        await area.getByRole('status').filter({hasText:'완료 ·'}).waitFor({timeout:25000});
        const completed=await(await req.get(endpoint)).json();
        check('same-process worker finishes real batch',completed.state==='complete' && completed.failed===1 && completed.skipped===30);
        check('failed remote fetch preserves avatar',await(await req.get(base+'/api/member/avatar')).text()===avatar);
        const after=await(await req.get(base+'/api/member/session')).json();
        check('name and private identity unchanged',after.member.display_name===member.member.display_name && !after.linked_accounts[0].is_public);
        check('batch exposes counts not target identities',!JSON.stringify(completed).includes(member.member.id) && !JSON.stringify(completed).includes('@'));
        await page.locator('body').ariaSnapshot();
        await area.getByRole('checkbox').check();
        await area.getByRole('button',{name:'전체 갱신 시작'}).click();
        await area.getByRole('button',{name:'남은 갱신 중단'}).waitFor();
        await page.locator('body').ariaSnapshot();
        await area.getByRole('button',{name:'남은 갱신 중단'}).click();
        await area.getByRole('status').filter({hasText:'중단됨 ·'}).waitFor();
        const cancelled=await(await req.get(endpoint)).json();
        check('real cancellation stops pending targets',cancelled.state==='cancelled' && cancelled.cancelled>0 && cancelled.running===0 && cancelled.pending===0);
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            await page.reload();
            await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
            await page.locator('body').ariaSnapshot();
            check('persisted cancellation '+width,await area.getByRole('status').filter({hasText:'중단됨 ·'}).isVisible());
            check('no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            const box=await area.getByRole('checkbox').boundingBox();
            check('shared compact checkbox '+width,box && box.width===20 && box.height===20);
            await page.screenshot({path:'output/playwright/profile-batch-'+width+'.png',fullPage:true});
        }
        check('no browser exceptions',errors.length===0);
        return {passed:checks.length,checks,errors,scope:'Real PG queue and worker; reserved remote domain fails. Not successful live AP or production data.'};
    } finally { await anon.close(); }
}
