// Windows 成品回归：最终 EXE 的真实后端 + 真实 ZCode CLI + 隔离的本地模型服务。
// 用法：VELATERM_TEST_EXE、ZCODE_TEST_BIN 指向已构建程序；需要 Node 22+。
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { checkZcodeBrowser as inspectZcodeBrowser } from './zcode-browser-smoke.mjs';

if (process.platform !== 'win32') throw Error('此回归脚本验证 Windows 成品；协议单测支持其他平台。');
const exe = process.env.VELATERM_TEST_EXE;
const bin = process.env.ZCODE_TEST_BIN;
const expectedVersion = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8')).version;
if (!exe || !bin) throw Error('请设置 VELATERM_TEST_EXE 和 ZCODE_TEST_BIN。');
const root = await mkdtemp(path.join(tmpdir(), 'vela-zcode-e2e-'));
console.log(JSON.stringify({ fixtureRoot: root }));
const workspace = path.join(root, 'workspace');
const nativeRoot = path.join(root, 'native');
const profile = path.join(root, 'profile');
await Promise.all([mkdir(workspace), mkdir(profile), mkdir(path.join(nativeRoot, '.zcode/v2'), { recursive: true })]);
// 真实发现器：用户、工作区和插件技能，以及本地与插件 MCP。
const pluginRoot = path.join(root, 'extension-plugin');
for (const [dir, name] of [
  [path.join(profile, '.agents/skills/user-probe'), 'user-probe'],
  [path.join(workspace, '.agents/skills/workspace-probe'), 'workspace-probe'],
  [path.join(pluginRoot, 'skills/plugin-probe'), 'plugin-probe'],
]) {
  await mkdir(dir, { recursive: true });
  await writeFile(path.join(dir, 'SKILL.md'), `---\nname: ${name}\ndescription: Read-only extension regression probe\n---\nEXTENSION_SKILL_CONTENT_OK\n`);
}
await mkdir(path.join(pluginRoot, '.zcode-plugin'), { recursive: true });
await writeFile(path.join(pluginRoot, '.zcode-plugin/plugin.json'), JSON.stringify({ name: 'extension-fixture', version: '1.0.0' }));
const mcpScript = path.join(root, 'fixture-mcp.cjs');
const slowMcpFlag = path.join(root, 'slow-mcp.flag');
const slowMcpStarted = path.join(root, 'slow-mcp.started');
await writeFile(mcpScript, `const readline=require('readline'),fs=require('fs');
readline.createInterface({input:process.stdin}).on('line',line=>{const q=JSON.parse(line);if(q.id===undefined)return;
const result=q.method==='initialize'?{protocolVersion:'2024-11-05',capabilities:{tools:{}},serverInfo:{name:'extension-fixture',version:'1'}}:
q.method==='tools/list'?{tools:[{name:'echo',description:'Read-only regression probe',inputSchema:{type:'object',properties:{},additionalProperties:false}}]}:
q.method==='tools/call'?{content:[{type:'text',text:process.argv[2]==='plugin'?'EXTENSION_PLUGIN_MCP_CONTENT_OK':'EXTENSION_MCP_CONTENT_OK'}]}:{};
const send=()=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:q.id,result})+'\\n');
if(fs.existsSync(${JSON.stringify(slowMcpFlag)})&&q.method!=='tools/call'){fs.writeFileSync(${JSON.stringify(slowMcpStarted)},'started');setTimeout(send,8000)}else send();});`);
const mcpConfig = { type: 'stdio', command: process.execPath, args: [mcpScript] };
await writeFile(path.join(pluginRoot, '.mcp.json'), JSON.stringify({ mcpServers: { tools: { ...mcpConfig, args: [mcpScript, 'plugin'] } } }));
await mkdir(path.join(workspace, '.zcode'), { recursive: true });
await writeFile(path.join(workspace, '.zcode/config.json'), JSON.stringify({ plugins: { dirs: [pluginRoot] }, mcp: { servers: {
  'extension-local': mcpConfig,
  'extension-disabled': { ...mcpConfig, enabled: false },
  'extension-broken': { type: 'stdio', command: path.join(root, 'missing-mcp.exe'), args: [] },
} } }));
// Windows 会话会重建系统环境。仅在测试 wrapper 中指定 profile，避免加载个人 hooks。
const wrapper = path.join(root, 'fixture-zcode.cmd');
const environmentLog = path.join(root, 'launch-profiles.txt');
// 0.2.10 还会单独执行 --version；该探测不读取历史，不属于 app-server 环境比较。
await writeFile(wrapper, '@echo off\r\nif /I "%~1"=="app-server" >> "%VELA_ZCODE_ENV_LOG%" echo("%USERPROFILE%"\r\nset "USERPROFILE=%VELA_ZCODE_TEST_PROFILE%"\r\n"%VELA_ZCODE_TEST_BINARY%" %*\r\n');
let generationRequests = 0;
let startPlanPhase = 'pending';
let startPlanRequests = 0;
let startPlanRemaining = 80;
let unexpectedAccountRequests = 0;
const modelServer = createServer(async (request, response) => {
  if (request.url.startsWith('/api/v1/oauth/cli/')) {
    for await (const _chunk of request) { /* 仅消费本地假登录请求。 */ }
    response.writeHead(200, { 'Content-Type': 'application/json' });
    response.end(JSON.stringify({ code: 0, data: request.url.includes('/init')
      ? { flow_id: 'start-fixture', poll_token: 'fixture-poll', authorize_url: 'https://example.test/fixture-login', expires_at: Math.floor(Date.now() / 1000) + 120, poll_interval_sec: 1 }
      : { status: 'ready', token: 'fixture-start-jwt', user: { user_id: 'fixture-user' }, bigmodel: { access_token: 'fixture-oauth' } } })); return;
  }
  if (request.url.startsWith('/api/v1/zcode-plan/billing/balance')) {
    assert.equal(request.headers.authorization, 'Bearer fixture-start-jwt');
    assert(request.headers['x-device-mid']);
    assert.equal(request.headers['x-title'], 'Z Code@cli');
    assert(request.url.includes('app_version=3.15.1'), '必须使用真实产品版本，而不是未知开发版本');
    const now = Math.floor(Date.now() / 1000);
    response.writeHead(200, { 'Content-Type': 'application/json', Date: new Date().toUTCString() });
    response.end(JSON.stringify({ code: 0, data: { server_time: now,
      plans: [{ user_plan_id: 'fixture-plan', name: 'ZCode Global Build', status: 'active', ends_at: now + 7200,
        entitlements: [{ entitlement_id: 'fixture-entitlement', effective_at: startPlanPhase === 'pending' ? now + 3600 : now - 3600 }] }],
      balances: startPlanPhase === 'pending' ? [] : [{ user_plan_id: 'fixture-plan', entitlement_id: 'fixture-entitlement',
        show_name: 'GLM-5.3-Flash', capabilities: ['model:GLM-5.3-Flash'], total_units: 100,
        used_units: 100 - startPlanRemaining, available_units: startPlanRemaining, unit_type: 'credits' }],
    } })); return;
  }
  if (request.url.startsWith('/api/v1/zcode-plan/anthropic/')) {
    startPlanRequests++;
    const authorized = request.headers.authorization === 'Bearer fixture-start-jwt' || request.headers['x-api-key'] === 'fixture-start-jwt';
    assert(authorized, '模型请求必须使用 Start Plan JWT');
    for await (const _chunk of request) { /* 消费请求体，禁止记录可能含敏感信息的正文。 */ }
    startPlanRemaining -= 5;
    response.writeHead(200, { 'Content-Type': 'text/event-stream' });
    const event = (type, value) => response.write(`event: ${type}\ndata: ${JSON.stringify({ type, ...value })}\n\n`);
    event('message_start', { message: { id: 'start-plan-reply', type: 'message', role: 'assistant', model: 'GLM-5.3-Flash', content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 1, output_tokens: 0 } } });
    event('content_block_start', { index: 0, content_block: { type: 'text', text: '' } });
    event('content_block_delta', { index: 0, delta: { type: 'text_delta', text: 'START_PLAN_OK' } });
    event('content_block_stop', { index: 0 });
    event('message_delta', { delta: { stop_reason: 'end_turn', stop_sequence: null }, usage: { output_tokens: 4 } });
    event('message_stop', {}); response.end(); return;
  }
  if (!request.url.startsWith('/v1/')) {
    if (request.url.includes('token') || request.url.includes('apikey')) unexpectedAccountRequests++;
    response.writeHead(404); response.end(); return;
  }
  generationRequests++;
  let raw = '';
  for await (const chunk of request) raw += chunk;
  const body = JSON.parse(raw);
  // 原生 CLI 会插入合成 user 消息；按最后一个测试指令选分支，避免历史工具请求污染新回合。
  const text = body.messages.filter(message => message.role === 'user').map(message => JSON.stringify(message.content))
    .findLast(text => /PARITY_(?:IMAGE|STEER_ROOT|NO_TOOL|GUIDE|AGENT_ROOT|AGENT_BACKGROUND|CHILD)|UI_RENDER_TEST|ALPHA|BETA|WRITE|DENIAL|PLAN_REVIEW|EXTENSION_(?:PLUGIN_MCP|MCP|SKILL)|STALL/.test(text)) ?? '';
  if (text.includes('PARITY_IMAGE')) {
    const urls=body.messages.filter(message=>message.role==='user').flatMap(message=>Array.isArray(message.content)?message.content:[])
      .filter(part=>part.type==='image_url').map(part=>part.image_url.url);
    assert(urls.includes('data:image/png;base64,'+parityImage), '原生模型请求必须收到原始图片字节: '+JSON.stringify(body.messages.filter(m=>m.role==='user').map(m=>typeof m.content==='string'?'text':m.content.map(p=>({type:p.type,url:p.image_url?.url?.slice(0,120),text:p.text?.slice(0,350)})))));
  }
  response.writeHead(200, { 'Content-Type': 'text/event-stream' });
  if (text.includes('STALL')) { response.write(': waiting\n\n'); return; }
  const base = { id: 'chatcmpl-fixture', object: 'chat.completion.chunk', created: 1, model: 'gpt-4o' };
  const chunk = (delta, finish_reason = null) => response.write('data: ' + JSON.stringify({ ...base, choices: [{ index: 0, delta, finish_reason }] }) + '\n\n');
  if (!body.messages.some(message => message.role === 'tool')) {
    let tool;
    if (text.includes('PARITY_STEER_ROOT')) tool={name:'Bash',arguments:{command:'sleep 3',description:'guide checkpoint'}};
    if (text.includes('PARITY_AGENT_BACKGROUND')) tool={name:'Agent',arguments:{subagent_type:'general-purpose',run_in_background:true,description:'Parity background child',prompt:'PARITY_CHILD: read parity-child.txt and report PARITY_CHILD_OK'}};
    if (text.includes('PARITY_AGENT_ROOT')) tool={name:'Agent',arguments:{subagent_type:'general-purpose',description:'Parity native child',prompt:'PARITY_CHILD: read parity-child.txt and report PARITY_CHILD_OK'}};
    if (text.includes('PARITY_CHILD')) tool={name:'Read',arguments:{file_path:path.join(workspace,'parity-child.txt')}};
    if (text.includes('WRITE')) tool = { name: 'Write', arguments: { file_path: path.join(workspace, 'written.txt'), content: 'WRITE_OK' } };
    if (text.includes('DENIAL')) tool = { name: 'Bash', arguments: { command: 'printf DENIED > blocked.txt', description: '权限拒绝测试' } };
    if (text.includes('PLAN_REVIEW')) tool = { name: 'ExitPlanMode', arguments: { plan: '实现经过批准的计划。' } };
    if (text.includes('EXTENSION_SKILL')) tool = { name: 'Skill', arguments: { skill: 'plugin-probe' } };
    if (text.includes('EXTENSION_MCP') || text.includes('EXTENSION_PLUGIN_MCP')) {
      const pattern = text.includes('EXTENSION_PLUGIN_MCP') ? /extension.fixture.*echo/i : /extension.local.*echo/i;
      const native = body.tools.find(tool => pattern.test(tool.function.name));
      assert(native, '模型上下文必须收到真实 MCP 工具');
      tool = { name: native.function.name, arguments: {} };
    }
    if (tool) {
      chunk({ role: 'assistant', tool_calls: [{ index: 0, id: 'call-fixture', type: 'function', function: { name: tool.name, arguments: JSON.stringify(tool.arguments) } }] });
      chunk({}, 'tool_calls'); response.end('data: [DONE]\n\n'); return;
    }
  }
  const extension = text.includes('EXTENSION_SKILL') ? 'EXTENSION_SKILL_CONTENT_OK' : text.includes('EXTENSION_PLUGIN_MCP') ? 'EXTENSION_PLUGIN_MCP_CONTENT_OK' : text.includes('EXTENSION_MCP') ? 'EXTENSION_MCP_CONTENT_OK' : null;
  if (extension) assert(body.messages.some(message => message.role === 'tool' && JSON.stringify(message.content).includes(extension)), '必须收到实际技能/MCP 执行结果');
  if (text.includes('PARITY_CHILD')) await delay(1500);
  if (text.includes('PARITY_NO_TOOL')) {await writeFile(path.join(root,'guide-no-tool-started'),'ready');await delay(3000);}
  const textResult = text.includes('PARITY_AGENT_BACKGROUND') ? 'PARITY_BACKGROUND_OK' : text.includes('PARITY_GUIDE') ? 'PARITY_STEER_OK' : text.includes('PARITY_CHILD') ? 'PARITY_CHILD_OK' : text.includes('PARITY_AGENT_ROOT') ? 'PARITY_AGENT_OK' : extension ?? (text.includes('PARITY_IMAGE') ? 'PARITY_IMAGE_OK' : text.includes('UI_RENDER_TEST') ? 'UI_RENDER_OK' : text.includes('ALPHA') ? 'ALPHA_RESULT' : text.includes('BETA') ? 'BETA_RESULT' : 'FIXTURE_RESULT');
  chunk({ role: 'assistant', content: textResult.slice(0, 5) });
  chunk({ content: textResult.slice(5) });
  chunk({}, 'stop'); response.write('data: '+JSON.stringify({...base,choices:[],usage:{prompt_tokens:120,completion_tokens:12,total_tokens:132,prompt_tokens_details:{cached_tokens:30},completion_tokens_details:{reasoning_tokens:2}}})+'\n\n'); response.end('data: [DONE]\n\n');
});
await new Promise(resolve => modelServer.listen(0, '127.0.0.1', resolve));
await writeFile(path.join(nativeRoot, '.zcode/v2/provider_config.json'), JSON.stringify({ schemaVersion: 1, config: {
  providerConfigRules: { providerRules: [{ providerId: 'local-fixture', providerName: 'Local fixture', enabled: true, config: {
    group: 'standard-personal', access: { type: 'api-key', apiKey: 'fixture-only' },
    api: { type: 'openai-chat-completions', baseUrl: `http://127.0.0.1:${modelServer.address().port}/v1` }, personalModelIds: ['gpt-4o', 'gpt-4o-mini'], visibility: 'visible',
  } }] }, modelConfigRules: { providerModelRules: [{providerId:'local-fixture',modelId:'gpt-4o',config:{properties:{contextWindow:128000,inputFormat:{supportsImage:true}}}}], manualProviderModelRules: [] }, defaultModelSelection: { providerId: 'local-fixture', modelId: 'gpt-4o' },
} }));
const reservation = net.createServer();
await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
const port = reservation.address().port;
await new Promise(resolve => reservation.close(resolve));
const password = randomUUID();
const app = spawn(exe, ['--serve', '--local-http', '--port', String(port), '--data-dir', path.join(root, 'vela')], {
  windowsHide: true, stdio: 'ignore', env: { ...process.env, USERPROFILE: profile, VELA_ZCODE_ENV_LOG: environmentLog, VELA_ZCODE_TEST_PROFILE: profile, VELA_ZCODE_TEST_BINARY: bin, ZCODE_DATA_BASE_DIR: nativeRoot, ZCODE_ENDPOINT_ORIGIN: `http://127.0.0.1:${modelServer.address().port}`, VELA_ISOLATED_RUNTIME: '1', VELA_SERVE_PASSWORD: password },
});
let socket;
const sessions = [];
let call;
const ownedProcesses = new Set([app]);
const checkZcodeBrowser = options => inspectZcodeBrowser({ ...options, onBrowser: browser => ownedProcesses.add(browser) });
let outcome;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const bounded = (promise, ms, label) => {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(Error(label)), ms); })])
    .finally(() => clearTimeout(timer));
};
const stopOwnedProcess = async child => {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return;
  const alive=()=>{try{process.kill(child.pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;throw error;}};
  if(!alive()) return;
  const killer = spawn(path.join(process.env.SystemRoot ?? 'C:/Windows', 'System32/taskkill.exe'), ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let report='';
  for (const stream of [killer.stdout,killer.stderr]) stream.on('data',bytes=>{report=(report+bytes.toString()).slice(-2048);});
  await bounded(new Promise((resolve, reject) => { killer.once('exit', resolve); killer.once('error', reject); }), 15000, '测试进程树停止超时');
  // 不依赖可能不可用的 WMI/tasklist；通过系统进程句柄检查实际存活状态。
  if(alive()) child.kill('SIGKILL');
  const end=Date.now()+15000;
  do {
    if(!alive()) return;
    await delay(100);
  } while(Date.now()<end);
  throw Error(`测试进程未退出 ${child.pid}: ${report}`);
};
const parityImage = 'iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAE0lEQVR4nGPUqDjBAANMcBZeDgBGtAFwfeO9fAAAAABJRU5ErkJggg==';
const until = async (read, accept, label, timeout = 45000) => {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) { last = await read(); if (accept(last)) return last; await delay(100); }
  throw Error(label + ' 超时 '+JSON.stringify({extras:{nativeUsage:last?.nativeUsage,turnUsage:last?.turnUsage,contextTokens:last?.contextTokens,contextWindow:last?.contextWindow},rows:last?.rows?.map(r=>({kind:r.kind,name:r.name,status:r.status,error:r.kind==='error'?r.message:undefined})),permissions:last?.permissions?.length}));
};
async function exercise() {
try {
  const base = `http://127.0.0.1:${port}`;
  await until(async () => { try { return (await fetch(base + '/api/mode', { signal: AbortSignal.timeout(500) })).ok; } catch { return false; } }, Boolean, '后端启动');
  const login = await fetch(base + '/api/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ password }) });
  assert(login.ok);
  const { token } = await login.json();
  socket = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${encodeURIComponent(token)}`);
  await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, { once: true }); socket.addEventListener('error', reject, { once: true }); });
  let sequence = 0;
  const pending = new Map();
  const guideEvents = [];
  socket.addEventListener('message', event => {
    const frame = JSON.parse(event.data);
    if (frame.t === 'event' && ['steerPending','steerAccepted','steerFallback'].includes(frame.payload?.type)) guideEvents.push({name:frame.name,...frame.payload});
    if (frame.t !== 'reply') return;
    const request = pending.get(frame.id);
    if (!request) return;
    pending.delete(frame.id);
    if (frame.ok) request.resolve(frame.result); else request.reject(Error(frame.error));
  });
  call = (cmd, args = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(cmd + ' 超时')); }, 45000);
    pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(Error(cmd + ': ' + error.message)); } });
    socket.send(JSON.stringify({ t: 'invoke', id, cmd, args }));
  });
  assert.equal(await call('app_version'), expectedVersion);
  await call('set_app_settings', { entries: { 'vlx-settings': JSON.stringify({ agentDefaults: { zcode: { path: wrapper } } }) } });
  const project = await call('import_project', { rootPath: workspace });
  const parent = await call('create_session', { projectId: project.id, name: 'Zcode 测试父会话', kind: 'terminal', cwd: workspace, engine: 'tui' });
  const create = async (start = true) => {
    const session = await call('agent_session_create', { request: { requestId: randomUUID(), context: { projectId: project.id, groupId: null, activeSessionId: parent.id, placement: 'child' }, kind: 'zcode', presetId: null } });
    assert.equal(session.kind, 'zcode'); assert.equal(session.parentSessionId, parent.id); assert.equal(session.engine, 'chat');
    sessions.push(session.id);
    if (!start) return session.id;
    await call('chat_start', { sessionId: session.id, fastMode: false });
    await until(() => call('chat_snapshot', { sessionId: session.id }), snapshot => snapshot.model === 'local-fixture/gpt-4o', '原生模型就绪');
    return session.id;
  };
  const parityCheck=async()=>{
  await writeFile(path.join(workspace,'parity-child.txt'),'PARITY_CHILD_READ_BYTES');
  const imageSession=await create();
  let imageSentAgain=false;
  await call('chat_send',{sessionId:imageSession,text:'PARITY_IMAGE',images:[{mimeType:'image/png',data:parityImage}]});
  await until(()=>call('chat_snapshot',{sessionId:imageSession}),snapshot=>snapshot.rows.some(row=>row.kind==='assistant'&&row.text==='PARITY_IMAGE_OK'),'图片到达原生模型');
  if (!process.env.ZCODE_PARITY_ONLY || ['all','usage'].includes(process.env.ZCODE_PARITY_ONLY)) {
    const liveUsage=await until(()=>call('chat_snapshot',{sessionId:imageSession}),s=>!s.turnStartedAt&&s.turnUsage?.outputTokens===12,'当前回合用量');
    assert.equal(liveUsage.turnUsage.inputTokens,120);assert.equal(liveUsage.turnUsage.cacheReadTokens,30);
  }
  await call('chat_stop',{sessionId:imageSession});
  await call('chat_start',{sessionId:imageSession,fastMode:false});
  const restoredImage=await until(()=>call('chat_snapshot',{sessionId:imageSession}),snapshot=>snapshot.rows.some(row=>row.kind==='user'&&row.text==='PARITY_IMAGE'&&row.images.length),'恢复原生图片历史');
  const imageRef=restoredImage.rows.find(row=>row.kind==='user'&&row.text==='PARITY_IMAGE').images[0];
  const imageBytes=await call('chat_attachment',{sessionId:imageSession,attachmentId:imageRef.attachmentId});
  assert.equal(imageBytes.data,parityImage); assert.equal(imageBytes.mimeType,'image/png');
  if (!process.env.ZCODE_PARITY_ONLY || process.env.ZCODE_PARITY_ONLY==='all' || process.env.ZCODE_PARITY_ONLY==='usage') {
  const imageUsage=await until(()=>call('chat_snapshot',{sessionId:imageSession}),s=>s.nativeUsage?.inputTokens===120&&s.contextWindow===128000,'原生会话用量');
  assert.equal(imageUsage.nativeUsage.outputTokens,12);
  assert.equal(imageUsage.nativeUsage.cachedInputTokens,30);
  assert.equal(imageUsage.contextWindow,128000);
  assert.equal(imageUsage.totalCostUsd,undefined,'不能编造美元费用');
  await call('chat_send',{sessionId:imageSession,text:'PARITY_IMAGE_AGAIN'});
  const resumedUsage=await until(()=>call('chat_snapshot',{sessionId:imageSession}),s=>!s.turnStartedAt&&s.nativeUsage?.inputTokens===240,'恢复后累计应包含两个真实请求');
  assert.equal(resumedUsage.nativeUsage.outputTokens,24);assert.equal(resumedUsage.nativeUsage.cachedInputTokens,60);
  imageSentAgain=true;
  }
  if (!process.env.ZCODE_PARITY_ONLY || process.env.ZCODE_PARITY_ONLY==='all' || process.env.ZCODE_PARITY_ONLY==='steer') {
    const steering=await create();
    await call('chat_set_mode',{sessionId:steering,mode:'yolo'});
    await call('chat_send',{sessionId:steering,text:'PARITY_STEER_ROOT'});
    const active=await until(()=>call('chat_snapshot',{sessionId:steering}),s=>s.rows.some(r=>r.kind==='tool'&&r.name==='Bash'&&r.status==='running'),'原生 guide 检查点');
    await assert.rejects(call('chat_send',{sessionId:steering,text:'PARITY_GUIDE_IMAGE',behavior:'steer',images:[{mimeType:'image/png',data:parityImage}]}),/图片.*排队/);
    assert.equal(await call('chat_send',{sessionId:steering,text:'PARITY_GUIDE',behavior:'steer'}),'steered');
    const accepted=await call('chat_snapshot',{sessionId:steering});
    assert.equal(accepted.turnStartedAt,active.turnStartedAt,'guide 不能创建替代回合');
    assert.equal(accepted.queue?.length??0,0);
    assert.equal(await call('chat_send',{sessionId:steering,text:'PARITY_IMAGE_QUEUE',behavior:'queue',images:[{mimeType:'image/png',data:parityImage}]}),'queued');
    const queued=await call('chat_snapshot',{sessionId:steering});assert.equal(queued.queue[0].images[0].mimeType,'image/png');
    const done=await until(()=>call('chat_snapshot',{sessionId:steering}),s=>!s.turnStartedAt&&s.rows.some(r=>r.kind==='assistant'&&r.text==='PARITY_STEER_OK')&&s.rows.some(r=>r.kind==='assistant'&&r.text==='PARITY_IMAGE_OK'),'guide 生效且排队图片没有丢失');
    assert.equal(done.rows.filter(r=>r.kind==='user'&&r.text==='PARITY_GUIDE').length,1);
    assert(guideEvents.some(e=>e.name.endsWith('/'+steering)&&e.type==='steerAccepted'),'实际工具边界采用 guide 后才报告成功');
    const noTool=await create();
    await call('chat_send',{sessionId:noTool,text:'PARITY_NO_TOOL'});
    await until(async()=>{try{await readFile(path.join(root,'guide-no-tool-started'));return true;}catch{return false;}},Boolean,'无工具边界模型正在生成');
    const noToolActive=await call('chat_snapshot',{sessionId:noTool});
    assert.equal(await call('chat_send',{sessionId:noTool,text:'PARITY_GUIDE_NO_TOOL',behavior:'steer'}),'steered');
    await until(()=>Promise.resolve(guideEvents),events=>events.some(e=>e.name.endsWith('/'+noTool)&&e.type==='steerAccepted'),'原生可在纯文本步骤边界采用 guide');
    const inlineDone=await until(()=>call('chat_snapshot',{sessionId:noTool}),s=>!s.turnStartedAt&&s.rows.some(r=>r.kind==='assistant'&&r.text==='PARITY_STEER_OK'),'纯文本步骤采用引导');
    assert(!guideEvents.some(e=>e.name.endsWith('/'+noTool)&&e.type==='steerFallback'));
    await rm(path.join(root,'guide-no-tool-started'));
    const interruptedGuide=await create();
    await call('chat_send',{sessionId:interruptedGuide,text:'PARITY_NO_TOOL_INTERRUPT'});
    await until(async()=>{try{await readFile(path.join(root,'guide-no-tool-started'));return true;}catch{return false;}},Boolean,'中断测试的模型正在生成');
    assert.equal(await call('chat_send',{sessionId:interruptedGuide,text:'PARITY_GUIDE_INTERRUPTED',behavior:'steer'}),'steered');
    await call('chat_interrupt',{sessionId:interruptedGuide});
    await until(()=>Promise.resolve(guideEvents),events=>events.some(e=>e.name.endsWith('/'+interruptedGuide)&&e.type==='steerFallback'&&e.reason==='guide.turnInterrupted'),'原生中断导致 guide 降级必须明确报告');
    assert(!guideEvents.some(e=>e.name.endsWith('/'+interruptedGuide)&&e.type==='steerAccepted'),'不能把排队降级报告为当前回合引导成功');
  }
  if (!process.env.ZCODE_PARITY_ONLY || process.env.ZCODE_PARITY_ONLY==='all' || process.env.ZCODE_PARITY_ONLY==='subagent') {
    const parentAgent=await create();
    await call('chat_set_mode',{sessionId:parentAgent,mode:'yolo'});
    await call('chat_send',{sessionId:parentAgent,text:'PARITY_AGENT_ROOT'});
    const childCard=await until(()=>call('chat_snapshot',{sessionId:parentAgent}),s=>s.rows.some(r=>r.kind==='tool'&&r.name==='Agent'&&r.subagent?.model==='local-fixture/gpt-4o'&&r.childCount>0),'原生子任务过程可见');
    const preview=childCard.rows.find(r=>r.kind==='tool'&&r.name==='Agent');
    const detail=await until(()=>call('chat_row',{sessionId:parentAgent,rowId:preview.id}),r=>r.children.some(c=>c.kind==='tool'&&c.name==='Read'&&c.status==='completed'&&c.output?.includes('PARITY_CHILD_READ_BYTES')),'子工具结果已投影');
    assert(detail.children.some(c=>c.kind==='tool'&&c.name==='Read'),'展开卡片应取得真实子工具');
    const childRead=detail.children.find(c=>c.kind==='tool'&&c.name==='Read');
    assert.equal(childRead.input.file_path,path.join(workspace,'parity-child.txt'));
    assert(childRead.output.includes('PARITY_CHILD_READ_BYTES'),'过程卡片应显示实际子工具结果');
    assert(!childCard.rows.some(r=>r.kind==='tool'&&r.name==='Read'),'子任务不能混入主时间线');
    await until(()=>call('chat_snapshot',{sessionId:parentAgent}),s=>!s.turnStartedAt&&s.rows.some(r=>r.kind==='assistant'&&r.text==='PARITY_AGENT_OK'),'原生子任务完成');
    await call('chat_stop',{sessionId:parentAgent});
    await call('chat_start',{sessionId:parentAgent,fastMode:false});
    const restoredChild=await until(()=>call('chat_snapshot',{sessionId:parentAgent}),s=>s.rows.some(r=>r.kind==='tool'&&r.name==='Agent'&&r.childCount>1),'恢复原生子任务历史');
    const restoredDetail=await call('chat_row',{sessionId:parentAgent,rowId:restoredChild.rows.find(r=>r.kind==='tool'&&r.name==='Agent').id});
    assert(restoredDetail.children.some(c=>c.kind==='assistant'&&c.text==='PARITY_CHILD_OK'));
    const background=await create();
    await call('chat_set_mode',{sessionId:background,mode:'yolo'});
    await call('chat_send',{sessionId:background,text:'PARITY_AGENT_BACKGROUND'});
    const rootDone=await until(()=>call('chat_snapshot',{sessionId:background}),s=>!s.turnStartedAt&&s.rows.some(r=>r.kind==='assistant'&&r.text==='PARITY_BACKGROUND_OK'),'父回合先于后台子任务结束');
    await until(()=>call('chat_snapshot',{sessionId:background}),s=>!s.turnStartedAt&&s.rows.some(r=>r.kind==='tool'&&r.name==='Agent'&&r.status==='running'),'父工具返回后，后台子任务卡片仍须显示运行中',1000);
    await until(()=>call('chat_snapshot',{sessionId:background}),s=>s.rows.some(r=>r.kind==='tool'&&r.name==='Agent'&&r.status==='completed'&&r.childCount>1),'后台子任务终态');
    const parityTree=await call('list_tree');
    await checkZcodeBrowser({base,token,root,sessionName:parityTree.sessions.find(s=>s.id===parentAgent).name,parityImageSession:parityTree.sessions.find(s=>s.id===imageSession).name,parityUsage:{input:imageSentAgain?240:120,cached:imageSentAgain?60:30}});
  }
  };
  if (process.env.ZCODE_PARITY_ONLY) {await parityCheck();outcome={parity:process.env.ZCODE_PARITY_ONLY,passed:true};return;}
  // 空白会话尚无 CLI 进程：必须先能选择模型，不能先发送消息才能解锁菜单。
  const cold = await create(false);
  const coldCommands = await call('chat_commands', { sessionId: cold });
  for (const name of ['user-probe', 'workspace-probe', 'plugin-probe']) assert(coldCommands.some(command => command.name === name && command.invocation === '/'), '空白会话缺少技能：' + name);
  const noLoginPlan = await call('chat_zcode_start_plan', { sessionId: cold });
  assert(noLoginPlan.plans.length > 0 && noLoginPlan.plans.every(plan => plan.status === 'not-authenticated'), '未登录时需给出明确的 Start Plan 状态');
  const coldModels = await call('chat_models', { sessionId: cold });
  const launchModels = await call('launch_models', { kind: 'zcode', context: { parentSessionId: parent.id, cwd: workspace } });
  assert.deepEqual(launchModels.models.map(({ id, effortLevels }) => ({ id, effortLevels })),
    coldModels.map(({ id, effortLevels }) => ({ id, effortLevels })), '启动模型菜单必须保留原生逐模型思考档位');
  assert(coldModels.some(model => model.id === 'local-fixture/gpt-4o-mini' && model.description.includes('Local fixture')), '首次启动前模型菜单必须显示原生模型和供应商');
  assert(!(await call('list_tree')).sessions.find(session => session.id === cold).agentSessionId, '目录查询不能绑定临时原生会话');
  const coldBrowserUI = await checkZcodeBrowser({ base, token, sessionName: (await call('list_tree')).sessions.find(session => session.id === cold).name, root, cold: true });
  assert.equal(generationRequests, 0, '打开空白模型菜单不能触发模型生成');
  await call('chat_set_model', { sessionId: cold, model: 'local-fixture/gpt-4o-mini' });
  await call('chat_start', { sessionId: cold, fastMode: false });
  await until(() => call('chat_snapshot', { sessionId: cold }), snapshot => snapshot.model === 'local-fixture/gpt-4o-mini', '首次启动应用预选模型');
  const coldNative = (await call('list_tree')).sessions.find(session => session.id === cold).agentSessionId;
  await call('chat_stop', { sessionId: cold });
  assert((await call('chat_models', { sessionId: cold })).some(model => model.id === 'local-fixture/gpt-4o-mini'), '停止后已有会话仍须提供模型菜单');
  assert.equal((await call('list_tree')).sessions.find(session => session.id === cold).agentSessionId, coldNative, '查询不能改写已保存的原生会话 ID');
  await parityCheck();
  const finished = sessionId => until(() => call('chat_snapshot', { sessionId }), snapshot => !snapshot.turnStartedAt && snapshot.rows.some(row => row.kind === 'assistant' && !row.streaming), '原生回合结束');
  const [alpha, beta] = await Promise.all([create(), create()]);
  await Promise.all([call('chat_send', { sessionId: alpha, text: 'ALPHA' }), call('chat_send', { sessionId: beta, text: 'BETA' })]);
  const answers = await Promise.all([finished(alpha), finished(beta)]);
  assert(answers[0].rows.some(row => row.kind === 'assistant' && row.text === 'ALPHA_RESULT'));
  assert(!JSON.stringify(answers[0].rows).includes('BETA_RESULT'));
  assert(answers[1].rows.some(row => row.kind === 'assistant' && row.text === 'BETA_RESULT'));
  const before = (await call('list_tree')).sessions.find(session => session.id === alpha).agentSessionId;
  assert(before?.startsWith('sess_'));
  const catalogue = await call('chat_models', { sessionId: alpha });
  assert(JSON.stringify(catalogue).includes('local-fixture/gpt-4o'));
  assert(catalogue.every(model => typeof model.description === 'string'), '模型目录的说明必须为字符串，避免聊天界面 startsWith 崩溃');
  for (const mode of ['plan', 'edit', 'yolo', 'build']) await call('chat_set_mode', { sessionId: alpha, mode });
  await call('chat_set_effort', { sessionId: alpha, effort: 'disabled' });
  await call('chat_set_effort', { sessionId: alpha, effort: null });
  await call('chat_stop', { sessionId: alpha });
  assert(JSON.stringify(await call('read_agent_transcript', { sessionId: alpha })).includes('ALPHA_RESULT'));
  const launchProfiles = (await readFile(environmentLog, 'utf8')).trim().split(/\r?\n/);
  assert(launchProfiles.length >= 3);
  assert(launchProfiles.every(value => value === launchProfiles[0]), '主 CLI 与历史查询必须使用相同的登录环境');
  await call('chat_start', { sessionId: alpha, fastMode: false });
  await call('chat_send', { sessionId: alpha, text: 'ALPHA AGAIN' });
  await until(() => call('chat_snapshot', { sessionId: alpha }), snapshot => snapshot.rows.filter(row => row.kind === 'assistant' && !row.streaming).length >= 2, '恢复后继续对话');
  assert.equal((await call('list_tree')).sessions.find(session => session.id === alpha).agentSessionId, before);
  const writer = await create();
  const extensionSession = await create();
  const liveCommands = await call('chat_commands', { sessionId: extensionSession });
  assert(liveCommands.some(command => command.name === 'plugin-probe'));
  const mcpBefore = await call('chat_mcp_status', { sessionId: extensionSession });
  assert(mcpBefore.mcpServers.some(server => server.name === 'extension-local'));
  const mcpConnected = await call('chat_mcp_reconnect', { sessionId: extensionSession, server: 'extension-local' });
  for (const name of ['extension-local', 'plugin:extension-fixture:tools']) {
    const server = mcpConnected.mcpServers.find(server => server.name === name);
    assert.equal(server?.status, 'connected'); assert.equal(server.toolCount, 1); assert.equal(server.canToggle, false);
  }
  assert.equal(mcpConnected.mcpServers.find(server => server.name === 'extension-disabled')?.status, 'disabled');
  assert(mcpConnected.mcpServers.find(server => server.name === 'extension-broken')?.error, '连接失败必须保留原生错误');
  await call('chat_send', { sessionId: extensionSession, text: '/plugin-probe EXTENSION_SKILL' });
  assert((await finished(extensionSession)).rows.some(row => row.kind === 'tool' && row.name === 'Skill' && row.status === 'completed' && !row.isError));
  const mcpSession = await create();
  await call('chat_set_mode', { sessionId: mcpSession, mode: 'yolo' });
  await call('chat_send', { sessionId: mcpSession, text: 'EXTENSION_MCP' });
  const mcpGate = await until(() => call('chat_snapshot', { sessionId: mcpSession }), snapshot => snapshot.permissions.length || snapshot.rows.some(row => row.kind === 'assistant' && row.text === 'EXTENSION_MCP_CONTENT_OK'), 'MCP 工具执行或权限确认');
  if (mcpGate.permissions.length) {
    assert(mcpGate.permissions.every(permission => /echo/.test(permission.tool_name)), '只允许测试自己的 MCP 工具');
    for (const permission of mcpGate.permissions) await call('chat_permission', { sessionId: mcpSession, requestId: permission.id, allow: true });
  }
  assert((await finished(mcpSession)).rows.some(row => row.kind === 'assistant' && row.text === 'EXTENSION_MCP_CONTENT_OK'));
  const pluginMcpSession = await create();
  await call('chat_send', { sessionId: pluginMcpSession, text: 'EXTENSION_PLUGIN_MCP' });
  const pluginGate = await until(() => call('chat_snapshot', { sessionId: pluginMcpSession }), snapshot => snapshot.permissions.length || snapshot.rows.some(row => row.kind === 'assistant' && row.text === 'EXTENSION_PLUGIN_MCP_CONTENT_OK'), '插件 MCP 工具执行或权限确认');
  for (const permission of pluginGate.permissions) {
    assert(/extension.fixture.*echo/.test(permission.tool_name), '只允许测试插件自己的 MCP 工具');
    await call('chat_permission', { sessionId: pluginMcpSession, requestId: permission.id, allow: true });
  }
  const pluginResult = await finished(pluginMcpSession);
  assert(pluginResult.rows.some(row => row.kind === 'tool' && /extension.fixture.*echo/.test(row.name) && row.status === 'completed' && row.output?.includes('EXTENSION_PLUGIN_MCP_CONTENT_OK')));
  // 设置页探测不能占用 resident CLI 的串行传输，阻塞已经就绪的聊天。
  await writeFile(slowMcpFlag, 'slow');
  const slowProbe = call('chat_mcp_reconnect', { sessionId: mcpSession, server: 'extension-local' }).then(value => ({ value }), error => ({ error }));
  await until(async () => { try { await readFile(slowMcpStarted); return true; } catch { return false; } }, Boolean, '慢 MCP 已开始探测');
  const beforeConcurrentChat = Date.now();
  await call('chat_send', { sessionId: mcpSession, text: 'ALPHA_CONCURRENT' });
  await until(() => call('chat_snapshot', { sessionId: mcpSession }), snapshot => snapshot.rows.some(row => row.kind === 'assistant' && row.text === 'ALPHA_RESULT'), 'MCP 探测期间仍能聊天', 5000);
  assert(Date.now() - beforeConcurrentChat < 5000, 'MCP 探测堵塞聊天');
  await rm(slowMcpFlag);
  const probed = await slowProbe;
  assert(!probed.error, '慢 MCP 探测应能收尾');
  await call('chat_set_mode', { sessionId: writer, mode: 'yolo' });
  await call('chat_send', { sessionId: writer, text: 'WRITE' });
  const written = await finished(writer);
  assert.equal(await readFile(path.join(workspace, 'written.txt'), 'utf8'), 'WRITE_OK');
  assert(written.rows.some(row => row.kind === 'tool' && row.name === 'Write' && row.input.content === 'WRITE_OK' && row.status === 'completed'));
  const denied = await create();
  await call('chat_send', { sessionId: denied, text: 'DENIAL' });
  const deny = await until(() => call('chat_snapshot', { sessionId: denied }), snapshot => snapshot.permissions.length, '工具权限确认');
  await delay(2200);
  const repeated = await call('chat_snapshot', { sessionId: denied });
  assert.equal(repeated.permissions.length, 1, '原生交互重发应保留一张卡片');
  assert.equal(repeated.permissions[0].id, deny.permissions[0].id);
  await call('chat_permission', { sessionId: denied, requestId: deny.permissions[0].id, allow: false, message: '测试拒绝' });
  await finished(denied);
  await assert.rejects(readFile(path.join(workspace, 'blocked.txt')), { code: 'ENOENT' });
  const cancelled = await create();
  await call('chat_send', { sessionId: cancelled, text: 'DENIAL' });
  const pendingPermission = await until(() => call('chat_snapshot', { sessionId: cancelled }), snapshot => snapshot.permissions.length, '中断前的待确认卡片');
  await call('chat_interrupt', { sessionId: cancelled });
  await until(() => call('chat_snapshot', { sessionId: cancelled }), snapshot => !snapshot.turnStartedAt && !snapshot.permissions.length, '中断清除待确认卡片');
  await assert.rejects(call('chat_permission', { sessionId: cancelled, requestId: pendingPermission.permissions[0].id, allow: true }));
  const planner = await create();
  await call('chat_set_mode', { sessionId: planner, mode: 'plan' });
  await call('chat_send', { sessionId: planner, text: 'PLAN_REVIEW' });
  const review = await until(() => call('chat_snapshot', { sessionId: planner }), snapshot => snapshot.permissions.some(permission => permission.tool_name === 'AskUserQuestion'), '计划批准卡片');
  const question = review.permissions.find(permission => permission.tool_name === 'AskUserQuestion');
  const plan = question.input.questions[0];
  assert.equal(plan.options[0].label, 'Approve'); assert.equal(plan.options[0].value, 'approve');
  await call('chat_permission', { sessionId: planner, requestId: question.id, allow: true, updatedInput: { ...question.input, answers: { [plan.question]: plan.options[0].value } } });
  const approved = await finished(planner);
  assert(approved.rows.some(row => row.kind === 'tool' && row.name === 'ExitPlanMode' && row.status === 'completed' && !row.isError));
  assert.equal(approved.mode, 'build', '原生计划批准后，界面必须反映实际权限模式');
  await call('chat_send', { sessionId: beta, text: 'STALL' });
  await delay(500);
  await call('chat_interrupt', { sessionId: beta });
  await until(() => call('chat_snapshot', { sessionId: beta }), snapshot => !snapshot.turnStartedAt, '原生中断');
  const tree = await call('list_tree');
  const browserUI = await checkZcodeBrowser({ base, token, sessionName: tree.sessions.find(session => session.id === alpha).name, root });
  const loginHelper = spawn(bin, ['login', 'bigmodel', 'start-plan', '--no-browser', '--json'], { cwd: workspace,
    windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'], env: { ...process.env, USERPROFILE: profile,
      ZCODE_DATA_BASE_DIR: nativeRoot, ZCODE_ENDPOINT_ORIGIN: `http://127.0.0.1:${modelServer.address().port}` } });
  ownedProcesses.add(loginHelper);
  loginHelper.stdout.resume();
  assert.equal(await new Promise((resolve, reject) => { const timer = setTimeout(() => { loginHelper.kill(); reject(Error('Start Plan 登录夹具超时')); }, 45000);
    loginHelper.on('error', reject); loginHelper.once('exit', code => { clearTimeout(timer); resolve(code); }); }), 0, '只有 Start Plan 时也必须通过原生 CLI 登录');
  const startPlan = await create(false);
  const pendingPlan = await call('chat_zcode_start_plan', { sessionId: startPlan });
  assert(pendingPlan.plans.some(plan => plan.status === 'pending' && plan.plans.some(item => item.name === 'ZCode Global Build')));
  assert(!(await call('chat_models', { sessionId: startPlan })).some(model => model.id.includes('start-plan/')));
  const startPlanBrowserUI = await checkZcodeBrowser({ base, token, sessionName: (await call('list_tree')).sessions.find(session => session.id === startPlan).name,
    root, cold: true, startPlanFixture: { activate: () => { startPlanPhase = 'available'; } } });
  startPlanPhase = 'available';
  await call('chat_zcode_start_plan', { sessionId: startPlan });
  const planModels = await call('chat_models', { sessionId: startPlan });
  const planModel = 'account:bigmodel-start-plan/GLM-5.3-Flash';
  assert(planModels.some(model => model.id === planModel && model.description.includes('Start Plan')));
  if (!startPlanBrowserUI) {
    await call('chat_set_model', { sessionId: startPlan, model: planModel });
    await call('chat_start', { sessionId: startPlan, fastMode: false });
    await until(() => call('chat_snapshot', { sessionId: startPlan }), snapshot => snapshot.model === planModel, 'Start Plan 模型就绪');
    await call('chat_send', { sessionId: startPlan, text: 'START_PLAN_TEST' });
    assert((await finished(startPlan)).rows.some(row => row.kind === 'assistant' && row.text === 'START_PLAN_OK'));
  }
  const usedPlan = await call('chat_zcode_start_plan', { sessionId: startPlan });
  assert.equal(usedPlan.plans.find(plan => plan.providerId === 'account:bigmodel-start-plan').balances[0].available, 75);
  assert.equal(startPlanRequests, 1); assert.equal(unexpectedAccountRequests, 0, '不能兑换个人 Coding Plan 的 Token 或 Key');
  startPlanRemaining = 0;
  const exhausted = await call('chat_zcode_start_plan', { sessionId: startPlan });
  assert.equal(exhausted.plans.find(plan => plan.providerId === 'account:bigmodel-start-plan').status, 'unavailable');
  assert(!(await call('chat_models', { sessionId: startPlan })).some(model => model.id === planModel), '额度耗尽后运行中的目录也必须移除模型');
  const generationBeforeExhaustedSend = generationRequests;
  await call('chat_send', { sessionId: startPlan, text: 'EXHAUSTED_START_PLAN' }).catch(() => {});
  await delay(1500);
  assert.equal(startPlanRequests, 1); assert.equal(generationRequests, generationBeforeExhaustedSend, '额度耗尽不能自动回退到第三方模型');
  const result = { nativeImages: true, nativeImageHistory: true, nativeGuide: true, nativeGuideImageRejection: true, nativeSubagentProjection: true, nativeUsage: true, nativeSkillCatalogue: true, nativeSkillTool: true, nativeMcpTool: true, nativePluginMcpTool: true, nativeMcpFailureAndDisabled: true, slowMcpDoesNotBlockChat: true, appVersion: expectedVersion, coldModelCatalogue: true, coldModelSelection: true, stoppedModelCatalogue: true, coldBrowserUI, nativeConcurrentSessions: true, nativeResume: true, offlineHistory: true, permissionDenial: true, permissionReannouncement: true, pendingInteractionCancellation: true, writeToolAndArguments: true, planApproval: true, interrupt: true, browserUI, startPlanBrowserUI, startPlanPending: true, startPlanJwtRequest: true, startPlanQuotaUpdated: true, startPlanNoFallback: true, zcodeGuiUsed: false, productionAccountAuthorization: false };
  await writeFile(path.join(root, 'result.json'), JSON.stringify(result, null, 2));
  outcome={ ...result, evidence: path.join(root, 'result.json') };
} finally {
  if (call && socket?.readyState === WebSocket.OPEN) await bounded(Promise.allSettled(sessions.map(sessionId => call('chat_stop', { sessionId }))), 12000, '测试会话停止超时').catch(() => {});
  socket?.close();
  const stopped=await Promise.allSettled(Array.from(ownedProcesses,stopOwnedProcess));
  modelServer.closeAllConnections();
  await bounded(new Promise(resolve => modelServer.close(resolve)), 5000, '本地模型测试服务停止超时');
  const failures=stopped.filter(result=>result.status==='rejected').map(result=>result.reason);
  if(failures.length) throw new AggregateError(failures,'测试进程清理失败');
}

}
await exercise();
console.log(JSON.stringify({ ...outcome, cleanupVerified: true }));
