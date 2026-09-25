// CLI scenario; generated local PG member with IncludeCatalog + IncludeCatalogAdmin.
// Every successful save goes through the real HTTP/PG implementation. No mocks.
async page => {
    const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
    const get=async path=>await(await req.get(base+path)).json();
    const post=(path,data)=>req.post(base+path,{data,headers:{origin:base}});
    const state=await get('/api/member/session');
    if(state.member?.display_name!=='Browser test fixture'||!await get('/api/member/moderation/access'))throw Error('Synthetic catalog administrator required');
    const name='browser-'+state.member.id,kind='browser-kind-'+state.member.id,extra='browser-extra-kind-'+state.member.id;
    const itemPath='/api/member/moderation/software/item?name='+name;
    const edit={display_name:'가상 읽기 도구',family:'',description:'책과 글을 함께 나눠요.',categories:[kind],features:['책 공유'],website_url:'https://example.org',tech_stack:'Rust'};
    const act=(revision,action)=>post('/api/member/moderation/software/act',{request:{name,revision,action,note:'관리자만 읽는 테스트 사유'}});
    const open=async path=>{await page.goto(base+path);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');};
    const save=async(path,label)=>{const r=page.waitForResponse(r=>r.url().endsWith(path)&&r.request().method()==='POST');await page.getByRole('button',{name:label,exact:true}).click();return await r;};
    const anon=await ctx.browser().newContext();
    try{
        for(const path of ['/api/member/moderation/software?query=&locked=false&page=0',itemPath,'/api/member/moderation/software/editable?name='+name,'/api/member/moderation/categories?query=&page=0','/api/member/moderation/category?name='+kind,'/api/member/moderation/catalog/history?name='+name+'&category=false&page=0'])check('anonymous private read '+path,(await anon.request.get(base+path)).status()===401);
        const calls=[['/api/member/moderation/software/save',{name,revision:0,edit,summary:'공개 수정'}],['/api/member/moderation/software/act',{request:{name,revision:0,action:{kind:'locked',value:true},note:'검사'}}],['/api/member/moderation/category/save',{request:{name:kind,revision:0,edit:{label:'종류',emoji:'',display_order:0},note:'검사'}}]];
        for(const[path,data]of calls){check('anonymous write '+path,(await anon.request.post(base+path,{data,headers:{origin:base}})).status()===401);check('Origin '+path,(await req.post(base+path,{data,headers:{origin:'https://evil.example'}})).status()===403);}
        check('settings 8 KiB',(await post('/api/member/moderation/software/act',{padding:'x'.repeat(8193)})).status()===413);
        check('content 64 KiB',(await post('/api/member/moderation/software/save',{padding:'x'.repeat(65537)})).status()===413);
        check('category 8 KiB',(await post('/api/member/moderation/category/save',{padding:'x'.repeat(8193)})).status()===413);
        check('member software creation',(await post('/api/member/software/create',{name,edit,summary:'처음 소개'})).status()===200);
        const path='/account/moderation/catalog/software/'+name;
        await open('/account/moderation/catalog');await page.getByRole('link',{name:edit.display_name,exact:true}).click();await page.getByRole('heading',{name:'제품 관리 설정',exact:true}).waitFor();
        const html=await req.get(base+path),markup=await html.text(),cookie=(await ctx.cookies()).find(c=>c.name==='fedkr_session');
        check('private SSR no secrets',html.status()===200&&html.headers()['cache-control'].includes('no-store')&&!markup.includes(cookie.value)&&!markup.includes('browser-fixture.example'));
        await page.getByRole('button',{name:'회원 편집 잠금',exact:true}).click();await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).fill('관리자만 읽는 테스트 사유');
        check('UI locks',(await save('/api/member/moderation/software/act','확인하고 저장')).status()===200);
        await page.getByRole('button',{name:'회원 편집 잠금 해제',exact:true}).waitFor();let current=await get(itemPath);check('lock persisted',current.locked&&current.revision===2);
        check('member endpoint stays locked',(await post('/api/member/software/save',{name,revision:2,edit,summary:'일반 수정'})).status()===423);
        await page.getByRole('link',{name:'소개 수정',exact:true}).click();await page.getByLabel('소개',{exact:true}).waitFor();
        check('shared editor permits authorized locked edit',await page.getByLabel('소개',{exact:true}).isEnabled());
        const description='한글'.repeat(2000);await page.getByLabel('소개',{exact:true}).fill(description);await page.getByLabel('변경 요약',{exact:true}).fill('관리자 본문 교정');
        check('shared 4000-character edit',(await save('/api/member/moderation/software/save','변경 저장')).status()===200);await page.waitForURL(base+'/software/'+name);
        const updated=await get('/api/public/software-edit?name='+name);check('public content updated lock kept',updated.edit.description===description&&updated.locked&&updated.revision===3);
        const publicHistory=await get('/api/public/software-history?name='+name+'&page=0');check('public history excludes private reason',publicHistory.entries[0].action==='admin_edit'&&!JSON.stringify(publicHistory).includes('관리자만 읽는'));
        await open(path);await page.getByRole('button',{name:'표시 순서 변경',exact:true}).click();await page.getByLabel('표시 순서',{exact:true}).fill('-12');await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).fill('내 정렬 사유');
        current=await get(itemPath);check('concurrent metadata edit',(await act(current.revision,{kind:'featured',value:true})).status()===200);
        check('stale settings rejected',(await save('/api/member/moderation/software/act','확인하고 저장')).status()===409);
        check('conflict keeps note',await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).inputValue()==='내 정렬 사유');
        await page.getByRole('button',{name:'최신 설정 확인',exact:true}).click();await page.getByRole('button',{name:'최신 설정을 확인하고 계속',exact:true}).click();
        check('explicit conflict retry',(await save('/api/member/moderation/software/act','확인하고 저장')).status()===200);
        current=await get(itemPath);check('independent fields preserved',current.display_order===-12&&current.featured&&current.locked);
        check('unsafe color rejected',(await act(current.revision,{kind:'brand_color',value:'red;display:none'})).status()===400);
        await page.getByRole('button',{name:'브랜드 색상 변경',exact:true}).click();await page.getByLabel('색상 · 비우면 해제',{exact:true}).fill('#123abc');await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).fill('색상 정리');
        check('UI color stored',(await save('/api/member/moderation/software/act','확인하고 저장')).status()===200);check('canonical color',(await get(itemPath)).brand_color==='#123ABC');
        for(const width of[1440,390,320]){await page.setViewportSize({width,height:width===1440?1000:844});check('settings no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await page.evaluate(()=>window.scrollTo({top:0,behavior:'instant'}));await page.screenshot({path:`output/playwright/catalog-admin-${width}.png`,fullPage:true});}
        await open('/account/moderation/catalog/categories/new');await page.getByLabel('식별자',{exact:true}).fill(extra);await page.getByLabel('표시 이름',{exact:true}).fill('독서와 기록');await page.getByLabel('아이콘 · 문자 또는 이모지, 선택',{exact:true}).fill('📖');await page.getByLabel('표시 순서 · 작은 수부터',{exact:true}).fill('-5');await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).fill('비공개 종류 추가 사유');
        check('UI category created',(await save('/api/member/moderation/category/save','종류 저장')).status()===200);await page.waitForURL(base+'/account/moderation/catalog/category/'+extra);
        check('identifier immutable',await page.getByLabel('식별자',{exact:true}).getAttribute('readonly')!==null);
        check('kind visible to members',(await get('/api/public/software-kinds')).items.some(c=>c.name===extra&&c.label==='독서와 기록'));
        await page.getByLabel('표시 이름',{exact:true}).fill('내 분류 이름');await page.getByLabel('관리 사유 · 관리자에게만 공개',{exact:true}).fill('내 분류 변경 사유');
        let category=await get('/api/member/moderation/category?name='+extra);
        check('concurrent category change',(await post('/api/member/moderation/category/save',{request:{name:extra,revision:category.revision,edit:{...category.edit,label:'먼저 저장한 이름'},note:'다른 창'}})).status()===200);
        check('category stale revision',(await save('/api/member/moderation/category/save','종류 저장')).status()===409);
        check('category draft preserved',await page.getByLabel('표시 이름',{exact:true}).inputValue()==='내 분류 이름');
        await page.getByRole('button',{name:'최신 내용과 비교',exact:true}).click();await page.getByRole('button',{name:'비교했으며 현재 입력으로 계속',exact:true}).click();
        check('category conflict resolved',(await save('/api/member/moderation/category/save','종류 저장')).status()===200);
        const h=await get('/api/member/moderation/catalog/history?name='+extra+'&category=true&page=0');check('category history complete',h.items.length===3&&h.items[2].before==='null');
        await page.locator('.moderation-events').getByText('내 분류 변경 사유',{exact:true}).waitFor();
        check('history shows human-readable fields',!(await page.locator('.moderation-events').innerText()).includes('updated_at'));
        for(const width of[1440,390,320]){await page.setViewportSize({width,height:width===1440?1000:844});check('category no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await page.evaluate(()=>window.scrollTo({top:0,behavior:'instant'}));await page.screenshot({path:`output/playwright/category-admin-${width}.png`,fullPage:true});}
        const anonPage=await anon.newPage();const response=await anonPage.goto(base+'/account/moderation/catalog/category/'+extra);check('private category SSR denied',response.status()===401&&!(await anonPage.content()).includes('내 분류 변경 사유'));
        check('no runtime errors',errors.length===0);return {passed:checks.length,checks,runtimeErrors:errors};
    }finally{await anon.close();}
}
