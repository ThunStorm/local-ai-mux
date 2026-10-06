// Design-only browser checks. No production API/Provider is accessed.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(process.env.LOCALMUX_QA_PLAYWRIGHT || 'playwright');
const base = process.env.LOCALMUX_DESIGN_URL || 'http://127.0.0.1:18417/prototype.html';
const output = __dirname;
(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1120, height: 800 }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  page.setDefaultTimeout(5000);
  const errors = [], requests = [], results = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => requests.push({ method: request.method(), url: request.url() }));
  async function check(name, action) {
    try { await action(); results.push({ name, status: 'PASS' }); }
    catch (error) { results.push({ name, status: 'FAIL', reason: error.message }); }
  }
  async function state(value) { await page.locator('#state').selectOption(value); }
  async function screenshot(name) { await page.locator('.frame').screenshot({ path: path.join(output, name) }); }
  const preview = () => page.locator('#preview').textContent().then(JSON.parse);
  await page.goto(base);
  await check('design label and six navigation entries', async () => {
    assert.match(await page.locator('.designbar').textContent(), /不连接 API/);
    assert.equal(await page.locator('[data-nav]').count(), 6);
  });
  await check('wide screenshot has reference app dimensions 1120x760', async () => {
    assert.deepEqual(await page.locator('.frame').evaluate(el => [el.clientWidth, el.clientHeight]), [1120, 760]);
    await screenshot('prototype-chat-light.png');
  });
  await check('default parameters omitted', async () => {
    const value = await preview(); assert.ok(!('temperature' in value)); assert.ok(!('max_tokens' in value)); assert.ok(!('top_p' in value));
  });
  await check('temperature opt-in updates JSON', async () => {
    await page.locator('#temp-on').check(); await page.locator('#temp-number').fill('1.2');
    assert.equal((await preview()).temperature, 1.2);
  });
  await check('output limit opt-in updates JSON', async () => {
    await page.locator('#limit-on').check(); await page.locator('#limit').fill('1024'); assert.equal((await preview()).max_tokens, 1024);
  });
  await check('system role precedes ordered history', async () => {
    await page.locator('#system').fill('只回答中文'); const value = await preview();
    assert.deepEqual(value.messages.map(message => message.role), ['system', 'user', 'assistant']);
  });
  await check('stream toggle and reset restore defaults without sending', async () => {
    await page.locator('#stream').uncheck(); assert.equal((await preview()).stream, false);
    await page.locator('#reset').click(); const value = await preview(); assert.equal(value.stream, true);
    assert.ok(!('temperature' in value)); assert.ok(!('max_tokens' in value)); assert.equal(value.messages[0].content, '只回答中文');
  });
  await check('single-turn model disables unsupported fields and drops prior request history', async () => {
    await page.locator('#model').selectOption('codex/示例模型'); await page.locator('#draft').fill('单轮测试');
    assert.ok(await page.locator('#system').isDisabled()); assert.ok(await page.locator('#temp-on').isDisabled());
    const value = await preview(); assert.deepEqual(value.messages, [{role:'user', content:'单轮测试'}]);
    assert.ok(!('temperature' in value)); assert.match(await page.locator('.messages').textContent(), /旧上下文/); assert.match(await page.locator('.composer-status').textContent(), /不携带历史/);
  });
  await check('simulation send reaches waiting and locks model', async () => {
    await page.locator('#send').click(); assert.equal(await page.locator('#state').inputValue(), 'waiting');
    assert.ok(await page.locator('#model').isDisabled()); assert.equal(await page.locator('#send').textContent(), '停止');
  });
  await check('stop terminates local simulation and restores model', async () => {
    await page.locator('#send').click(); assert.equal(await page.locator('#state').inputValue(), 'stopped');
    assert.ok(!(await page.locator('#model').isDisabled())); assert.match(await page.locator('.notice').textContent(), /上游可能继续执行/);
  });
  await check('stream/error/blocked states remain explicit', async () => {
    await state('streaming'); assert.match(await page.locator('.messages').textContent(), /接收中/);
    await state('error'); assert.match(await page.locator('.error-box').textContent(), /不会自动切换模型/);
    await state('blocked'); assert.ok(await page.locator('#send').isDisabled()); assert.match(await page.locator('.messages').textContent(), /暂无可调用模型/);
  });
  await check('new conversation confirmation clears only design data', async () => {
    await state('ready'); page.once('dialog', dialog => dialog.accept()); await page.locator('#new-chat').click();
    assert.equal(await page.locator('#draft').inputValue(), ''); assert.equal(await page.locator('#state').inputValue(), 'empty');
  });
  await check('apps search and state filter operate on sample rows', async () => {
    await page.locator('[data-nav=apps]').click(); await page.locator('#search').fill('Qoder'); assert.equal(await page.locator('.row').count(), 2);
    await page.locator('[data-filter="待连接"]').click(); assert.equal(await page.locator('.row').count(), 1);
    await page.locator('#search').fill(''); await page.locator('[data-filter="全部"]').click(); await screenshot('prototype-apps-light.png');
  });
  await check('existing top-level pages and preferences all render', async () => {
    for (const id of ['home','api','routing','sessions']) { await page.locator(`[data-nav=${id}]`).click(); assert.ok(await page.locator('h1').isVisible()); }
    await page.locator('#preferences').click(); assert.equal(await page.locator('h1').textContent(), '偏好设置');
  });
  await check('three existing window sizes preserve content and composer', async () => {
    await page.locator('[data-nav=chat]').click(); await page.locator('#model').selectOption('mimocode/示例模型'); await state('ready');
    for (const [width, appHeight] of [[960,680],[800,600],[760,560]]) {
      await page.setViewportSize({width,height:appHeight+40});
      if (await page.locator('.inspector').isVisible()) await page.keyboard.press('Escape');
      const bounds = await page.locator('#send').boundingBox(); assert.ok(bounds.x >= 0 && bounds.x + bounds.width <= width && bounds.y + bounds.height <= appHeight+40);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    }
    await page.reload();
    await screenshot('prototype-chat-narrow-light.png');
  });
  await check('narrow inspector opens and Escape returns focus', async () => {
    await page.locator('#params').click(); assert.ok(await page.locator('#inspector').isVisible()); await page.keyboard.press('Escape');
    assert.ok(!(await page.locator('#inspector').isVisible())); assert.equal(await page.evaluate(()=>document.activeElement.id), 'params');
  });
  await check('dark and opaque fallback preserve navigation and send', async () => {
    await page.locator('#appearance').selectOption('dark'); await page.locator('#solid').check();
    assert.equal(await page.locator('.sidebar').evaluate(el=>getComputedStyle(el).backdropFilter), 'none');
    assert.ok(await page.locator('#send').isVisible()); await screenshot('prototype-chat-narrow-dark.png');
  });
  await check('reduced motion and increased contrast have fallback styles', async () => {
    await page.emulateMedia({reducedMotion:'reduce',contrast:'more'}); await page.locator('#solid').uncheck();
    assert.equal(await page.locator('.sidebar').evaluate(el=>getComputedStyle(el).backdropFilter), 'none');
    assert.equal(await page.locator('#send').evaluate(el=>getComputedStyle(el).transitionDuration), '0s');
  });
  await check('message content rendered as text, never HTML', async () => {
    await state('empty'); await page.locator('#draft').fill('<img src="https://example.invalid/secret" onerror="alert(1)">'); await page.locator('#send').click();
    assert.equal(await page.locator('.messages img').count(), 0); assert.match(await page.locator('.messages').textContent(), /<img/);
  });
  await check('no script errors, no outbound API, no message storage', async () => {
    assert.deepEqual(errors, []);
    assert.ok(requests.every(request => request.method==='GET' && (request.url===base || request.url.endsWith('/favicon.ico'))));
    assert.deepEqual(await page.evaluate(()=>({local:localStorage.length,session:sessionStorage.length})), {local:0,session:0});
    assert.equal((await context.cookies()).length, 0);
  });
  const report = {date:'2026-10-03',kind:'design prototype only',browser:'Playwright Chromium headless shell',checks:results.length,passed:results.filter(r=>r.status==='PASS').length,failed:results.filter(r=>r.status==='FAIL').length,results,errors,requests};
  fs.writeFileSync(path.join(output,'prototype-qa.json'), JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2)); await browser.close(); if(report.failed) process.exitCode=1;
})().catch(error=>{console.error(error); process.exitCode=1});
