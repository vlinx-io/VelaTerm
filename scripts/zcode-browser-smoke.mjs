// 成品内嵌前端 + 真实后端/CLI；浏览器 profile 与账户均由调用方测试夹具隔离。
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';

export async function checkZcodeBrowser({ base, token, sessionName, root, cold = false, startPlanFixture, parityImageSession, parityUsage, onBrowser }) {
  const executable = process.env.VELATERM_TEST_BROWSER;
  if (!executable) return false;
  const reserve = net.createServer();
  await new Promise(resolve => reserve.listen(0, '127.0.0.1', resolve));
  const port = reserve.address().port;
  await new Promise(resolve => reserve.close(resolve));
  const browser = spawn(executable, ['--headless=new', '--disable-gpu', '--no-first-run',
    `--remote-debugging-port=${port}`, `--user-data-dir=${path.join(root, cold ? 'cold-browser-profile' : 'browser-profile')}`, 'about:blank'],
  { windowsHide: true, stdio: 'ignore' });
  onBrowser?.(browser);
  let socket;
  const pending = new Map();
  const exceptions = [];
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
  const until = async (read, accept, label) => {
    const end = Date.now() + 45000;
    while (Date.now() < end) {
      const value = await read();
      if (accept(value)) return value;
      await pause(150);
    }
    throw Error(`Zcode 界面验收超时：${label}`);
  };
  try {
    const target = await until(async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) });
        return (await response.json()).find(page => page.type === 'page' && page.webSocketDebuggerUrl);
      } catch { return null; }
    }, Boolean, '浏览器启动');
    socket = new WebSocket(target.webSocketDebuggerUrl);
    socket.addEventListener('message', event => {
      const frame = JSON.parse(event.data);
      if (frame.method === 'Runtime.exceptionThrown') exceptions.push(frame.params.exceptionDetails.exception?.description ?? frame.params.exceptionDetails.text);
      const request = pending.get(frame.id);
      if (!request) return;
      pending.delete(frame.id);
      if (frame.error) request.reject(Error(frame.error.message)); else request.resolve(frame.result);
    });
    await new Promise((resolve, reject) => {
      socket.addEventListener('open', resolve, { once: true });
      socket.addEventListener('error', reject, { once: true });
    });
    let sequence = 0;
    const send = (method, params = {}) => new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(Error(`浏览器命令超时：${method}`)); }, 20000);
      pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } });
      socket.send(JSON.stringify({ id, method, params }));
    });
    const evaluate = async expression => {
      const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
      if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
      return result.result.value;
    };
    await send('Runtime.enable');
    await send('Page.enable');
    await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
    const checkExtensions = async () => {
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
      await evaluate(`(()=>{const input=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'/plugin-');input.dispatchEvent(new Event('input',{bubbles:true}));input.focus();input.setSelectionRange(8,8);input.dispatchEvent(new Event('select',{bubbles:true}));})()`);
      await until(() => evaluate(`Array.from(document.querySelectorAll('.sv-complete-item')).some(row=>row.textContent.includes('plugin-probe'))`), Boolean, '原生插件技能补全');
      let shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(path.join(root, cold ? 'zcode-cold-skills.png' : 'zcode-live-skills.png'), Buffer.from(shot.data, 'base64'));
      await evaluate(`(()=>{const input=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'');input.dispatchEvent(new Event('input',{bubbles:true}));})()`);
      if (cold) return;
      await evaluate(`(()=>{if(!Array.from(document.querySelectorAll('button.sv-chip')).some(row=>row.textContent.includes('MCP')&&getComputedStyle(row).visibility!=='hidden')){document.querySelector('button.sv-more-toggle')?.click()}})()`);
      await until(() => evaluate(`Array.from(document.querySelectorAll('button.sv-chip')).some(row=>row.textContent.includes('MCP')&&!row.disabled&&getComputedStyle(row).visibility!=='hidden')`), Boolean, 'Zcode MCP 菜单');
      await evaluate(`Array.from(document.querySelectorAll('button.sv-chip')).find(row=>row.textContent.includes('MCP')).click()`);
      await until(() => evaluate(`document.body.innerText.includes('extension-local')&&document.body.innerText.includes('extension-broken')`), Boolean, '显示配置探测与连接失败');
      assert(await evaluate(`/separate CLI|独立 CLI/.test(document.body.innerText)`), '成品应明确独立探测语义');
      shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(path.join(root, 'zcode-mcp-menu.png'), Buffer.from(shot.data, 'base64'));
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
    };
    await send('Page.addScriptToEvaluateOnNewDocument', { source: `if(location.origin===${JSON.stringify(base)})sessionStorage.setItem('vlx-token',${JSON.stringify(token)});` });
    const selectSession = async (targetName=sessionName) => {
      await until(() => evaluate(`Array.from(document.querySelectorAll('.nm')).some(n=>n.textContent===${JSON.stringify(targetName)})`), Boolean, '独立 Zcode 会话树节点');
      await evaluate(`Array.from(document.querySelectorAll('.nm')).find(n=>n.textContent===${JSON.stringify(targetName)}).click()`);
      await until(() => evaluate(`Boolean(document.querySelector('.sv-box textarea'))&&(${cold}||Array.from(document.querySelectorAll('.sv-chip-label')).some(n=>n.textContent.includes('gpt-4o')))`), Boolean, '模型目录和聊天输入框');
    };
    await send('Page.navigate', { url: base + '/?view=desktop' });
    await selectSession();
    if (parityImageSession) {
      await until(()=>evaluate(`Array.from(document.querySelectorAll('.sv-tool-head')).some(e=>e.textContent.includes('Parity native child')&&e.textContent.includes('local-fixture/gpt-4o'))`),Boolean,'原生子任务卡片和模型');
      await evaluate(`Array.from(document.querySelectorAll('.sv-tool-head')).find(e=>e.textContent.includes('Parity native child')).click()`);
      await until(()=>evaluate(`document.querySelector('.sv-subagent')?.innerText.includes('PARITY_CHILD_OK')&&document.querySelector('.sv-subagent')?.innerText.includes('Read')`),Boolean,'展开子任务过程');
      let shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(path.join(root,'zcode-native-subagent.png'),Buffer.from(shot.data,'base64'));
      await selectSession(parityImageSession);
      await until(()=>evaluate(`Array.from(document.querySelectorAll('.sv-msg-image')).some(e=>e.tagName==='IMG'&&e.complete&&e.naturalWidth>0)`),Boolean,'恢复后的图片可显示');
      await until(()=>evaluate(`Array.from(document.querySelectorAll('button.sv-chip')).some(e=>/Usage|用量/.test(e.textContent))`),Boolean,'原生用量入口');
      await evaluate(`Array.from(document.querySelectorAll('button.sv-chip')).find(e=>/Usage|用量/.test(e.textContent)).click()`);
      await until(()=>evaluate(`/Session total|会话累计/.test(document.body.innerText)&&Array.from(document.querySelectorAll('.sv-popover section')).some(e=>/Session total|会话累计/.test(e.textContent)&&e.textContent.includes('${parityUsage?.input??120}')&&e.textContent.includes('${parityUsage?.cached??30}'))`),Boolean,'恢复后的累计用量');
      const usageVisible = () => evaluate(`(()=>{const p=document.querySelector('.sv-popover');const r=p.getBoundingClientRect(),c=p.closest('.sv').getBoundingClientRect();return r.left>=c.left&&r.right<=c.right&&r.top>=0&&r.bottom<=innerHeight&&Array.from(p.querySelectorAll('dd')).every(e=>{const d=e.getBoundingClientRect();return d.left>=r.left&&d.right<=r.right})})()`);
      assert(await usageVisible(),'用量弹窗及数字必须处于聊天区域可见范围');
      shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(path.join(root,'zcode-images-usage.png'),Buffer.from(shot.data,'base64'));
      await send('Emulation.setDeviceMetricsOverride',{width:1100,height:800,deviceScaleFactor:1,mobile:false});
      await until(usageVisible,Boolean,'较小窗口用量弹窗仍可读');
      shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(path.join(root,'zcode-images-usage-narrow.png'),Buffer.from(shot.data,'base64'));
      await send('Input.dispatchKeyEvent',{type:'keyDown',key:'Escape',code:'Escape',windowsVirtualKeyCode:27});
      await evaluate(`(()=>{const e=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(e,'STALL');e.dispatchEvent(new Event('input',{bubbles:true}));e.focus()})()`);
      await send('Input.dispatchKeyEvent',{type:'keyDown',key:'Enter',code:'Enter',windowsVirtualKeyCode:13});
      await until(()=>evaluate(`Boolean(document.querySelector('.sv-steer'))`),Boolean,'执行中图片引导入口');
      await evaluate(`(()=>{const e=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(e,'Keep guide draft');e.dispatchEvent(new Event('input',{bubbles:true}));const d=new DataTransfer();d.items.add(new File([new Uint8Array([137,80,78,71])],'guide.png',{type:'image/png'}));document.querySelector('.sv-composer').dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:d}))})()`);
      await until(()=>evaluate(`Boolean(document.querySelector('.sv-attach img'))`),Boolean,'图片已加入草稿');
      await evaluate(`document.querySelector('.sv-steer').click()`);
      await until(()=>evaluate(`document.querySelector('.sv-box textarea').value==='Keep guide draft'&&Boolean(document.querySelector('.sv-attach img'))&&/queue|队列/.test(document.querySelector('.sv-attach-note')?.textContent??'')`),Boolean,'拒绝带图引导后草稿与图片仍可普通发送');
      assert.deepEqual(exceptions,[]);
      await writeFile(path.join(root,'parity-browser-result.json'),JSON.stringify({nativeChildCard:true,childModel:true,expandChild:true,restoredImage:true,restoredUsage:true,usageVisible:true,narrowUsageVisible:true,imageGuideDraftPreserved:true,uncaughtExceptions:exceptions},null,2));
      return true;
    }
    await evaluate(`(${cold}?document.querySelector('button.sv-chip'):Array.from(document.querySelectorAll('button.sv-chip')).find(n=>n.textContent.includes('gpt-4o'))).click()`);
    await until(() => evaluate(`Array.from(document.querySelectorAll('.sv-chip-box [role="option"]')).some(row=>row.textContent.includes('gpt-4o'))`), Boolean, '展开模型菜单并显示原生模型');
    assert(await evaluate(`Array.from(document.querySelectorAll('.sv-chip-box [role="option"]')).some(row=>row.textContent.includes('gpt-4o')&&row.textContent.includes('Local fixture'))`), '模型菜单应显示原生供应商名称');
    if (startPlanFixture) {
      await until(() => evaluate(`document.querySelector('[aria-label="Start Plan"]')?.textContent.includes('ZCode Global Build')`), Boolean, 'Start Plan 套餐名称');
      assert(await evaluate(`/Pending|待生效/.test(document.querySelector('[aria-label="Start Plan"]').textContent)`));
      let screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(path.join(root, 'start-plan-pending.png'), Buffer.from(screenshot.data, 'base64'));
      startPlanFixture.activate();
      await evaluate(`document.querySelector('[aria-label="Start Plan"] button').click()`);
      await until(() => evaluate(`document.querySelector('[aria-label="Start Plan"]')?.textContent.includes('80')`), Boolean, '刷新后显示官方原始额度');
      await until(() => evaluate(`Array.from(document.querySelectorAll('[role="option"]')).some(row=>row.textContent.includes('GLM-5.3-Flash')&&row.textContent.includes('Start Plan'))`), Boolean, '刷新后开放套餐模型');
      screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(path.join(root, 'start-plan-active.png'), Buffer.from(screenshot.data, 'base64'));
      await evaluate(`Array.from(document.querySelectorAll('[role="option"]')).find(row=>row.textContent.includes('GLM-5.3-Flash')&&row.textContent.includes('Start Plan')).click()`);
      await evaluate(`(()=>{const input=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'START_PLAN_UI');input.dispatchEvent(new Event('input',{bubbles:true}));input.focus();})()`);
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
      await until(() => evaluate(`document.body.innerText.includes('START_PLAN_OK')`), Boolean, '通过 UI 使用套餐 JWT 完成回合');
      assert.deepEqual(exceptions, []);
      return true;
    }
    if (cold) {
      assert(!await evaluate(`document.body.innerText.includes('UI_RENDER_OK')`), '空白菜单验收必须发生在首次发送之前');
      const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(path.join(root, 'zcode-cold-model-menu.png'), Buffer.from(screenshot.data, 'base64'));
      await checkExtensions();
      assert.deepEqual(exceptions, []);
      await writeFile(path.join(root, 'cold-browser-result.json'), JSON.stringify({ modelMenu: true, providerHint: true, beforeFirstStart: true, uncaughtExceptions: exceptions, embeddedFrontend: true }, null, 2));
      return true;
    }
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
    await checkExtensions();
    await evaluate(`(()=>{const input=document.querySelector('.sv-box textarea');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'UI_RENDER_TEST');input.dispatchEvent(new Event('input',{bubbles:true}));input.focus();})()`);
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
    await until(() => evaluate(`document.body.innerText.includes('UI_RENDER_OK')`), Boolean, '从界面发送并显示模型回复');
    await send('Page.reload');
    await selectSession();
    await until(() => evaluate(`document.body.innerText.includes('UI_RENDER_OK')`), Boolean, '重新加载后恢复会话');
    assert.deepEqual(exceptions, [], '前端产生了未捕获异常');
    assert(!await evaluate(`document.body.innerText.includes('界面渲染出错')||document.body.innerText.includes('Cannot read properties')`), '前端错误边界被触发');
    const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    await writeFile(path.join(root, 'zcode-render-smoke.png'), Buffer.from(screenshot.data, 'base64'));
    await writeFile(path.join(root, 'browser-result.json'), JSON.stringify({ modelMenu: true, composerSend: true, reply: 'UI_RENDER_OK', reloadResume: true, uncaughtExceptions: exceptions, embeddedFrontend: true }, null, 2));
    return true;
  } finally {
    if(socket?.readyState===WebSocket.OPEN) socket.send(JSON.stringify({id:2147483647,method:'Browser.close'}));
    for (const request of pending.values()) request.reject(Error('测试浏览器已关闭'));
    socket?.close();
    if (browser.exitCode === null && browser.pid) {
      await new Promise(resolve=>{const timer=setTimeout(resolve,2000);browser.once('exit',()=>{clearTimeout(timer);resolve()})});
      if(browser.exitCode===null) browser.kill('SIGKILL');
    }
  }
}
