// playwright-cli callback. Seed IncludeSite/IncludeCommunity/IncludeModeration/
// IncludeWorkerMonitor, then load the generated browser state. Real HTTP/PG;
// the held synthetic lease is not evidence of a real running crawler. Run the
// CLI role test first: this callback logs out the synthetic session at the end.
async page => {
    const ctx = page.context(), req = ctx.request, base = await page.evaluate(() => location.origin);
    if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(base)) throw Error('Local fixture server required');
    const endpoint = base + '/api/member/moderation/workers';
    const url = base + '/account/moderation/workers', checks = [], errors = [];
    const check = (label, ok) => { if (!ok) throw Error(label); checks.push(label); };
    page.on('pageerror', e => errors.push(e.message));
    const me = await (await req.get(base + '/api/member/session')).json();
    if (me.member?.display_name !== 'Browser test fixture') throw Error('Synthetic member required');
    const domain = 'browser-' + me.member.id + '.example.org';
    const remote = 'worker-' + me.member.id + '.example.org';
    const anon = await ctx.browser().newContext();
    const nojs = await ctx.browser().newContext({javaScriptEnabled: false, storageState: await ctx.storageState()});
    const sentinels = ['private worker raw exception sentinel', 'private remote error sentinel',
        'private nodeinfo error sentinel', 'lease_token', 'token_hash', 'browser-fixture.example'];
    const snapshot = () => page.locator('body').ariaSnapshot();
    try {
        const denied = await anon.request.get(endpoint);
        check('anonymous API denied', denied.status() === 401);
        check('anonymous error not cached', (denied.headers()['cache-control'] || '').includes('no-store'));
        const anonSsr = await anon.request.get(url), anonText = await anonSsr.text();
        check('anonymous SSR hides private sites', !anonText.includes(domain) && !anonText.includes(remote));
        check('anonymous SSR not cached', (anonSsr.headers()['cache-control'] || '').includes('no-store'));
        const response = await req.get(endpoint), data = await response.json();
        check('admin API succeeds', response.status() === 200);
        check('admin response not cached', (response.headers()['cache-control'] || '').includes('no-store'));
        check('closed job failure retained', data.job_issues.some(i => i.domain === domain && i.kind === 'dead' && i.attempts === 3));
        check('closed automatic job paused', data.queue.paused >= 1);
        check('held synthetic lease counted', data.queue.running >= 1);
        check('hidden remote failure retained', data.site_issues.some(i => i.domain === remote && i.status_code === 503 && !i.alive && i.nodeinfo_failed));
        check('remote failure is not worker failure', !data.job_issues.some(i => i.domain === remote));
        check('closed server absent from remote issues', !data.site_issues.some(i => i.domain === domain));
        check('bounded API issue lists', data.job_issues.length <= 20 && data.site_issues.length <= 20);
        check('raw errors and identity omitted', sentinels.every(s => !JSON.stringify(data).includes(s)));
        const fields = ['captured_at', 'queue', 'job_issues_total', 'job_issues', 'site_issues_total', 'site_issues'];
        check('overview field whitelist', Object.keys(data).sort().join() === fields.sort().join());
        check('job field whitelist', data.job_issues.every(i => Object.keys(i).sort().join() === ['site_id','domain','scheduled_at','attempts','kind'].sort().join()));
        check('remote field whitelist', data.site_issues.every(i => Object.keys(i).sort().join() === ['site_id','domain','checked_at','alive','status_code','nodeinfo_failed'].sort().join()));
        const ssr = await req.get(url), ssrText = await ssr.text();
        check('authenticated SSR contains fixture data', ssrText.includes(domain) && ssrText.includes(remote));
        check('private SSR cache boundary', (ssr.headers()['cache-control'] || '').includes('no-store') && /cookie/i.test(ssr.headers().vary || ''));
        check('SSR omits raw errors', sentinels.every(s => !ssrText.includes(s)));
        const staticPage = await nojs.newPage();
        await staticPage.goto(url); await staticPage.locator('body').ariaSnapshot();
        check('JS-disabled SSR usable', await staticPage.getByRole('heading', {name: domain, exact: true}).isVisible() && await staticPage.getByRole('heading', {name: remote, exact: true}).isVisible());
        check('JS-disabled refresh disabled', await staticPage.getByRole('button', {name: '수집 현황 새로고침'}).isDisabled());
        await page.goto(base + '/account');
        await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
        await snapshot();
        await page.getByRole('link', {name: '백오피스 열기', exact: true}).click();
        await page.getByRole('heading', {name: '운영 개요', exact: true}).waitFor();
        await page.locator('.backoffice-nav-rail').getByRole('link', {name: /^작업·유지보수/}).click();
        await page.waitForURL(url); await snapshot();
        const jobs = page.getByRole('region', {name: '작업 확인 필요 항목'});
        const sites = page.getByRole('region', {name: '상대 서버 확인 필요 항목'});
        await jobs.getByRole('heading', {name: domain, exact: true}).waitFor();
        await sites.getByRole('heading', {name: remote, exact: true}).waitFor();
        check('separate issue regions rendered', true);
        check('current snapshot and polling limitation explained', await page.getByText('현재 스냅샷입니다. DB에 기록된 실행 중 상태는 워커의 실제 응답을 증명하지 않습니다.', {exact: true}).isVisible());
        const jobDisplayed = Math.min(data.job_issues.length, 10), siteDisplayed = Math.min(data.site_issues.length, 10);
        check('job sample count distinguished from total', await jobs.getByText(`전체 ${data.job_issues_total}건 · ${jobDisplayed}건 표시`, {exact: true}).isVisible());
        check('remote sample count distinguished from total', await sites.getByText(`전체 ${data.site_issues_total}건 · ${siteDisplayed}건 표시`, {exact: true}).isVisible());
        check('rendered issue lists limited to ten', await jobs.locator('li').count() === jobDisplayed && await sites.locator('li').count() === siteDisplayed);
        check('completion is not labelled success', await page.getByText('완료는 수집 시도가 끝났다는 뜻입니다.', {exact: false}).isVisible());
        const refresh = page.waitForResponse(r => r.url() === endpoint && r.request().method() === 'GET');
        await page.getByRole('button', {name: '수집 현황 새로고침'}).click();
        check('manual refresh reads API', (await refresh).status() === 200);
        await jobs.getByRole('heading', {name: domain, exact: true}).waitFor(); await snapshot();
        const auto = page.waitForResponse(r => r.url() === endpoint && r.request().method() === 'GET', {timeout: 22000});
        check('automatic refresh reads API', (await auto).status() === 200);
        await snapshot();
        await sites.getByRole('link', {name: '서버 관리에서 확인'}).click();
        await page.waitForURL(base + '/account/moderation/sites/' + data.site_issues.find(i => i.domain === remote).site_id);
        await page.getByRole('heading', {name: remote, exact: true}).waitFor();
        await snapshot();
        check('existing server management reached', await page.getByRole('heading', {name: remote, exact: true}).isVisible());
        await page.locator('.backoffice-nav-rail').getByRole('link', {name: /^작업·유지보수/}).click();
        await page.waitForURL(url); await snapshot();
        await sites.getByRole('heading', {name: remote, exact: true}).waitFor();
        for (const width of [1440, 390, 320]) {
            await page.setViewportSize({width, height: width === 1440 ? 1000 : 844});
            await snapshot();
            check('no horizontal overflow ' + width, await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
            check('long domain visible ' + width, await sites.getByRole('heading', {name: remote, exact: true}).isVisible());
            await page.locator('#content').screenshot({path: 'output/playwright/worker-monitor-' + width + '.png'});
        }
        // A real revoked session must replace a previously rendered private
        // snapshot, not silently retain its last successful value on error.
        const logout = await req.post(base + '/api/member/logout', {headers: {origin: base}, data: {}});
        check('synthetic logout succeeds', logout.status() === 200);
        const revoked = await page.waitForResponse(r => r.url() === endpoint && r.status() === 401, {timeout: 22000});
        check('automatic refresh detects revoked session', revoked.status() === 401);
        await page.getByRole('alert').waitFor(); await snapshot();
        check('revoked session clears prior private data', await jobs.count() === 0 && await sites.count() === 0);
        check('revoked session API still denied', (await req.get(endpoint)).status() === 401);
        check('no runtime exceptions', errors.length === 0);
        return {passed: checks.length, runtimeErrors: errors.length, checks,
            scope: 'Synthetic PG records, real HTTP/SSR/browser; no live remote success or production data.'};
    } finally { await anon.close(); await nojs.close(); }
}
