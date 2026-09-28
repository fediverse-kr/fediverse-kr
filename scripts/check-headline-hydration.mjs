async (page) => {
  const base=new URL(page.url()).origin, checks=[];
  const check=(ok,label)=>{if(!ok)throw new Error(label);checks.push(label);};
  const indexOf=html=>html.match(/data-headline=["']?(\d+)/)?.[1];
  const indices=[];
  for(let i=0;i<16;i++){
    const response=await page.request.get(base+'/');
    check(response.status()===200,'Homepage SSR response '+i);
    const index=indexOf(await response.text());
    check(index!==undefined && Number(index)<4,'SSR contains a valid chosen headline '+i);
    indices.push(index);
  }
  check(new Set(indices).size>1,'The server chooses different initial headlines across requests');
  await page.addInitScript(()=>{
    window.__headlineSeen=[];
    new MutationObserver(()=>{
      const value=document.querySelector('.landing-headline')?.getAttribute('data-headline');
      if(value!==null && value!==undefined && window.__headlineSeen.at(-1)!==value)window.__headlineSeen.push(value);
    }).observe(document,{childList:true,subtree:true,attributes:true,attributeFilter:['data-headline']});
  });
  for(let i=0;i<5;i++){
    const response=await page.goto(base+'/');
    const initial=indexOf(await response.text());
    await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
    await page.waitForTimeout(2000);
    const seen=await page.evaluate(()=>window.__headlineSeen);
    check(seen.length===1 && seen[0]===initial,'Hydration retains the exact server choice without a flash '+i);
  }
  const initial=await page.locator('.landing-headline').getAttribute('data-headline');
  await page.waitForFunction(value=>document.querySelector('.landing-headline')?.dataset.headline!==value,initial,{timeout:24000});
  check(true,'Slow headline rotation still advances after the reading interval');
  return {checks,serverChoices:indices};
}
