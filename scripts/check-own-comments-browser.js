// Actual local PG/HTTP; Seed -IncludeSite -IncludeCommunity -IncludeOwnComments.
// With no IncludeCommunity, the same script checks an empty private history.
async page => {
    const ctx=page.context(),req=ctx.request,base=await page.evaluate(()=>location.origin),checks=[],errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
    const state=await(await req.get(base+'/api/member/session')).json();
    if(state.member?.display_name!=='Browser test fixture')throw Error('Requires only synthetic fixture');
    const endpoint=base+'/api/member/own-comments?page=0',url=base+'/account/comments';
    const anon=await ctx.browser().newContext();
    try {
        check('anonymous API denied',(await anon.request.get(endpoint)).status()===401);
        const loggedOut=await anon.request.get(url),text=await loggedOut.text();
        check('anonymous SSR shows login without comments',text.includes('먼저 로그인해 주세요.')&&!text.includes('Own fixture comment')&&!text.includes('Own history'));
        check('private SSR cannot be cached',loggedOut.headers()['cache-control'].includes('private')&&loggedOut.headers()['cache-control'].includes('no-store')&&loggedOut.headers()['vary'].toLowerCase().includes('cookie'));
        const response=await req.get(endpoint),data=await response.json();
        check('private API cannot be cached',response.status()===200&&response.headers()['cache-control'].includes('no-store'));
        check('invalid page rejected',(await req.get(base+'/api/member/own-comments?page=10001')).status()===400);
        check('no other identity or deleted contents',!JSON.stringify(data).match(/Deleted own fixture secret|Community fixture question|Fixture reply|actor_url|login_id|linked_accounts/));
        await page.goto(base+'/account');
        await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        await page.locator('body').ariaSnapshot();
        await page.getByRole('navigation',{name:'내 활동'}).getByRole('link',{name:'내 댓글',exact:true}).click();
        await page.waitForURL(url);
        await page.getByRole('region',{name:'내가 남긴 댓글'}).waitFor();
        await page.locator('body').ariaSnapshot();
        check('account entry reaches own comments',true);
        if(data.comments.length===0) {
            check('empty list explained',await page.getByText('아직 작성한 댓글이 없어요.',{exact:true}).isVisible());
            check('empty list has no next page',await page.getByRole('navigation',{name:'내 댓글 목록 페이지'}).getByRole('button',{name:'다음',exact:true}).isDisabled());
        } else {
            check('20 row page',data.comments.length===20&&data.has_next);
            const second=await(await req.get(base+'/api/member/own-comments?page=1')).json();
            check('remaining page contains 5 unique comments',second.comments.length===5&&!second.has_next&&second.comments.every(c=>!data.comments.some(a=>a.id===c.id)));
            const ssr=await(await req.get(url)).text();
            check('SSR includes own history',ssr.includes('Own fixture comment'));
            check('script text not executed',await page.evaluate(()=>window.ownCommentExecuted!==true));
            if(data.comments[0].server_available) {
                const firstLink=page.locator('.own-comment-list > li').first().getByRole('link');
                const target=await firstLink.getAttribute('href');
                await firstLink.click(); await page.waitForURL(base+target);
                await page.getByRole('region',{name:'서버 이용자 이야기'}).waitFor();
                check('server link opens shared discussion',true);
                await page.goto(url);
                await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
                await page.locator('body').ariaSnapshot();
            } else {
                check('hidden server has no public link',await page.locator('.own-comment-list a').count()===0);
                check('hidden history remains readable only to its author',(await page.locator('.own-comment-list').innerText()).includes('내 댓글만 확인할 수 있어요.'));
            }
            for(const width of [1440,390,320]) {
                await page.setViewportSize({width,height:900});
                check('no page overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
                check('20 rows rendered '+width,await page.locator('.own-comment-list > li').count()===20);
                const pager=page.getByRole('navigation',{name:'내 댓글 목록 페이지'});
                await pager.getByRole('button',{name:'다음',exact:true}).click();
                await page.getByText('2페이지',{exact:true}).waitFor();
                await page.locator('body').ariaSnapshot();
                check('next page renders '+width,await page.locator('.own-comment-list > li').count()===5);
                check('last page disables next '+width,await pager.getByRole('button',{name:'다음',exact:true}).isDisabled());
                await pager.getByRole('button',{name:'이전',exact:true}).click();
                await page.getByText('1페이지',{exact:true}).waitFor();
                await page.locator('body').ariaSnapshot();
                await page.evaluate(()=>scrollTo(0,0));
                await page.screenshot({path:'output/playwright/own-comments-'+width+'.png'});
            }
        }
        check('logout succeeds',(await req.post(base+'/api/member/logout',{headers:{origin:base},data:{}})).status()===200);
        await page.getByRole('button',{name:'목록 새로고침',exact:true}).click();
        await page.getByRole('alert').waitFor();
        check('revoked session clears personal list',await page.locator('.own-comment-list > li').count()===0);
        check('revoked API denied',(await req.get(endpoint)).status()===401);
        check('no hydration exceptions',errors.length===0);
        return {passed:checks.length,checks,pageErrors:errors};
    } finally { await anon.close(); }
}
