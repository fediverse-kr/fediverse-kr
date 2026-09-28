async(page)=>{
 const base=new URL(page.url()).origin;if(new URL(base).hostname!=='127.0.0.1')throw Error('Loopback fixture required');
 await page.goto(base+'/account/hosting/new');
 await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
 const slug='multiline-'+Date.now();
 await page.locator('#hosting-slug').fill(slug);
 await page.locator('#hosting-name').fill('여러 줄 입력 예시');
 await page.locator('#hosting-website_url').fill('https://'+slug+'.example.com/');
 const text='설치와 업데이트\n백업과 장애 대응';
 await page.locator('#hosting-provider_responsibilities').fill(text);
 await page.locator('#hosting-customer_responsibilities').fill('가입 정책\n이용자 문의 대응');
 await page.locator('#hosting-summary').fill('여러 줄 설명 저장');
 await page.getByRole('button',{name:'저장하기',exact:true}).click();
 await page.waitForURL(base+'/hosting/'+slug,{timeout:5000});
 const saved=await(await page.request.get(base+'/api/public/hosting/detail?slug='+slug)).json();
 await page.getByRole('heading',{name:'여러 줄 입력 예시',exact:true}).waitFor();
 if(await page.locator('.hosting-facts dd').filter({hasText:text}).evaluate(el=>getComputedStyle(el).whiteSpace)!=='pre-wrap')throw Error('Saved line breaks must remain visible');
 if(saved.edit.provider_responsibilities!==text)throw Error('Textarea line breaks must survive canonical storage');
 return {checks:['Multiline responsibilities save through the actual form and retain line breaks']};
}
