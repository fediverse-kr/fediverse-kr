// member-browser-fixture.ps1 Seed -IncludeSite [-IncludeHealth], state-load,
// then run-code this file. All records/HTTP responses are real local PG data.
async page => {
    const req=page.context().request, base=await page.evaluate(()=>location.origin), checks=[],errors=[];
    const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
    page.on('pageerror',e=>errors.push(e.message));
    const state=await(await req.get(base+'/api/member/session')).json();
    if(state.member?.display_name!=='Browser test fixture')throw Error('Requires synthetic fixture');
    const domain='browser-'+state.member.id+'.example.org', url=base+'/servers/'+domain;
    const endpoint=base+'/api/public/server-health?domain='+encodeURIComponent(domain);
    const result=await req.get(endpoint);
    check('public response successful',result.status()===200);
    const data=await result.json();
    check('actual DB, not preview',data.preview===false);
    check('missing site is 404',(await req.get(base+'/api/public/server-health?domain=missing-health.example.org')).status()===404);
    check('invalid input is 400',(await req.get(base+'/api/public/server-health?domain=%0A')).status()===400);
    check('private diagnostic fields excluded',!JSON.stringify(data).match(/private health|job_id|site_id|status_code|"error"/));
    const ssr=await(await req.get(url)).text();
    check('history rendered by server',ssr.includes('server-health-summary'));
    await page.goto(url);
    await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    await page.locator('body').ariaSnapshot();
    const section=page.getByRole('region',{name:'최근 응답 기록',exact:true});
    if(!data.checks.length) {
        check('empty history is explicit',await section.getByText('아직 확인 기록이 없어요.',{exact:true}).isVisible());
        check('empty history has no fake bars',await section.locator('.server-health-tick').count()===0);
        check('empty history has no meaningless details',await section.locator('details').count()===0);
    } else {
        check('latest 48 cap',data.checks.length===48);
        check('oldest first, correct KST',data.checks[0].checked_at_kst==='2026-09-14 09:16:00' && data.checks[47].checked_at_kst==='2026-09-14 10:03:00');
        check('snapshot summary',await section.getByText('최근 48회 중 6회 응답이 없었어요.',{exact:true}).isVisible());
        for(const width of [1440,390,320]) {
            await page.setViewportSize({width,height:900});
            await section.scrollIntoViewIfNeeded();
            check('48 cells '+width,await section.locator('.server-health-tick').count()===48);
            check('normal/slow/failure colors '+width,await section.locator('.server-health-tick.responding').count()===36 && await section.locator('.server-health-tick.slow').count()===6 && await section.locator('.server-health-tick.unreachable').count()===6);
            check('no page overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            const summary=section.locator('summary');
            await summary.focus();
            await page.keyboard.press('Enter');
            await page.locator('body').ariaSnapshot();
            check('keyboard opens records '+width,await section.locator('details').getAttribute('open')!==null);
            const rows=section.locator('tbody tr');
            check('48 detailed rows '+width,await rows.count()===48);
            check('newest first, unknown latency not zero '+width,(await rows.first().innerText()).includes('2026-09-14 10:03:00') && (await rows.first().innerText()).includes('측정값 없음'));
            const table=section.getByRole('region',{name:'응답 확인 기록',exact:true});
            await table.focus();
            await page.keyboard.press('End');
            await page.waitForFunction(()=>document.querySelector('.server-health-table').scrollTop>0);
            check('table scrolls internally '+width,await table.evaluate(el=>el.scrollTop>0 && el.scrollHeight>el.clientHeight && el.clientHeight<=322));
            check('expanded table has no page overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            await summary.focus();
            await page.keyboard.press('Enter');
            await page.locator('body').ariaSnapshot();
            check('keyboard closes records '+width,await section.locator('details').getAttribute('open')===null);
            await section.screenshot({path:'output/playwright/health-history-'+width+'.png'});
        }
    }
    check('no hydration or page errors',errors.length===0);
    return {passed:checks.length,checks,pageErrors:errors};
}
