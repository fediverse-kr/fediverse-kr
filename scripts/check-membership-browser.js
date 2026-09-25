// playwright-cli run-code --filename scripts/check-membership-browser.js
// Load the isolated fixture with state-load .local/browser-state.json first.
// Does not contact, mock, or post to any external ActivityPub server.
async (page) => {
    const base = 'http://127.0.0.1:12239';
    const context = page.context();
    const errors = [];
    const checks = [];
    page.on('pageerror', e => errors.push(e.message));
    const check = (name, pass) => { if (!pass) throw new Error(name); checks.push(name); };
    const request = context.request;
    const post = (path, data, origin = base) => request.post(base + path, { data, headers: { origin } });
    const anonymous = await context.browser().newContext();
    try {
        const anon = await anonymous.request.get(base + '/api/member/session');
        check('anonymous session response is empty and no-store', (await anon.json()).member === null && anon.headers()['cache-control'].includes('no-store'));
        const noAuth = await anonymous.request.post(base + '/api/member/credentials', { data: { login_id: 'not_a_signup', password: 'not-a-real-password-123' }, headers: { origin: base } });
        check('credentials endpoint cannot create anonymous members', noAuth.status() === 401);
        const csrf = await post('/api/member/password/login', { login_id: 'any', password: 'any' }, 'https://evil.example');
        check('cross-origin login is denied', csrf.status() === 403);
        const noOrigin = await anonymous.request.post(base + '/api/member/logout', { data: {} });
        check('missing Origin is denied', noOrigin.status() === 403);
        for (const [path, data] of [
            ['/api/member/name', {name:'anonymous'}],
            ['/api/member/linked/visibility', {account_id:'00000000-0000-0000-0000-000000000000',public:true}],
            ['/api/member/linked/remove', {account_id:'00000000-0000-0000-0000-000000000000'}],
            ['/api/member/password/recover', {new_password:'fixture-only password'}],
            ['/api/member/withdraw', {confirmation:'탈퇴'}],
        ]) {
            const denied = await anonymous.request.post(base + path, {data, headers:{origin:base}});
            check(path + ' requires a session', denied.status() === 401);
            const cross = await post(path, data, 'https://evil.example');
            check(path + ' rejects cross origin before mutation', cross.status() === 403);
        }
        // Renew a nearly-expired browser binding without any remote lookup.
        await context.addCookies([{ name: 'fedkr_browser', value: 'a'.repeat(64), url: base, httpOnly: true, sameSite: 'Lax', expires: Date.now() / 1000 + 5 }]);
        const invalidActor = await post('/api/member/federated/begin', { handle: 'not-an-account' });
        const binding = (await context.cookies()).find(c => c.name === 'fedkr_browser');
        check('a new authentication attempt renews browser binding lifetime', invalidActor.status() === 400 && binding?.expires > Date.now() / 1000 + 3500);
        const actor = await request.get(base + '/actor');
        const actorText = await actor.text();
        check('actor exposes AP public key only', actor.headers()['content-type'].includes('application/activity+json') && JSON.parse(actorText).id === base + '/actor' && actorText.includes('BEGIN PUBLIC KEY') && !actorText.includes('PRIVATE KEY'));

        await page.goto(base + '/account');
        await page.waitForFunction(() => document.querySelector('.portal-root')?.dataset.ready === 'true');
        const initialResponse = await request.get(base + '/api/member/session');
        const initial = await initialResponse.json();
        check('isolated fixture is a federated-only member', initial.member?.display_name === 'Browser test fixture' && initial.member.login_id === null && initial.linked_accounts.length === 1 && !initial.linked_accounts[0].is_public);
        check('last login method cannot be detached in the UI', await page.getByRole('button',{name:'연결 해제',exact:true}).isDisabled());
        const protectedLink = await post('/api/member/linked/remove',{account_id:initial.linked_accounts[0].id});
        check('last login method also protected at API boundary', protectedLink.status() === 400);
        await page.getByLabel('fediverse.kr에서 쓸 이름').fill('표시 이름 검토');
        const renamed = page.waitForResponse(r=>r.url().endsWith('/api/member/name') && r.request().method()==='POST');
        await page.getByRole('button',{name:'이름 저장',exact:true}).click();
        check('name edit uses real persistence', (await renamed).status() === 200 && (await (await request.get(base+'/api/member/session')).json()).member.display_name === '표시 이름 검토');
        await post('/api/member/name',{name:'Browser test fixture'});
        await page.reload();
        await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true');
        const publish = page.waitForResponse(r=>r.url().endsWith('/api/member/linked/visibility') && r.request().method()==='POST');
        await page.getByRole('button',{name:'프로필 표시 허용',exact:true}).click();
        check('per-link display permission persists', (await publish).status() === 200 && (await (await request.get(base+'/api/member/session')).json()).linked_accounts[0].is_public);
        await page.getByRole('button',{name:'비공개로 변경',exact:true}).waitFor();
        const unpublish = page.waitForResponse(r=>r.url().endsWith('/api/member/linked/visibility') && r.request().method()==='POST');
        await page.getByRole('button',{name:'비공개로 변경',exact:true}).click();
        check('link may be made private again', (await unpublish).status() === 200);
        const oldToken = (await context.cookies()).find(c => c.name === 'fedkr_session').value;
        const id = 'browser_' + initial.member.id.replaceAll('-', '').slice(0, 16);
        const password = 'browser fixture original passphrase';
        const replacement = 'browser fixture changed passphrase';
        await page.getByLabel('사용할 fediverse.kr ID', { exact: true }).fill(id);
        await page.locator('#account-new-password').fill(password);
        await page.locator('#account-confirm-password').fill(password);
        const saved = page.waitForResponse(r => r.url().endsWith('/api/member/credentials') && r.request().method() === 'POST');
        await page.getByRole('button', { name: 'ID와 암호 설정', exact: true }).click();
        check('optional credentials save through real UI and API', (await saved).status() === 200);
        await page.getByLabel('현재 암호', { exact: true }).waitFor();
        const savedCookie = (await context.cookies()).find(c => c.name === 'fedkr_session');
        check('credential save rotates HttpOnly SameSite cookie', savedCookie.value !== oldToken && savedCookie.httpOnly && savedCookie.sameSite === 'Lax' && !(await page.evaluate(() => document.cookie)).includes('fedkr_session'));
        const stale = await anonymous.request.get(base + '/api/member/session', { headers: { cookie: 'fedkr_session=' + oldToken } });
        check('previous session is revoked after credential setup', (await stale.json()).member === null);
        const html = await request.get(base + '/account');
        const markup = await html.text();
        check('account SSR is private and contains no credentials', html.headers()['cache-control'].includes('no-store') && markup.includes('Browser test fixture') && !markup.includes(savedCookie.value) && !markup.includes(password));
        await page.reload();
        await page.getByLabel('현재 암호', { exact: true }).waitFor();
        await page.waitForFunction(()=>document.querySelector('.portal-root')?.dataset.ready==='true' && !document.querySelector('#account-current-password')?.disabled);
        check('authenticated session survives reload', (await (await request.get(base + '/api/member/session')).json()).member.login_id === id);
        await page.setViewportSize({ width: 390, height: 844 });
        check('account page has no mobile horizontal overflow', await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
        await page.screenshot({ path: 'output/playwright/membership-account-mobile.png', fullPage: true });
        await page.setViewportSize({ width: 1440, height: 1000 });
        await page.screenshot({ path: 'output/playwright/membership-account-desktop.png', fullPage: true });
        await page.getByRole('button',{name: initial.linked_accounts[0].handle + ' 다시 인증',exact:true}).click();
        check('reauth form targets an existing linked account without remote submission', await page.locator('#reauthenticate-handle').inputValue() === initial.linked_accounts[0].handle && await page.locator('#reauthenticate-handle').getAttribute('readonly') !== null);
        await page.getByText('현재 암호를 잊었나요?',{exact:true}).click();
        check('all labels have unique IDs including recovery fields', await page.evaluate(()=>{const ids=[...document.querySelectorAll('[id]')].map(x=>x.id);return ids.length===new Set(ids).size;}));
        await page.locator('#recovery-new-password').fill(replacement);
        await page.locator('#recovery-confirm-password').fill(replacement);
        const unverifiedRecovery = page.waitForResponse(r=>r.url().endsWith('/api/member/password/recover') && r.request().method()==='POST');
        await page.getByRole('button',{name:'인증한 계정으로 암호 재설정',exact:true}).click();
        check('fixture session is not fresh AP authentication', (await unverifiedRecovery).status() === 400);
        await page.getByLabel('현재 암호', { exact: true }).fill(password);
        await page.locator('#account-new-password').fill(replacement);
        await page.locator('#account-confirm-password').fill(replacement);
        const changed = page.waitForResponse(r => r.url().endsWith('/api/member/password/change') && r.request().method() === 'POST');
        await page.getByRole('button', { name: '암호 변경', exact: true }).click();
        check('password change succeeds through real UI', (await changed).status() === 200);
        const changedCookie = (await context.cookies()).find(c => c.name === 'fedkr_session');
        check('password change rotates session', changedCookie.value !== savedCookie.value);
        const staleChanged = await anonymous.request.get(base + '/api/member/session', { headers: { cookie: 'fedkr_session=' + savedCookie.value } });
        check('pre-change session is revoked', (await staleChanged.json()).member === null);
        const oldPassword = await post('/api/member/password/login', { login_id: id, password });
        check('previous password is rejected', oldPassword.status() === 401);
        await page.getByRole('button', { name: '로그아웃', exact: true }).click();
        await page.waitForURL(base + '/login');
        check('logout removes browser credential and DB session', !(await context.cookies()).some(c => c.name === 'fedkr_session') && (await (await request.get(base + '/api/member/session')).json()).member === null);
        await page.getByText('fediverse.kr ID와 암호가 있나요?', { exact: true }).click();
        await page.getByLabel('fediverse.kr ID', { exact: true }).fill(id);
        await page.getByLabel('암호', { exact: true }).fill(replacement);
        const loggedIn = page.waitForResponse(r => r.url().endsWith('/api/member/password/login') && r.request().method() === 'POST');
        await page.getByRole('button', { name: 'ID로 로그인', exact: true }).click();
        check('registered local credentials sign in through UI', (await loggedIn).status() === 200);
        await page.waitForURL(base + '/account');
        check('local login returns same UUID and private links', (await (await request.get(base + '/api/member/session')).json()).member.id === initial.member.id);
        for (const width of [390,320]) {
            await page.setViewportSize({width,height:844});
            check('management has no overflow at '+width, await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
        }
        await page.getByRole('button',{name:'연결 해제',exact:true}).click();
        await page.getByRole('button',{name:'취소',exact:true}).click();
        check('unlink confirmation may be cancelled', !(await page.getByRole('button',{name:'해제하고 로그아웃',exact:true}).count()));
        await page.getByRole('button',{name:'연결 해제',exact:true}).click();
        const unlinked = page.waitForResponse(r=>r.url().endsWith('/api/member/linked/remove') && r.request().method()==='POST');
        await page.getByRole('button',{name:'해제하고 로그아웃',exact:true}).click();
        check('unlink commits and clears cookies', (await unlinked).status() === 200);
        await page.waitForURL(base + '/login');
        check('unlink logs current browser out', !(await context.cookies()).some(c=>c.name==='fedkr_session'));
        await page.getByText('fediverse.kr ID와 암호가 있나요?',{exact:true}).click();
        await page.getByLabel('fediverse.kr ID',{exact:true}).fill(id);
        await page.getByLabel('암호',{exact:true}).fill(replacement);
        await page.getByRole('button',{name:'ID로 로그인',exact:true}).click();
        await page.waitForURL(base+'/account');
        check('remaining local login still reaches original member without links', (await (await request.get(base+'/api/member/session')).json()).linked_accounts.length===0);
        await page.getByText('fediverse.kr 탈퇴',{exact:true}).click();
        check('withdraw needs explicit confirmation', await page.getByRole('button',{name:'탈퇴하고 로그아웃',exact:true}).isDisabled());
        await page.locator('#withdraw-confirmation').fill('탈퇴');
        const withdrew = page.waitForResponse(r=>r.url().endsWith('/api/member/withdraw') && r.request().method()==='POST');
        await page.getByRole('button',{name:'탈퇴하고 로그아웃',exact:true}).click();
        check('withdraw succeeds through actual form', (await withdrew).status()===200);
        await page.waitForURL(base+'/');
        check('withdraw leaves no authenticated session', (await (await request.get(base+'/api/member/session')).json()).member===null);
        check('withdrawn credentials no longer work', (await post('/api/member/password/login',{login_id:id,password:replacement})).status()===401);
        check('no browser runtime errors', errors.length === 0);
        return { passed: checks.length, checks, pageErrors: errors.length, boundary: 'Fixture-backed local HTTP/UI only. Live ActivityPub ownership is not claimed.' };
    } finally {
        await anonymous.close();
    }
}
