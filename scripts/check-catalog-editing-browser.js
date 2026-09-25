// CLI run-code scenario: isolated member-browser-fixture.ps1 -IncludeCatalog,
// state-load .local/browser-state.json, DB-backed localhost only. No remote IO.
async (page) => {
    const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
    const state=await(await req.get(base+'/api/member/session')).json();
    if(state.member?.display_name!=='Browser test fixture')throw Error('Generated member fixture required');
    const name='browser-'+state.member.id,kind='browser-kind-'+state.member.id;
    const post=(path,data)=>req.post(base+path,{data,headers:{origin:base}});
    const read=async()=>await(await req.get(base+'/api/public/software-edit?name='+name)).json();
    const open=async path=>{await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    const edit={display_name:'가상 사진 도구',family:'',description:'사진으로 인사를 나눠요.\n<script>window.catalogInjected=true</script>',categories:[kind],features:['사진 공유'],website_url:'https://example.org',tech_stack:'Rust'};
    const anon=await ctx.browser().newContext();
    try {
        for(const [path,data] of [
            ['/api/member/software/create',{name,edit,summary:'처음 소개'}],
            ['/api/member/software/save',{name,revision:1,edit,summary:'수정'}],
            ['/api/member/software/restore',{name,current:2,revision:1,summary:'복원'}],
        ]) {
            check(path+' anonymous denied',(await anon.request.post(base+path,{data,headers:{origin:base}})).status()===401);
            check(path+' cross-site denied',(await req.post(base+path,{data,headers:{origin:'https://evil.example'}})).status()===403);
        }
        check('catalog 64 KiB cap',(await post('/api/member/software/save',{padding:'x'.repeat(65537)})).status()===413);
        check('restore 8 KiB cap',(await post('/api/member/software/restore',{padding:'x'.repeat(8193)})).status()===413);
        check('invalid revision rejected',(await req.get(base+'/api/public/software-revision?name='+name+'&revision=-1')).status()===400);
        check('invalid name rejected',(await req.get(base+'/api/public/software-edit?name='+('x'.repeat(129)))).status()===400);
        await open('/account/software/new');
        const html=await req.get(base+'/account/software/new'),markup=await html.text();
        const cookie=(await ctx.cookies()).find(x=>x.name==='fedkr_session');
        check('editor SSR private and secret-free',html.headers()['cache-control'].includes('no-store')&&!markup.includes(cookie.value)&&!markup.includes('fk-verify='));
        await page.getByLabel('식별자',{exact:true}).fill(name.toUpperCase());
        await page.getByLabel('이름',{exact:true}).fill(edit.display_name);
        await page.getByLabel('소개',{exact:true}).fill(edit.description);
        await page.getByLabel('프로젝트 웹사이트 · 선택',{exact:true}).fill(edit.website_url);
        await page.getByLabel('사용 기술 · 선택',{exact:true}).fill(edit.tech_stack);
        await page.getByLabel('Browser catalog kind',{exact:true}).check();
        const feature=page.getByLabel('주요 기능 · 한 줄에 하나, 최대 32개',{exact:true});
        await feature.fill('사진 공유');await feature.press('End');await feature.press('Enter');await feature.pressSequentially('댓글');
        check('feature editor keeps newlines while typing',(await feature.inputValue())==='사진 공유\n댓글');
        await page.getByLabel('변경 요약',{exact:true}).fill('처음 소개');
        for(const width of [1440,390,320]){
            await page.setViewportSize({width,height:width===1440?1000:844});
            check('create no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            check('create readable checkbox '+width,await page.locator('.catalog-kinds input[type=checkbox]').first().evaluate(el=>el.getBoundingClientRect().width<=24));
            await page.screenshot({path:`output/playwright/catalog-new-${width}.png`,fullPage:true});
        }
        let saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/create')&&r.request().method()==='POST');
        await page.getByRole('button',{name:'등록하기',exact:true}).click();check('member creates software',(await saved).status()===200);
        await page.waitForURL(base+'/software/'+name);await page.getByRole('heading',{name:edit.display_name,exact:true}).waitFor();
        check('created content immediately public',(await read()).revision===1);
        check('untrusted source preserved for editing',(await read()).edit.description===edit.description);
        const safeDescription=async()=>await page.locator('.rich-description').evaluate(root=>
            root.textContent.trim()==='사진으로 인사를 나눠요.' &&
            !root.querySelector('script,iframe,object,embed') &&
            [...root.querySelectorAll('*')].every(el=>[...el.attributes].every(a=>!/^on/i.test(a.name)&&!/^\s*javascript:/i.test(a.value)))
        ) && !(await page.evaluate(()=>window.catalogInjected));
        check('untrusted rich text sanitized without execution',await safeDescription());
        await page.reload();await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        check('sanitized SSR and hydration survive reload',await safeDescription());
        const timeOrigin=await page.evaluate(()=>performance.timeOrigin);
        await page.getByRole('link',{name:'정보 수정',exact:true}).click();await page.getByRole('heading',{name:'소프트웨어 정보 수정',exact:true}).waitFor();
        check('detail to edit stays SPA',(await page.evaluate(()=>performance.timeOrigin))===timeOrigin);
        await page.getByLabel('소개',{exact:true}).fill('첫 수정');await page.getByLabel('변경 요약',{exact:true}).fill('소개 수정');
        saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/save')&&r.request().method()==='POST');
        await page.getByRole('button',{name:'변경 저장',exact:true}).click();check('UI edit saved',(await saved).status()===200);await page.waitForURL(base+'/software/'+name);
        await page.getByRole('link',{name:'정보 수정',exact:true}).click();await page.getByLabel('소개',{exact:true}).waitFor();
        check('returning editor reads saved state',(await page.getByLabel('소개',{exact:true}).inputValue())==='첫 수정');
        await page.getByLabel('소개',{exact:true}).fill('내가 쓴 소개');await page.getByLabel('변경 요약',{exact:true}).fill('동시 편집 해결');
        let remote=await read();
        check('concurrent editor writes',(await post('/api/member/software/save',{name,revision:remote.revision,edit:{...remote.edit,description:'다른 사람이 쓴 소개',family:'원격 계열'},summary:'다른 수정'})).status()===200);
        saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/save')&&r.request().method()==='POST');await page.getByRole('button',{name:'변경 저장',exact:true}).click();
        check('stale draft is rejected',(await saved).status()===409);
        check('conflict retains unsaved text',(await page.getByLabel('소개',{exact:true}).inputValue())==='내가 쓴 소개');
        await page.getByRole('button',{name:'최신 내용과 비교',exact:true}).click();await page.getByRole('heading',{name:'서버의 최신 내용 · 버전 3',exact:true}).waitFor();
        await page.getByRole('button',{name:'최신 변경을 합쳐서 계속 편집',exact:true}).click();
        check('merge keeps remote-only field',(await page.getByLabel('계열 · 선택',{exact:true}).inputValue())==='원격 계열');
        check('overlap requires acknowledgement',await page.getByRole('button',{name:'변경 저장',exact:true}).isDisabled());
        check('latest text available for comparison',await page.getByText('다른 사람이 쓴 소개',{exact:true}).count()===1);
        await page.screenshot({path:'output/playwright/catalog-conflict-320.png',fullPage:true});
        await page.getByLabel('겹친 항목을 비교했으며 현재 입력으로 저장합니다.',{exact:true}).check();
        saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/save')&&r.request().method()==='POST');await page.getByRole('button',{name:'변경 저장',exact:true}).click();check('acknowledged merge saves',(await saved).status()===200);await page.waitForURL(base+'/software/'+name);
        check('merged fields persisted',(await read()).edit.family==='원격 계열'&&(await read()).edit.description==='내가 쓴 소개');
        await page.getByRole('link',{name:'변경 이력',exact:true}).click();await page.getByRole('button',{name:'버전 1 보기',exact:true}).waitFor();
        check('public history has all versions',await page.locator('.catalog-history li').count()===4);
        const history=await anon.request.get(base+'/api/public/software-history?name='+name+'&page=0');
        check('history anonymous and no attribution',history.status()===200&&!JSON.stringify(await history.json()).includes('actor_id'));
        const publicHtml=await(await anon.request.get(base+'/software/'+name+'/history')).text();
        check('public history SSR does not load private account',!publicHtml.includes('linked_accounts')&&!publicHtml.includes(cookie.value));
        await page.getByRole('button',{name:'버전 1 보기',exact:true}).click();await page.getByRole('link',{name:'이 버전으로 복원 검토',exact:true}).click();await page.getByRole('heading',{name:'버전 1 복원',exact:true}).waitFor();
        for(const width of [1440,390,320]){
            await page.setViewportSize({width,height:width===1440?1000:844});
            check('restore comparison no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
            await page.screenshot({path:`output/playwright/catalog-restore-${width}.png`,fullPage:true});
        }
        check('restore requires confirmation',await page.getByRole('button',{name:'확인한 버전으로 복원',exact:true}).isDisabled());
        await page.getByLabel('복원 이유',{exact:true}).fill('최초 내용과 비교 후 복원');await page.getByLabel('두 내용을 비교했고 이 버전으로 복원합니다.',{exact:true}).check();
        remote=await read();await post('/api/member/software/save',{name,revision:remote.revision,edit:{...remote.edit,description:'복원 중 다른 수정'},summary:'새 수정'});
        saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/restore')&&r.request().method()==='POST');await page.getByRole('button',{name:'확인한 버전으로 복원',exact:true}).click();check('restore CAS blocks newer changes',(await saved).status()===409);
        await page.getByRole('button',{name:'최신 내용 다시 비교',exact:true}).click();await page.getByRole('heading',{name:'현재 · 버전 5',exact:true}).waitFor();
        check('refresh comparison clears consent',!(await page.getByLabel('두 내용을 비교했고 이 버전으로 복원합니다.',{exact:true}).isChecked()));
        await page.getByLabel('두 내용을 비교했고 이 버전으로 복원합니다.',{exact:true}).check();
        saved=page.waitForResponse(r=>r.url().endsWith('/api/member/software/restore')&&r.request().method()==='POST');await page.getByRole('button',{name:'확인한 버전으로 복원',exact:true}).click();check('reviewed restore saves',(await saved).status()===200);await page.waitForURL(base+'/software/'+name);
        remote=await read();check('restore appends version and recovers content',remote.revision===6&&remote.edit.description===edit.description);
        // Valid Korean long text can exceed auth's 8 KiB without widening auth endpoints.
        check('Korean long content saves',(await post('/api/member/software/save',{name,revision:remote.revision,edit:{...remote.edit,description:'가'.repeat(4000)},summary:'긴 소개'})).status()===200);
        check('invalid website rejected',(await post('/api/member/software/save',{name,revision:7,edit:{...remote.edit,website_url:'javascript:alert(1)'},summary:'잘못된 링크'})).status()===400);
        check('unknown category rejected',(await post('/api/member/software/save',{name,revision:7,edit:{...remote.edit,categories:['no-such-kind']},summary:'없는 분류'})).status()===400);
        check('case-insensitive duplicate rejected',(await post('/api/member/software/create',{name:name.toUpperCase(),edit,summary:'중복 등록'})).status()===409);
        await open('/platforms');await page.getByRole('link',{name:'소프트웨어 등록',exact:true}).waitFor();
        check('catalog exposes direct contribution',await page.getByRole('link',{name:'소프트웨어 등록',exact:true}).count()===1);
        check('no runtime exceptions',errors.length===0);
        return {checks:checks.length,passed:checks,errors};
    } finally {await anon.close();}
}
