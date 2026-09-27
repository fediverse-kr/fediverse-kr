// Public guidance/navigation plus real rejection on an owned synthetic member fixture.
// No server is registered, no external message is sent, and no production member is used.
async (page) => {
    const base = process.env.FEDKR_BROWSER_ORIGIN || 'http://127.0.0.1:12239';
    const checks = [], errors = [];
    const check = (name, ok) => { if (!ok) throw Error(name); checks.push(name); };
    page.on('pageerror', e => errors.push(e.message));
    const anon = await page.context().browser().newContext({reducedMotion:'reduce'});
    const guest = await anon.newPage();
    guest.on('pageerror', e => errors.push(e.message));
    try {
        const response = await guest.goto(base + '/account/sites/new');
        check('anonymous registration HTTP', response.status() === 200);
        await guest.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
        await guest.getByRole('heading', {name:'로그인 후 등록할 수 있어요.'}).waitFor();
        for (const text of ['한국어를 주로 사용하는 커뮤니티', '완전한 비공개 커뮤니티가 아닌 곳', 'fediverse.kr에 공개되기를 원하는 경우', '자동 등록에는 NodeInfo 지원이 필요해요']) {
            check('public guidance: ' + text, (await guest.locator('main').innerText()).includes(text));
        }
        check('no prior consent checkbox', await guest.getByRole('checkbox').count() === 0);
        const detail = guest.locator('.registration-nodeinfo');
        check('technical details initially collapsed', !(await detail.evaluate(e => e.open)));
        await detail.locator('summary').click();
        check('technical details open', await detail.evaluate(e => e.open));
        check('NodeInfo discovery documented', (await detail.innerText()).includes('/.well-known/nodeinfo'));
        for (const width of [1440, 390]) {
            await guest.setViewportSize({width, height:1000});
            check('registration no overflow ' + width, await guest.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
            check('registration readable copy ' + width, await guest.locator('.registration-guidance').evaluate(el => Array.from(el.querySelectorAll('p,li,summary')).every(e => parseFloat(getComputedStyle(e).fontSize) >= 16)));
            await guest.screenshot({path:`output/playwright/registration-help-${width}.png`, fullPage:true});
        }
        await guest.locator('main a[href="/contact"]').first().click();
        await guest.waitForURL(base + '/contact');
        await guest.getByRole('heading', {name:'문의하기', exact:true}).waitFor();
        check('operator link exact', await guest.getByRole('link', {name:'narucafe@lake.naru.cafe', exact:true}).getAttribute('href') === 'https://lake.naru.cafe/@narucafe');
        check('GitHub public request link', await guest.locator('main a[href="https://github.com/fediverse-kr/fediverse-kr/issues/new"]').count() === 1);
        check('no application inbox', await guest.locator('main form,main textarea').count() === 0);
        check('request visibility explained', (await guest.locator('main').innerText()).includes('공개 이슈') && (await guest.locator('main').innerText()).includes('공개 범위'));
        for (const width of [1440, 390]) {
            await guest.setViewportSize({width, height:1000});
            check('contact no overflow ' + width, await guest.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
            check('contact readable copy ' + width, await guest.locator('.contact-content').evaluate(el => Array.from(el.querySelectorAll('p,a')).every(e => parseFloat(getComputedStyle(e).fontSize) >= 16)));
            await guest.screenshot({path:`output/playwright/contact-${width}.png`, fullPage:true});
        }
        await guest.reload();
        await guest.getByRole('heading', {name:'문의하기', exact:true}).waitFor();
        check('contact direct reload', await guest.locator('footer a[href="/contact"]').count() === 1);
        await guest.goto(base + '/about');
        check('about contact link', await guest.locator('main a[href="/contact"]').count() === 1);
        check('obsolete contact notice removed', !(await guest.locator('main').innerText()).includes('아직 연결할 주소는 없습니다'));
        if (process.env.FEDKR_TEST_MEMBER === '1') {
            check('member fixture stays loopback', base === 'http://127.0.0.1:12239');
            check('real anonymous preview denied', (await anon.request.post(base + '/api/member/sites/registration/preview', {data:{domain:'example.org'}, headers:{origin:base}})).status() === 401);
            await page.goto(base + '/account/sites/new');
            const input = page.getByRole('textbox', {name:'서버 주소'});
            await input.fill('http://127.0.0.1/');
            await page.getByRole('button', {name:'서버 정보 확인', exact:true}).click();
            await page.getByRole('alert').waitFor();
            check('real invalid target rejection', (await page.getByRole('alert').innerText()).includes('서버 주소만 입력'));
            check('rejected draft preserved', await input.inputValue() === 'http://127.0.0.1/');
            check('error offers contact path', await page.locator('.registration-error-help a[href="/contact"]').count() === 1);
            await page.locator('.registration-error-help a[href="/contact"]').click();
            await page.waitForURL(base + '/contact');
        }
        check('no browser runtime errors', errors.length === 0);
        return {passed:checks.length, checks, runtimeErrors:errors, scope:'Anonymous guidance, navigation, mobile/desktop, and optional synthetic member API rejection only'};
    } finally { await anon.close(); }
}
