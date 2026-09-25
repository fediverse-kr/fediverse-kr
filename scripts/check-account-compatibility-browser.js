// Run with an authenticated synthetic member whose linked account has no date.
// Recheck reaches real HTTP/PG; .example actor is rejected before remote access.
async (page) => {
    const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request;
    const endpoint='/api/member/sites/registration/recheck',checks=[];
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    const anon=await ctx.browser().newContext();
    try {
        check('anonymous retry rejected',(await anon.request.post(base+endpoint,{data:{},headers:{origin:base}})).status()===401);
        check('foreign Origin rejected',(await req.post(base+endpoint,{data:{},headers:{origin:'https://elsewhere.example.org'}})).status()===403);
        const before=await (await req.get(base+'/api/member/session')).json();
        check('missing age remains ineligible',(await req.get(base+'/api/member/sites/registration')).status()===403);
        await page.goto(base+'/account/sites/new');
        const button=page.getByRole('button',{name:'다시 확인',exact:true});
        await button.waitFor();
        let release,posts=0;
        await page.route(base+endpoint,async route=>{
            posts++;
            check('retry uses POST',route.request().method()==='POST');
            await new Promise(resolve=>release=resolve);
            await route.continue();
        });
        const response=page.waitForResponse(r=>new URL(r.url()).pathname===endpoint && r.request().method()==='POST');
        await button.click();
        const busy=page.getByRole('button',{name:'계정 생성일 확인 중…',exact:true});
        await busy.waitFor();
        check('pending retry disabled',await busy.isDisabled());
        while(!release)await page.waitForTimeout(20);
        release();
        const retried=await response;
        if(retried.status()!==200)throw Error('retry HTTP '+retried.status()+'; content-type='+retried.request().headers()['content-type']+'; origin-present='+Boolean(retried.request().headers().origin)+'; body='+(await retried.text()).slice(0,500));
        check('bounded retry completes',retried.status()===200);
        check('unknown date never relaxes eligibility',(await req.get(base+'/api/member/sites/registration')).status()===403);
        await button.waitFor();
        check('single bounded retry',posts===1);
        await page.unroute(base+endpoint);
        const after=await (await req.get(base+'/api/member/session')).json();
        check('retry preserves linked identity',JSON.stringify(before)===JSON.stringify(after));
        for(let i=0;i<3;i++)await req.post(base+endpoint,{data:{},headers:{origin:base}});
        check('persistent retry rate limit',(await req.post(base+endpoint,{data:{},headers:{origin:base}})).status()===429);
        await page.goto(base+'/account');
        await page.getByText('디렉터리 표시 안 함',{exact:true}).waitFor();
        const enable=page.getByRole('button',{name:'프로필을 디렉터리에 표시 허용',exact:true});
        const help=await enable.getAttribute('aria-describedby');
        check('consent help is associated',!!help && await page.locator('[id="'+help+'"]').count()===1);
        const text=await page.locator('[id="'+help+'"]').innerText();
        check('consent scope and current non-publication explained',text.includes('fediverse.kr의 사람 찾기 디렉터리') && text.includes('원래 SNS의 공개 범위') && text.includes('현재는 동의만 저장'));
        for(const width of [1440,390,320]){
            await page.setViewportSize({width,height:width===1440?1000:1100});
            check('actual viewport '+width,await page.evaluate(()=>innerWidth)===width);
            check('no root overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
            await page.screenshot({path:'account-compatibility-'+width+'.png',fullPage:true});
        }
        await enable.click();
        await page.getByText('디렉터리 표시 허용됨',{exact:true}).waitFor();
        await page.reload();
        const revoke=page.getByRole('button',{name:'디렉터리 표시 허용 취소',exact:true});
        await revoke.waitFor();
        check('consent persists across reload',true);
        await revoke.click();
        await page.getByText('디렉터리 표시 안 함',{exact:true}).waitFor();
        await page.reload();await enable.waitFor();
        check('consent revocation persists',true);
        check('GitHub contribution link',await page.locator('footer a[href="https://github.com/fediverse-kr/fediverse-kr"]').count()===1);
        return {passed:checks.length,checks,scope:'Real synthetic HTTP/PG, hydrated retry and consent mutation/reload; no remote challenge or production account'};
    } finally {await anon.close();}
}
