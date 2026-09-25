// playwright-cli run-code with an explicit, disposable IncludeModeration fixture.
// All successful mutations use the real local HTTP/PG path. No response mocks.
async page => {
  const base='http://127.0.0.1:12239',ctx=page.context(),req=ctx.request,checks=[],errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  const check=(label,ok)=>{if(!ok)throw Error(label);checks.push(label);};
  const me=await(await req.get(base+'/api/member/session')).json();
  if(me.member?.display_name!=='Browser test fixture')throw Error('Synthetic fixture required');
  const all=await(await req.get(base+'/api/member/moderation/reports?status=all&page=0')).json();
  const domain='browser-'+me.member.id+'.example.org',item=all.reports.find(r=>r.domain===domain);
  if(!item)throw Error('Synthetic report required');
  const id=item.id,url=base+'/account/moderation/'+id;
  const read=async()=>await(await req.get(base+'/api/member/moderation/report?id='+id)).json();
  const command=(d,action,note='다른 관리자 화면에서 처리')=>({report:id,revision:d.revision,comment_revision:d.comment?.revision??null,author_id:d.comment?.author_id??null,author_banned:d.comment?.author_banned??null,author_revision:d.comment?.author_revision??null,action,note});
  const act=request=>req.post(base+'/api/member/moderation/act',{headers:{origin:base},data:{request}});
  const anon=await ctx.browser().newContext();
  try {
    const initial=await read();
    for(const path of ['/api/member/moderation/reports?status=all&page=0','/api/member/moderation/report?id='+id,'/api/member/moderation/events?id='+id+'&page=0']) {
      const r=await anon.request.get(base+path);check('anonymous denied '+path.split('?')[0],r.status()===401);check('private error cache '+path.split('?')[0],r.headers()['cache-control'].includes('no-store'));
    }
    check('anonymous action denied',(await anon.request.post(base+'/api/member/moderation/act',{headers:{origin:base},data:{request:command(initial,'resolve')}})).status()===401);
    check('foreign Origin denied',(await req.post(base+'/api/member/moderation/act',{headers:{origin:'https://evil.example'},data:{request:command(initial,'resolve')}})).status()===403);
    check('8KiB body cap',(await req.post(base+'/api/member/moderation/act',{headers:{origin:base},data:{padding:'x'.repeat(8193)}})).status()===413);
    check('note limit',(await act(command(initial,'resolve','가'.repeat(1001)))).status()===400);
    check('unknown status rejected',(await req.get(base+'/api/member/moderation/reports?status=madeup&page=0')).status()===400);
    check('page upper bound',(await req.get(base+'/api/member/moderation/events?id='+id+'&page=10001')).status()===400);
    const anonHtml=await(await anon.request.get(url)).text();
    check('anonymous SSR excludes evidence',!anonHtml.includes('Private fixture report')&&!anonHtml.includes('Community fixture question'));
    const html=await req.get(url),text=await html.text();
    check('private SSR evidence and no-store',text.includes('Private fixture report')&&html.headers()['cache-control'].includes('no-store')&&html.headers().vary.includes('Cookie'));
    check('no federated linkage in private DTO',!JSON.stringify(initial).includes('browser-fixture.example')&&!JSON.stringify(initial).includes('token_hash'));
    check('legacy admin redirect',(await req.get(base+'/admin/reports',{maxRedirects:0})).headers().location==='/account/moderation/reports');
    await page.goto(base+'/account');await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    await page.getByRole('link',{name:'백오피스 열기',exact:true}).click();
    await page.getByRole('heading',{name:'운영 개요',exact:true}).waitFor();
    await page.locator('.backoffice-nav-rail').getByRole('link',{name:/^신고/}).click();
    await page.getByRole('link',{name:'신고 검토',exact:true}).click();
    await page.getByRole('heading',{name:'신고 당시 원문',exact:true}).waitFor();check('account to report navigation',page.url()===url);
    await page.getByRole('button',{name:'처리 완료',exact:true}).click();
    const note='내 검토 초안 <script>window.moderationInjected=true</script>';
    await page.getByLabel('조치 사유',{exact:true}).fill(note);
    check('concurrent decision commits',(await act(command(await read(),'dismiss'))).status()===200);
    let response=page.waitForResponse(r=>r.url().endsWith('/api/member/moderation/act')&&r.request().method()==='POST');
    await page.getByRole('button',{name:'확인하고 적용',exact:true}).click();
    check('stale UI action rejected',(await response).status()===409);
    await page.getByRole('alert').filter({hasText:'내용이나 상태가 바뀌었습니다'}).waitFor();
    check('draft survives conflict',await page.getByLabel('조치 사유',{exact:true}).inputValue()===note);
    await page.getByRole('button',{name:'최신 내용 확인',exact:true}).click();
    await page.getByRole('status').filter({hasText:'사유 초안은 남겨두었습니다'}).waitFor();
    await page.getByRole('button',{name:'다시 검토',exact:true}).click();
    check('explicit refresh preserves draft',await page.getByLabel('조치 사유',{exact:true}).inputValue()===note);
    const apply=async(label,noteText)=>{
      if(!await page.getByRole('heading',{name:label+' 확인',exact:true}).count())await page.getByRole('button',{name:label,exact:true}).click();
      await page.getByLabel('조치 사유',{exact:true}).fill(noteText);
      const r=page.waitForResponse(r=>r.url().endsWith('/api/member/moderation/act')&&r.request().method()==='POST');
      await page.getByRole('button',{name:'확인하고 적용',exact:true}).click();
      check('UI saves '+label,(await r).status()===200);
      await page.getByRole('status').filter({hasText:label+'를 반영했습니다.'}).waitFor();
    };
    await apply('다시 검토',note);
    await page.getByRole('heading',{name:'조치 이력',exact:true}).waitFor();
    await page.locator('.moderation-events').getByText(note,{exact:true}).waitFor();
    check('history is refreshed after save',true);check('HTML rendered as text',!await page.evaluate(()=>window.moderationInjected));
    await apply('댓글 삭제 표시','공개 화면에서 댓글을 숨김');
    let after=await read();check('delete preserves report and evidence',after.summary.status==='pending'&&after.comment.deleted&&after.comment.body===initial.comment.body&&after.evidence.body===initial.evidence.body);
    const publicPage=await(await req.get(base+'/api/public/comments?domain='+domain+'&page=0')).json();
    const deleted=publicPage.threads.find(t=>t.comment.id===initial.comment.id);
    check('public tombstone preserves replies',deleted.comment.deleted&&deleted.comment.body===''&&deleted.reply_count===4);
    await apply('작성자 이용 차단','반복 위반 확인');check('ban persisted',(await read()).comment.author_banned===true);
    await apply('작성자 차단 해제','재검토 후 해제');check('unban does not restore comment',(await read()).comment.deleted===true);
    await apply('처리 완료','필요한 조치를 마쳤습니다.');
    check('resolution persisted',(await read()).summary.status==='resolved');
    const events=await(await req.get(base+'/api/member/moderation/events?id='+id+'&page=0')).json();check('every successful action audited',events.events.length===6);
    for(const width of [1440,390,320]) {
      await page.setViewportSize({width,height:width===1440?1000:844});
      check('no horizontal overflow '+width,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
      await page.locator('#content').screenshot({path:'output/playwright/moderation-'+width+'.png'});
    }
    const beforeReload=page.url();await page.reload();await page.getByRole('heading',{name:'신고 검토',exact:true}).waitFor();
    check('reload retains state',(await read()).summary.status==='resolved'&&page.url()===beforeReload);
    const allLink=page.getByRole('link',{name:'신고 목록',exact:true});await allLink.click();
    await page.getByRole('button',{name:'처리 완료',exact:true}).click();await page.getByRole('link',{name:'신고 검토',exact:true}).waitFor();check('resolved filter displays handled report',true);
    check('no runtime exceptions',errors.length===0);
    return {passed:checks.length,runtimeErrors:errors.length,checks};
  } finally {await anon.close();}
}
