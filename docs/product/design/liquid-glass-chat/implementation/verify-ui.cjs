// Exercises the production React source with an isolated offline Tauri IPC fixture.
// It does not exercise native IPC, and never connects to a real Provider.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(process.env.LOCALMUX_QA_PLAYWRIGHT || 'playwright');
const output = __dirname;
(async()=>{
 const browser=await chromium.launch({headless:true});
 const context=await browser.newContext({viewport:{width:1120,height:760},deviceScaleFactor:1});
 await context.addInitScript(()=>{
   localStorage.setItem('localmux.seen','yes');localStorage.setItem('localmux.theme','light');
   const callbacks=new Map();let callbackId=0;let run=null;
   const cap=(min,max)=>({support:'SUPPORTED',min,max});const unknown={support:'UNKNOWN',min:0,max:null};
   const models=[{id:'mimocode/offline-sample',provider:'mimocode',stream:true,chat:{history:'SUPPORTED',system:'SUPPORTED',temperature:cap(0,2),max_tokens:cap(1,4096),top_p:cap(0,1),source:'离线验收夹具 · 非真实上游'}},{id:'codex/offline-sample',provider:'codex',stream:true,chat:{history:'UNSUPPORTED',system:'UNSUPPORTED',temperature:unknown,max_tokens:unknown,top_p:unknown,source:'离线单轮夹具 · 非真实上游'}}];
   window.__fixture={requests:[],commands:[],mode:'complete'};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={
     transformCallback(callback){const id=++callbackId;callbacks.set(id,callback);return id},
     unregisterCallback(id){callbacks.delete(id)},
     async invoke(cmd,args={}){
       window.__fixture.commands.push(cmd);
       if(cmd==='scan_apps')return {applications:[['codex','Codex'],['qoder','Qoder'],['qoder-cn','Qoder CN'],['mimo','MiMo Code']].map(([app_id,name])=>({app_id,name,version:'fixture',executable:'/offline/'+app_id,install_path:'/offline/'+name+'.app',detection_source:'executable',connectability:'SUPPORTED_VERSION_UNKNOWN',diagnostics:['离线 UI 夹具']})),scanned_roots:[]};
       if(cmd==='gateway_status')return true;
       if(cmd==='gateway_base_url')return 'http://127.0.0.1:8317/v1';
       if(cmd==='connected_apps')return [['codex','/offline/codex'],['qoder','/offline/qoder']];
       if(['added_apps','routing_status','quota_snapshots','quota_buckets','sessions_list','list_api_keys'].includes(cmd))return [];
       if(cmd==='routing_get')return {allow_reserve_in_free:false,allow_paid_in_balanced:false,best_model:null,balanced_priority:[]};
       if(['autostart_status','close_to_tray_status','updater_available','default_api_key_allows_agent'].includes(cmd))return false;
       if(cmd==='native_appearance')return {nativeGlass:false,reduceTransparency:false,increaseContrast:false,reduceMotion:false};
       if(cmd==='native_theme'||cmd==='plugin:event|unlisten')return;
       if(cmd==='plugin:event|listen')return ++callbackId;
       if(cmd==='default_api_key_status')return 'READY';
       if(cmd==='api_setup_models')return models.map(model=>model.id);
       if(cmd==='cli_auth_status')return 'READY';
       if(cmd==='chat_models')return {data:models};
       if(cmd==='chat_send'){
         window.__fixture.requests.push(JSON.parse(JSON.stringify(args.input.request)));
         const own={id:args.input.requestId,index:0,emit:null};
         const emit=(kind,text='',errorCode='')=>{if(run!==own)return;const index=own.index++;callbacks.get(args.events.id)?.({index,message:{requestId:args.input.requestId,sequence:index+1,kind,model:args.input.request.model,text,errorCode,elapsedMs:(index+1)*20}})};
         own.emit=emit;run=own;
         const mode=window.__fixture.mode;
         // Deliberately deliver Channel events after invoke resolves: the UI must
         // keep the request alive until its terminal Channel event.
         setTimeout(()=>{emit('started');emit('delta','这是离线夹具返回的示例回答。');if(mode==='complete'){emit('completed');run=null}else if(mode==='error'){emit('failed','','LMX_PROVIDER_UNAVAILABLE');run=null}},30);
         return;
       }
       if(cmd==='chat_cancel'){if(run?.id===args.requestId){run.emit('stopped');run=null}return;}
       throw new Error('Offline fixture blocks command: '+cmd);
     }
   };
 });
 const page=await context.newPage();page.setDefaultTimeout(7000);
 const errors=[],checks=[],requests=[];
 page.on('pageerror',error=>errors.push(error.stack || error.message));page.on('request',r=>requests.push(r.url()));
 async function check(name,action){try{await action();checks.push({name,status:'PASS'})}catch(error){checks.push({name,status:'FAIL',reason:error.message})}}
 async function nav(name){await page.getByRole('button',{name,exact:true}).first().click()}
 async function shot(name){await page.screenshot({path:path.join(output,name),animations:'disabled'})}
 await page.goto('http://127.0.0.1:18418/');
 await check('production app identity, meaningful screen, no framework overlay',async()=>{await page.waitForFunction(()=>document.title==='LocalAiMux');await page.getByRole('heading',{name:'总览',exact:true}).waitFor();assert.equal(await page.title(),'LocalAiMux');assert.ok(await page.getByRole('heading',{name:'总览',exact:true}).isVisible());assert.equal(await page.locator('vite-error-overlay').count(),0)});
 await check('original application icons load and app search filters',async()=>{
   await nav('应用与连接');await page.getByRole('listitem').first().waitFor();
   assert.equal(await page.locator('.inventory-row .software-icon').count(),4);
   assert.ok(await page.locator('.software-icon').evaluateAll(images=>images.every(img=>img.complete&&img.naturalWidth>0)));
   await shot('ui-apps-light-fixture.png');await page.getByRole('searchbox').fill('Qoder');assert.equal(await page.locator('.inventory-row').count(),2);await page.getByRole('searchbox').fill('');
 });
 await check('model selection, opt-in parameters and actual submitted payload',async()=>{
   await nav('试对话');await page.getByLabel('对话模型',{exact:true}).selectOption('mimocode/offline-sample');
   await page.getByRole('button',{name:'参数',exact:true}).click();
   await page.getByRole('checkbox',{name:'采样温度',exact:true}).check();await page.getByRole('spinbutton',{name:'采样温度',exact:true}).fill('0.9');
   await page.getByRole('checkbox',{name:'最大输出长度',exact:true}).check();await page.getByRole('spinbutton',{name:'最大输出长度',exact:true}).fill('1024');
   await page.getByLabel('系统提示词',{exact:true}).fill('只回答中文');await page.getByLabel('消息输入',{exact:true}).fill('请用一句话介绍你自己。');
   await page.getByRole('button',{name:'发送',exact:true}).click();await page.locator('.chat-receipt').filter({hasText:'完成'}).first().waitFor();
   const payload=await page.evaluate(()=>window.__fixture.requests.at(-1));assert.equal(payload.temperature,.9);assert.equal(payload.max_tokens,1024);assert.equal(payload.messages[0].role,'system');assert.ok(!('top_p' in payload));
   await shot('ui-chat-light-fixture.png');
 });
 await check('successful multi-turn history preserved and no premature finish after invoke',async()=>{
   await page.getByLabel('消息输入',{exact:true}).fill('再解释一下。');await page.getByRole('button',{name:'发送',exact:true}).click();await page.locator('.chat-receipt').filter({hasText:'完成'}).nth(1).waitFor();
   const payload=await page.evaluate(()=>window.__fixture.requests.at(-1));assert.deepEqual(payload.messages.map(m=>m.role),['system','user','assistant','user']);
 });
 await check('sampling controls are mutually exclusive and reset omits fields',async()=>{
   await page.getByRole('checkbox',{name:'Top P',exact:true}).check();assert.ok(!(await page.getByRole('checkbox',{name:'采样温度',exact:true}).isChecked()));await page.getByRole('button',{name:'恢复默认',exact:true}).click();assert.ok(!(await page.getByRole('checkbox',{name:'Top P',exact:true}).isChecked()));assert.ok(!(await page.getByRole('checkbox',{name:'最大输出长度',exact:true}).isChecked()));
 });
 await check('switching model archives context and disables unsupported parameters',async()=>{
   await page.getByLabel('对话模型',{exact:true}).selectOption('codex/offline-sample');
   assert.ok(await page.locator('.chat-archive').isVisible());assert.ok(await page.getByRole('checkbox',{name:'采样温度',exact:true}).isDisabled());assert.ok(await page.getByLabel('系统提示词',{exact:true}).isDisabled());
   await page.getByLabel('消息输入',{exact:true}).fill('仅这一条');await page.getByRole('button',{name:'发送',exact:true}).click();await page.locator('.chat-receipt').filter({hasText:'完成'}).last().waitFor();
   const payload=await page.evaluate(()=>window.__fixture.requests.at(-1));assert.deepEqual(payload.messages,[{role:'user',content:'仅这一条'}]);assert.ok(!('temperature' in payload));
 });
 await check('navigation preserves draft and in-memory transcript',async()=>{
   await page.getByLabel('消息输入',{exact:true}).fill('未发送草稿');await nav('总览');await nav('试对话');assert.equal(await page.getByLabel('消息输入',{exact:true}).inputValue(),'未发送草稿');assert.ok(await page.locator('.chat-archive').isVisible());
 });
 await check('stop remains reachable and restores controls without completing partial text',async()=>{
   await page.evaluate(()=>window.__fixture.mode='pending');await page.getByRole('button',{name:'发送',exact:true}).click();await page.getByRole('button',{name:'停止',exact:true}).waitFor();assert.ok(await page.getByLabel('对话模型',{exact:true}).isDisabled());await page.getByRole('button',{name:'停止',exact:true}).click();await page.locator('.chat-warning').waitFor();assert.ok(!(await page.getByLabel('对话模型',{exact:true}).isDisabled()));
 });
 await check('errors preserve draft with no retry or model fallback',async()=>{
   await page.evaluate(()=>window.__fixture.mode='error');await page.getByLabel('消息输入',{exact:true}).fill('保留失败草稿');await page.getByRole('button',{name:'发送',exact:true}).click();await page.locator('.chat-error').last().waitFor();assert.equal(await page.getByLabel('消息输入',{exact:true}).inputValue(),'保留失败草稿');assert.equal(await page.getByLabel('对话模型',{exact:true}).inputValue(),'codex/offline-sample');
 });
 await check('all desktop widths preserve main controls and prevent horizontal overflow',async()=>{
   await page.getByRole('button',{name:'关闭参数',exact:true}).click();
   for(const [width,height] of [[1120,760],[960,680],[800,600],[760,560]]){
     await page.setViewportSize({width,height});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));const bounds=await page.getByRole('button',{name:'发送',exact:true}).boundingBox();assert.ok(bounds.x+bounds.width<=width&&bounds.y+bounds.height<=height);
   }
   await shot('ui-chat-narrow-light-fixture.png');
 });
 await check('default 960 sidebar labels and manual expand work at narrow width',async()=>{
   await page.setViewportSize({width:960,height:680});await page.getByRole('button',{name:'收起侧栏',exact:true}).waitFor();assert.ok(await page.locator('.sidebar-brand strong').isVisible());await page.getByRole('button',{name:'收起侧栏',exact:true}).click();await page.getByRole('button',{name:'展开侧栏',exact:true}).waitFor();assert.ok(!(await page.locator('.sidebar-brand strong').isVisible()));await page.getByRole('button',{name:'展开侧栏',exact:true}).click();assert.ok(await page.locator('.sidebar-brand strong').isVisible());
   await page.setViewportSize({width:760,height:560});await page.getByRole('button',{name:'展开侧栏',exact:true}).waitFor();await page.getByRole('button',{name:'展开侧栏',exact:true}).click();assert.ok(await page.locator('.sidebar-brand strong').isVisible());assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.getByRole('button',{name:'收起侧栏',exact:true}).click();
 });
 await check('760 app list keeps three rows identifiable without a blank action column',async()=>{
   await nav('应用与连接');await page.locator('.inventory-row').first().waitFor();for(const row of await page.locator('.inventory-row h2').all()){const box=await row.boundingBox();assert.ok(box.x+box.width<=760)}
   const third=await page.locator('.inventory-row h2').nth(2).boundingBox();assert.ok(third.y+third.height<=560);await shot('ui-apps-narrow-light-fixture.png');await nav('试对话');
 });
 await check('narrow inspector cannot cover stop and Escape returns focus',async()=>{
   await page.getByRole('button',{name:'参数',exact:true}).click();const panel=await page.locator('.chat-inspector').boundingBox();const send=await page.getByRole('button',{name:'发送',exact:true}).boundingBox();assert.ok(panel.y+panel.height<send.y);await page.keyboard.press('Escape');assert.ok(!(await page.locator('.chat-inspector').isVisible()));assert.equal(await page.evaluate(()=>document.activeElement.textContent),'参数');
 });
 await check('dark, opaque fallback and keyboard focus render',async()=>{
   await page.locator('summary[aria-label="偏好设置"]').click();await page.getByLabel('外观',{exact:true}).selectOption('dark');await page.getByRole('checkbox',{name:'减少透明度',exact:true}).check();await page.keyboard.press('Escape');await page.getByLabel('消息输入',{exact:true}).focus();assert.equal(await page.locator('.sidebar').evaluate(el=>getComputedStyle(el).backdropFilter),'none');await shot('ui-chat-narrow-dark-fixture.png');
 });
 await check('message body is plain text, never HTML; no message storage or implicit auth',async()=>{
   await page.evaluate(()=>window.__fixture.mode='complete');await page.getByLabel('消息输入',{exact:true}).fill('<img src="https://example.invalid/private">');await page.getByRole('button',{name:'发送',exact:true}).click();await page.locator('.chat-receipt').filter({hasText:'完成'}).last().waitFor();assert.equal(await page.locator('.chat-user img').count(),0);const storage=await page.evaluate(()=>JSON.stringify(localStorage));assert.ok(!storage.includes('保留失败草稿'));assert.ok(!storage.includes('private'));const commands=await page.evaluate(()=>window.__fixture.commands);assert.ok(!commands.includes('reset_default_api_key'));assert.ok(!commands.includes('reveal_default_api_key'));assert.ok(!commands.includes('connect_app'));
 });
 await check('console health and no upstream network requests',async()=>{assert.deepEqual(errors,[]);assert.ok(requests.every(url=>url.startsWith('http://127.0.0.1:18418/')||url.startsWith('ws://127.0.0.1:18418/')))});
 const report={date:new Intl.DateTimeFormat('sv-SE',{timeZone:'Asia/Shanghai',dateStyle:'short'}).format(new Date()),kind:'production React UI with offline IPC fixture; native IPC and live Provider not tested here',browser:'Browser plugin absent; isolated Playwright Chromium',checks:checks.length,passed:checks.filter(c=>c.status==='PASS').length,failed:checks.filter(c=>c.status==='FAIL').length,results:checks,errors};
 fs.writeFileSync(path.join(output,'ui-qa.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));await browser.close();if(report.failed)process.exitCode=1;
})().catch(error=>{console.error(error);process.exitCode=1});
