// CLI run-code scenario. Requires member-browser-fixture.ps1 -IncludeSite.
// Actual local PG/HTTP writes; no DNS record/proof post is published. A manual
// worker request may resolve the random example.org fixture domain and fail.
async (page) => {
    const base='http://127.0.0.1:12239',ctx=page.context(),request=ctx.request;
    const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
    const check=(name,pass)=>{if(!pass)throw Error(name);checks.push(name);};
    const post=(path,data)=>request.post(base+path,{data,headers:{origin:base}});
    const anon=await ctx.browser().newContext();
    try {
        for(const [path,data] of [
            ['/api/member/sites/dns/begin',{domain:'example.org'}],
            ['/api/member/sites/dns/finish',{id:'none',value:'none'}],
            ['/api/member/sites/operator/verify',{account_id:'none',domain:'example.org'}],
            ['/api/member/sites/refresh',{domain:'example.org'}],
            ['/api/member/sites/resign',{domain:'example.org',revision:0,confirmation:'example.org'}],
            ['/api/member/sites/edit',{domain:'example.org',revision:0,changes:{name:'',description:'',rules:'',language:'',tags:'',owner_comment:'',invite_only:null,approval_required:null,hidden:false}}],
        ]) {
            check(path+' denies anonymous mutation',(await anon.request.post(base+path,{data,headers:{origin:base}})).status()===401);
            check(path+' rejects foreign Origin',(await request.post(base+path,{data,headers:{origin:'https://evil.example'}})).status()===403);
        }
        const noAuth=await anon.request.get(base+'/api/member/sites?page=0');check('owned list is private',noAuth.status()===401 && noAuth.headers()['cache-control'].includes('no-store'));
        await page.goto(base+'/account/sites');await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        const response=await request.get(base+'/api/member/sites?page=0');let item=(await response.json()).sites[0];
        check('isolated owner fixture loaded',item.edit.name==='Browser owner fixture' && item.closed && item.owner_method==='dns');
        const html=await request.get(base+'/account/sites');const markup=await html.text();const cookie=(await ctx.cookies()).find(x=>x.name==='fedkr_session');
        check('registration guidance matches implemented route',markup.includes('/account/sites/new')&&!markup.includes('아직 목록에 없는 서버 등록은 준비 중입니다.'));
        check('SSR is private and no session/proof secret serialized',html.headers()['cache-control'].includes('no-store') && !markup.includes(cookie.value) && !markup.includes('fk-verify='));
        check('operator account picker is rendered',await page.getByLabel('연동한 운영자 계정',{exact:true}).count()===1);
        await page.getByLabel('운영하는 서버 도메인',{exact:true}).fill('foreign.example.org');
        const rejected=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/operator/verify') && r.request().method()==='POST');
        await page.getByRole('button',{name:'운영자 확인',exact:true}).click();
        check('foreign-domain operator claim is rejected before network',(await rejected).status()===422);
        await page.getByText('연동된 계정과 서버의 운영자 정보가 일치하지 않아요.',{exact:true}).waitFor({state:'visible'});
        check('operator failure is visible',await page.getByText('연동된 계정과 서버의 운영자 정보가 일치하지 않아요.',{exact:true}).count()===1);
        await page.locator('.owner-editor > summary').click();
        const queued=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/refresh') && r.request().method()==='POST');
        await page.getByRole('button',{name:'정보 갱신 요청',exact:true}).click();
        check('manual refresh queues through actual owner API',(await queued).status()===200);
        const statusPath='/api/member/sites/refresh?domain='+encodeURIComponent(item.domain);
        const stateResponse=await request.get(base+statusPath),refreshState=await stateResponse.json();
        check('real queue state is private',stateResponse.headers()['cache-control'].includes('no-store') && ['pending','running','failed'].includes(refreshState.state));
        check('manual refresh has server-side hour limit',(await post('/api/member/sites/refresh',{domain:item.domain})).status()===429);
        check('anonymous refresh state is inaccessible',(await anon.request.get(base+statusPath)).status()===401);
        const statusRead=page.waitForResponse(r=>r.url().includes('/api/member/sites/refresh?') && r.request().method()==='GET');
        await page.getByRole('button',{name:'진행 상태 확인',exact:true}).click();check('status check reads persisted job',(await statusRead).status()===200);
        const field=(key)=>page.locator(`[id="owner-${item.domain}-${key}"]`);
        await field('name').fill('직접 고친 서버');await field('description').fill('운영자가 직접 저장한 소개');
        await field('rules').fill('서로 존중해 주세요.\n스팸은 금지해요.');
        await field('owner_comment').fill('<script>window.ownerInjected=true</script>');
        await field('language').fill('한국어');await field('tags').fill('사진, 개발');
        await page.getByLabel('초대가 필요한가요?',{exact:true}).selectOption('yes');
        await page.getByLabel('가입 승인이 필요한가요?',{exact:true}).selectOption('yes');
        let saved=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/edit') && r.request().method()==='POST');
        await page.getByRole('button',{name:'변경 저장',exact:true}).click();check('editor saves through real API',(await saved).status()===200);
        const current=async()=> (await (await request.get(base+'/api/member/sites?page=0')).json()).sites[0];item=await current();
        check('owner changes persisted',item.edit.name==='직접 고친 서버' && item.edit.approval_required===true && item.edit.invite_only===true);
        let publicData=await (await request.get(base+'/api/public/server/'+item.domain)).json();
        check('public projection carries only public guidance',publicData.site.guidance.rules.includes('서로 존중') && publicData.site.guidance.tags.length===2 && !JSON.stringify(publicData).includes('owner_method'));
        await page.goto(base+'/servers/'+item.domain);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        check('owner text renders escaped instead of executing',await page.getByText('<script>window.ownerInjected=true</script>',{exact:true}).count()===1 && !(await page.evaluate(()=>window.ownerInjected)));
        await page.goto(base+'/account/sites');await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');await page.locator('.owner-editor > summary').click();
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:width===1440?1000:844});
            check('owner form no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            check('hide checkbox keeps a readable horizontal label '+width,await page.locator('.owner-checkbox').evaluate(el=>el.getBoundingClientRect().height<64 && el.querySelector('input').getBoundingClientRect().width<=24));
            await page.screenshot({path:`output/playwright/site-owner-${width}.png`,fullPage:true});
        }
        check('owner form IDs remain unique',await page.evaluate(()=>{const ids=[...document.querySelectorAll('[id]')].map(x=>x.id);return ids.length===new Set(ids).size;}));
        await page.getByLabel('공개 목록에서 숨기기',{exact:true}).check();saved=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/edit') && r.request().method()==='POST');
        await page.getByRole('button',{name:'변경 저장',exact:true}).click();check('hiding saves without approval',(await saved).status()===200);
        check('hidden direct public URL returns no site',(await (await request.get(base+'/api/public/server/'+item.domain)).json()).site===null);
        check('hidden private owner row remains editable',(await current()).edit.hidden);
        // Stale revision must not overwrite the hiding decision.
        check('stale client receives conflict',(await post('/api/member/sites/edit',{domain:item.domain,revision:item.revision,changes:item.edit})).status()===409);
        item=await current();
        // Korean text exceeds the auth endpoints' 8 KiB limit but remains within
        // the explicitly scoped, bounded editorial request allowance.
        let changes={...item.edit,rules:'가'.repeat(2000),owner_comment:'나'.repeat(2000)};
        check('valid Korean editorial payload above 8 KiB saves',(await post('/api/member/sites/edit',{domain:item.domain,revision:item.revision,changes})).status()===200);
        check('editorial body above 32 KiB rejected',(await post('/api/member/sites/edit',{padding:'x'.repeat(32769)})).status()===413);
        check('password endpoint retains 8 KiB cap',(await post('/api/member/password/login',{login_id:'x',password:'x'.repeat(8193)})).status()===413);
        await page.reload();await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        await page.getByLabel('서버 도메인',{exact:true}).fill(item.domain);
        const issuedResponse=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/dns/begin') && r.request().method()==='POST');await page.getByRole('button',{name:'DNS 인증 시작',exact:true}).click();
        const issued=await issuedResponse;check('DNS challenge issued only after explicit POST',issued.status()===200);
        check('DNS record displayed without submission',await page.getByLabel('레코드 이름',{exact:true}).inputValue()==='_fediverse-kr.'+item.domain && (await page.getByLabel('TXT 값',{exact:true}).inputValue()).startsWith('fk-verify='));
        // No finish click: live DNS success is not claimed by this browser test.
        await page.getByRole('button',{name:'다시 시작',exact:true}).click();
        await page.locator('.owner-editor > summary').click();await page.getByText('운영자 관리 권한 내려놓기',{exact:true}).click();
        await page.getByLabel('확인을 위해 '+item.domain+' 입력',{exact:true}).fill(item.domain);
        const resigned=page.waitForResponse(r=>r.url().endsWith('/api/member/sites/resign') && r.request().method()==='POST');await page.getByRole('button',{name:'관리 권한 내려놓기',exact:true}).click();
        check('resigning removes owner access',(await resigned).status()===200 && (await (await request.get(base+'/api/member/sites?page=0')).json()).sites.length===0);
        check('resigned member cannot read or request refresh',(await request.get(base+statusPath)).status()===404 && (await post('/api/member/sites/refresh',{domain:item.domain})).status()===404);
        check('no runtime exceptions',errors.length===0);
        return {checks:checks.length,passed:checks,errors};
    } finally {await anon.close();}
}
