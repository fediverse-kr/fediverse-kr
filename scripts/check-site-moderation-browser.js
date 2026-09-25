// Run with playwright-cli and a fresh IncludeSite/IncludeCommunity/IncludeModeration
// fixture. Successful writes use actual HTTP/PG, with no response mocks.
async page => {
  const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  const check=(name,ok)=>{if(!ok)throw Error(name);checks.push(name);};
  const openSites=async()=>{
    const rail=page.locator('.backoffice-nav-rail');
    if(await rail.isVisible())await rail.getByRole('link',{name:/^서버/}).click();
    else {
      const menu=page.locator('.backoffice-mobile-sections');
      if(await menu.getAttribute('open')===null)await menu.locator('summary').click();
      await menu.getByRole('link',{name:/^서버/}).click();
    }
  };
  const me=await(await req.get(base+'/api/member/session')).json();
  if(me.member?.display_name!=='Browser test fixture')throw Error('Synthetic fixture required');
  const domain='browser-'+me.member.id+'.example.org';
  const listUrl=base+'/api/member/moderation/sites?query='+domain+'&status=all&sort=domain&page=0';
  const list=await(await req.get(listUrl)).json(),item=list.sites?.find(s=>s.domain===domain);
  if(!item||item.revision!==0||!item.closed||item.hidden||item.force_hidden)throw Error('Fresh closed synthetic site required');
  const id=item.id,url=base+'/account/moderation/sites/'+id,readUrl=base+'/api/member/moderation/site?id='+id;
  const read=async()=>await(await req.get(readUrl)).json();
  const command=(d,action,note='다른 관리자 화면의 변경')=>({id,revision:d.revision,action,note});
  const act=request=>req.post(base+'/api/member/moderation/site/act',{headers:{origin:base},data:{request}});
  const anon=await ctx.browser().newContext();
  try{
    for(const path of [listUrl,readUrl,base+'/api/member/moderation/site/history?id='+id+'&page=0']){
      const r=await anon.request.get(path);check('anonymous read denied '+path.split('?')[0],r.status()===401);check('private error headers '+path.split('?')[0],r.headers()['cache-control'].includes('no-store'));
    }
    const initial=await read();
    check('anonymous write denied',(await anon.request.post(base+'/api/member/moderation/site/act',{headers:{origin:base},data:{request:command(initial,{kind:'hidden',value:true})}})).status()===401);
    check('Origin denied',(await req.post(base+'/api/member/moderation/site/act',{headers:{origin:'https://evil.example'},data:{request:command(initial,{kind:'hidden',value:true})}})).status()===403);
    check('actual body bounded',(await req.post(base+'/api/member/moderation/site/act',{headers:{origin:base},data:{padding:'x'.repeat(8193)}})).status()===413);
    check('note upper bound',(await act(command(initial,{kind:'hidden',value:true},'가'.repeat(1001)))).status()===400);
    check('tag upper bound',(await act(command(initial,{kind:'tags',value:'가'.repeat(257)}))).status()===400);
    check('invalid sort rejected',(await req.get(listUrl.replace('sort=domain','sort=anything'))).status()===400);
    check('history page bound',(await req.get(base+'/api/member/moderation/site/history?id='+id+'&page=10001')).status()===400);
    check('missing private detail',(await req.get(base+'/api/member/moderation/site?id=00000000-0000-0000-0000-000000000000')).status()===404);
    check('private DTO excludes linkage',!JSON.stringify(initial).includes('browser-fixture.example')&&!JSON.stringify(initial).includes('owner_id')&&!JSON.stringify(initial).includes('previous'));
    const ssr=await req.get(url),html=await ssr.text();
    check('private SSR has current site',html.includes(domain)&&ssr.headers()['cache-control'].includes('no-store')&&ssr.headers().vary.includes('Cookie'));
    const anonHtml=await(await anon.request.get(url)).text();check('anonymous SSR excludes site',!anonHtml.includes(domain)&&!anonHtml.includes('Browser owner fixture'));
    check('legacy admin URL redirect',(await req.get(base+'/admin/servers',{maxRedirects:0})).headers().location==='/account/moderation/sites');
    await page.goto(base+'/account');await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    await page.getByRole('link',{name:'백오피스 열기',exact:true}).click();
    await page.getByRole('heading',{name:'운영 개요',exact:true}).waitFor();
    await openSites();
    await page.getByRole('link',{name:'상태와 조치 보기',exact:true}).click();
    await page.getByRole('heading',{name:'서버 상태와 조치',exact:true}).waitFor();check('account to server management navigation',page.url()===url);
    await page.getByRole('button',{name:'강제 숨김',exact:true}).click();
    const note='관리자 검토 초안 <script>window.siteAdminInjected=true</script>';
    await page.getByLabel('조치 사유',{exact:true}).fill(note);
    check('concurrent admin write commits',(await act(command(await read(),{kind:'hidden',value:true}))).status()===200);
    let response=page.waitForResponse(r=>r.url().endsWith('/api/member/moderation/site/act')&&r.request().method()==='POST');
    await page.getByRole('button',{name:'확인하고 적용',exact:true}).click();check('stale UI revision rejected',(await response).status()===409);
    await page.getByRole('alert').filter({hasText:'내용이나 상태가 바뀌었습니다'}).waitFor();
    check('draft retained after conflict',await page.getByLabel('조치 사유',{exact:true}).inputValue()===note);
    await page.getByRole('button',{name:'최신 상태 확인',exact:true}).click();
    await page.getByRole('status').filter({hasText:'사유 초안은 유지합니다'}).waitFor();
    await page.getByRole('button',{name:'강제 숨김',exact:true}).click();
    check('refresh retains draft',await page.getByLabel('조치 사유',{exact:true}).inputValue()===note);
    const apply=async(label,reason=note,tags=null)=>{
      if(!await page.getByRole('heading',{name:label+' 확인',exact:true}).count())await page.getByRole('button',{name:label,exact:true}).click();
      if(tags!==null)await page.getByLabel('교체할 태그',{exact:true}).fill(tags);
      await page.getByLabel('조치 사유',{exact:true}).fill(reason);
      const r=page.waitForResponse(r=>r.url().endsWith('/api/member/moderation/site/act')&&r.request().method()==='POST');
      await page.getByRole('button',{name:'확인하고 적용',exact:true}).click();
      check('UI save '+label,(await r).status()===200);
      await page.getByRole('status').filter({hasText:'요청을 반영했습니다.'}).waitFor();
    };
    await apply('강제 숨김');
    let d=await read();check('independent hiding flags',d.hidden&&d.force_hidden&&d.closed);
    check('hidden public detail 404',(await req.get(base+'/servers/'+domain)).status()===404);
    const forced=await(await req.get(listUrl.replace('status=all','status=force_hidden'))).json();check('admin still finds hidden site',forced.sites.length===1);
    await apply('강제 숨김 해제','일반 숨김은 유지');
    d=await read();check('general hiding survives forced unhide',d.hidden&&!d.force_hidden);
    check('general hiding still excludes public detail',(await req.get(base+'/servers/'+domain)).status()===404);
    await apply('강제 숨김','두 숨김 상태를 다시 확인');
    await apply('일반 숨김 해제','일반 숨김만 해제');
    check('force hiding stays active',(await read()).force_hidden===true);
    await apply('강제 숨김 해제','강제 숨김도 해제');
    check('public detail returns without reopening site',(await req.get(base+'/servers/'+domain)).status()===200&&(await read()).closed);
    await apply('태그 수정','태그만 변경','사진, 개발, 사진');check('tags canonicalized',(await read()).tags==='사진, 개발');
    await apply('초대제로 표시','초대제 안내 수정');check('invite flag persisted',(await read()).invite_only===true);
    await apply('초대제 미확인으로 표시','확인되지 않은 상태로');check('unknown not false',(await read()).invite_only===null);
    const beforeRefresh=await read();await apply('지금 수집 요청','가상 서버 단발 수집');
    d=await read();check('refresh keeps closure and adds revision',d.closed&&d.revision===beforeRefresh.revision+1&&!!d.refresh);
    check('shared manual refresh limit',(await act(command(d,{kind:'refresh'}))).status()===429);
    const history=await(await req.get(base+'/api/member/moderation/site/history?id='+id+'&page=0')).json();
    check('only successful changes audited',history.events.length===10);
    check('audit DTO excludes original snapshot',!JSON.stringify(history).includes('owner_id')&&!JSON.stringify(history).includes('previous'));
    const owner=await(await req.get(base+'/api/member/sites?page=0')).json();
    const owned=owner.sites?.find(s=>s.domain===domain);
    check('owner metadata preserved',owned?.edit.rules==='Fixture rules'&&owned.edit.owner_comment==='Fixture owner comment');
    check('HTML in notes not executed',!await page.evaluate(()=>window.siteAdminInjected));
    await page.reload();await page.getByRole('heading',{name:'서버 상태와 조치',exact:true}).waitFor();
    await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    await page.getByRole('button',{name:'태그 수정',exact:true}).click();
    check('reload retains tags',await page.getByLabel('교체할 태그',{exact:true}).inputValue()==='사진, 개발');
    await page.getByLabel('조치 사유',{exact:true}).fill('확인만 하는 초안');
    for(const width of [1440,390,320]){
      await page.setViewportSize({width,height:width===1440?1000:844});
      check('no horizontal overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
      // Capture from the document origin; element clipping during a smooth scroll
      // can paint offscreen fixed skip-links into tall Chromium screenshots.
      await page.evaluate(()=>window.scrollTo({top:0,behavior:'instant'}));
      await page.screenshot({path:'output/playwright/site-moderation-'+width+'.png',fullPage:true});
    }
    await page.getByRole('button',{name:'취소',exact:true}).click();
    await openSites();
    await page.getByLabel('도메인·이름 검색',{exact:true}).fill('no-matching-fixture');await page.getByRole('button',{name:'검색',exact:true}).click();
    await page.getByText('조건에 맞는 서버가 없습니다.',{exact:true}).waitFor();check('list search empty state',true);
    await page.getByLabel('도메인·이름 검색',{exact:true}).fill(domain);await page.getByRole('button',{name:'검색',exact:true}).click();
    await page.getByRole('link',{name:'상태와 조치 보기',exact:true}).waitFor();
    await page.getByLabel('표시 상태',{exact:true}).selectOption('closed');await page.getByRole('link',{name:'상태와 조치 보기',exact:true}).waitFor();check('closed filter retains fixture',true);
    check('runtime exceptions absent',errors.length===0);
    return{passed:checks.length,runtimeErrors:errors.length,checks};
  }finally{await anon.close();}
}
