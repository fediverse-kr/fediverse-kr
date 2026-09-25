// CLI run-code smoke: member-browser-fixture.ps1 -IncludeSite -IncludeCommunity.
// Local synthetic data only. Does not publish an AP post or contact the fake site.
async page => {
  const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
  const me=await(await req.get(base+'/api/member/session')).json();
  if(me.member?.display_name!=='Browser test fixture')throw Error('Synthetic fixture required');
  const domain='browser-'+me.member.id+'.example.org',url=base+'/servers/'+domain;
  const read=async()=>await(await req.get(base+'/api/public/comments?domain='+domain+'&page=0')).json();
  const post=(op,data)=>req.post(base+'/api/member/comments/'+op,{data:{domain,...data},headers:{origin:base}});
  await page.goto(url);await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
  await page.getByRole('button',{name:'수정',exact:true}).waitFor();
  const start=await read(),own=start.threads.find(t=>t.comment.body==='Own fixture comment').comment,other=start.threads.find(t=>t.comment.body==='Community fixture question').comment;
  const ownCard=page.locator('#comment-'+own.id),otherCard=page.locator('#comment-'+other.id);
  const anon=await ctx.browser().newContext();
  try {
    const html=await(await anon.request.get(url)).text();
    const cookie=(await ctx.cookies()).find(c=>c.name==='fedkr_session');
    check('public SSR has comments and no private linkage',html.includes('Community fixture question')&&!html.includes('browser-fixture.example')&&!html.includes(cookie.value)&&!html.includes('linked_accounts'));
    for(const [op,data] of [['create',{parent:null,body:'new'}],['edit',{id:own.id,revision:own.revision,body:'edit'}],['delete',{id:own.id,revision:own.revision}],['report',{id:other.id,reason:'spam',detail:''}]]) {
      check(op+' denies anonymous',(await anon.request.post(base+'/api/member/comments/'+op,{data:{domain,...data},headers:{origin:base}})).status()===401);
      check(op+' rejects foreign Origin',(await req.post(base+'/api/member/comments/'+op,{data:{domain,...data},headers:{origin:'https://evil.example'}})).status()===403);
    }
    check('comment payload cap',(await post('create',{padding:'x'.repeat(16385)})).status()===413);
    check('text limit',(await post('create',{parent:null,body:'가'.repeat(2001)})).status()===400);
    const publicResponse=await req.get(base+'/api/public/comments?domain='+domain+'&page=0');
    check('public response no-store',publicResponse.headers()['cache-control'].includes('no-store'));
    check('DTO excludes identities and report data',!JSON.stringify(await publicResponse.json()).includes('actor_url')&&!JSON.stringify(start).includes('reporter_id'));
    await page.getByRole('button',{name:'답글 4개 모두 보기',exact:true}).click();
    await page.getByText('Fixture reply 4',{exact:true}).waitFor();check('expanded replies render',true);
    await page.getByLabel('이 서버에 대한 경험이나 질문',{exact:true}).fill('아직 작성 중인 내용');
    await ownCard.getByRole('button',{name:'수정',exact:true}).click();
    await ownCard.getByLabel('댓글 수정',{exact:true}).fill('내 수정 초안');
    check('remote edit commits',(await post('edit',{id:own.id,revision:own.revision,body:'다른 화면에서 수정'})).status()===200);
    // Manual refresh shares the live-notification refresh path; no document reload.
    await page.getByRole('button',{name:'댓글 새로고침',exact:true}).click();
    await ownCard.getByRole('heading',{name:'먼저 저장된 내용',exact:true}).waitFor();
    check('refresh keeps root draft',(await page.getByLabel('이 서버에 대한 경험이나 질문',{exact:true}).inputValue())==='아직 작성 중인 내용');
    check('refresh keeps edit draft',(await ownCard.getByLabel('댓글 수정',{exact:true}).inputValue())==='내 수정 초안');
    let response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/edit')&&r.request().method()==='POST');
    await ownCard.getByRole('button',{name:'수정 저장',exact:true}).click();check('stale revision rejected',(await response).status()===409);
    await ownCard.getByRole('button',{name:'입력을 버리고 최신 내용 불러오기',exact:true}).click();
    check('explicit reload replaces draft',(await ownCard.getByLabel('댓글 수정',{exact:true}).inputValue())==='다른 화면에서 수정');
    const text='한글 수정\n<script>window.communityInjected=true</script>';
    await ownCard.getByLabel('댓글 수정',{exact:true}).fill(text);
    response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/edit')&&r.request().method()==='POST');
    await ownCard.getByRole('button',{name:'수정 저장',exact:true}).click();check('edit UI saves',(await response).status()===200);
    await ownCard.getByText(text,{exact:true}).waitFor();check('plain text escapes HTML',!(await page.evaluate(()=>window.communityInjected)));
    await otherCard.getByRole('button',{name:'답글',exact:true}).click();
    await otherCard.getByLabel('답글 내용',{exact:true}).fill('다른 회원에게 답글을 남겨요.');
    response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/create')&&r.request().method()==='POST');
    await otherCard.getByRole('button',{name:'댓글 작성',exact:true}).click();check('reply UI saves',(await response).status()===200);
    await page.getByText('다른 회원에게 답글을 남겨요.',{exact:true}).waitFor();check('expanded replies refresh after save',true);
    check('one minute rule',(await post('create',{parent:null,body:'too soon'})).status()===429);
    await otherCard.getByRole('button',{name:'신고',exact:true}).click();
    await otherCard.getByLabel('신고 사유',{exact:true}).selectOption('other');
    await otherCard.getByLabel('신고 설명 · 선택',{exact:true}).fill('비공개 신고 설명');
    for(const width of [1440,390,320]) {
      await page.setViewportSize({width,height:width===1440?1000:844});
      check('no overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
      await page.locator('.community-section').screenshot({path:'output/playwright/community-'+width+'.png'});
    }
    response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/report')&&r.request().method()==='POST');
    await otherCard.getByRole('button',{name:'신고 접수',exact:true}).click();check('report UI saves',(await response).status()===200);
    await otherCard.getByText('신고가 접수되었습니다. 바로 삭제되는 것은 아니에요.',{exact:true}).waitFor();
    check('report detail private',!JSON.stringify(await read()).includes('비공개 신고 설명'));
    check('duplicate report blocked',(await post('report',{id:other.id,reason:'spam',detail:''})).status()===409);
    await ownCard.getByRole('button',{name:'삭제',exact:true}).click();
    const before=(await read()).threads.find(t=>t.comment.id===own.id).comment;
    await post('edit',{id:own.id,revision:before.revision,body:'삭제 확인 중 다른 수정'});
    await page.getByRole('button',{name:'댓글 새로고침',exact:true}).click();
    await ownCard.getByText('삭제 확인 중 다른 수정',{exact:true}).waitFor();
    response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/delete')&&r.request().method()==='POST');
    await ownCard.getByRole('button',{name:'확인 · 댓글 삭제',exact:true}).click();check('delete pins confirmation revision',(await response).status()===409);
    await ownCard.getByRole('button',{name:'닫기',exact:true}).click();await ownCard.getByRole('button',{name:'삭제',exact:true}).click();
    response=page.waitForResponse(r=>r.url().endsWith('/api/member/comments/delete')&&r.request().method()==='POST');
    await ownCard.getByRole('button',{name:'확인 · 댓글 삭제',exact:true}).click();check('confirmed delete',(await response).status()===200);
    await ownCard.getByText('삭제된 댓글입니다.',{exact:true}).waitFor();
    const deleted=(await read()).threads.find(t=>t.comment.id===own.id).comment;
    check('tombstone removes name and body in API',deleted.deleted&&deleted.author_name===''&&deleted.body==='');
    check('runtime exceptions',errors.length===0);
    return {passed:checks.length,runtimeErrors:errors.length};
  } finally {await anon.close();}
}
