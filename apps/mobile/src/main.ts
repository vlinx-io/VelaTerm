import { Capacitor } from '@capacitor/core';
import { Remote, type Connection } from './remote';
import { routeFromHash, editHref, copyHref } from './routes';
import { initI18n, getLocale, t } from '../../../src/i18n';
import { AccountLogin } from './account-login';
import { ConnectionAttempt } from './connection-attempt';
import { notificationSettings } from './notification-settings';
import { NotificationNavigation } from './notification-navigation';
import './style.css';
const app=document.querySelector<HTMLElement>('#app')!;
let records:Connection[]=[];
let busy=false;
let scanning=false;
const accountLogin=new AccountLogin(action=>Remote.account({action}));
let releaseLoginView:()=>void=()=>{};
let renderGeneration=0;
const phases:Record<string,Parameters<typeof t>[0]>={connecting:'mobile.phaseConnecting',confirming:'mobile.phaseConfirming',preparing:'mobile.phasePreparing',forwarding:'mobile.phaseForwarding',ready:'mobile.phaseReady',disconnected:'mobile.phaseDisconnected',error:'mobile.phaseError'};
// Build time is embedded as UTC ISO text; show it in the device's locale and time zone.
const buildTime=()=>new Intl.DateTimeFormat(getLocale(),{dateStyle:'medium',timeStyle:'short'}).format(new Date(__MOBILE_BUILD_TIME__));
function el<K extends keyof HTMLElementTagNameMap>(tag:K,text?:string,cls?:string):HTMLElementTagNameMap[K] {const e=document.createElement(tag);if(text)e.textContent=text;if(cls)e.className=cls;return e}
function link(text:string,href:string):HTMLAnchorElement {const a=el('a',text);a.href=href;return a}
function button(text:string,action:()=>void,cls=''):HTMLButtonElement {const b=el('button',text,cls);b.type='button';b.onclick=action;return b}
function status(text:string,error=false) {const target=document.querySelector<HTMLElement>('.connection-loading [role=status]') ?? document.querySelector<HTMLElement>('[role=status]');if(target){target.textContent=text;target.classList.toggle('error',error)}}
function lock(value:boolean) {busy=value;document.querySelectorAll<HTMLButtonElement>('button[data-connect],button[type=submit]').forEach(b=>b.disabled=value)}
const connectionAttempt = new ConnectionAttempt();
const notificationNavigation = new NotificationNavigation();
let releaseConnectionLoading: () => void = () => {};
function cancelConnection() {
  notificationNavigation.cancel();
  connectionAttempt.cancel(); releaseConnectionLoading(); lock(false);
  void Remote.disconnect().catch(error => status(String(error), true));
}
async function connect(id:string,sessionId?:string) {
  if(busy || scanning)return;
  lock(true);
  const panel = el('dialog', undefined, 'connection-loading');
  panel.setAttribute('aria-labelledby', 'connection-loading-title');
  const content = el('div', undefined, 'connection-loading-content');
  const progress = el('span', undefined, 'connection-spinner'); progress.setAttribute('aria-hidden', 'true');
  const title = el('h2', t('common.loading')); title.id = 'connection-loading-title';
  const detail = el('p'); detail.setAttribute('role', 'status'); detail.setAttribute('aria-live', 'polite');
  const back = button(t('mobile.backConnections'), cancelConnection);
  back.id = 'connection-back';
  content.append(progress, title, detail, back); panel.append(content); document.body.append(panel);
  panel.addEventListener('cancel', event => { event.preventDefault(); cancelConnection() });
  const timer = window.setTimeout(() => { detail.textContent = t('mobile.loadSlow') }, 30000);
  releaseConnectionLoading = () => { clearTimeout(timer); panel.close(); panel.remove(); releaseConnectionLoading = () => {} };
  panel.showModal();
  await connectionAttempt.run(
    () => id.startsWith('account_')
      ? Remote.account({action:'open',grantId:id.slice(8),...(sessionId?{sessionId}:{})})
      : Remote.connect({id,...(sessionId?{sessionId}:{})}),
    error => { progress.hidden = true; title.textContent = t('mobile.connectionUnavailable'); detail.textContent = error instanceof Error ? error.message : String(error); clearTimeout(timer) },
    () => { if (!progress.hidden) { releaseConnectionLoading(); lock(false) } },
  );
}
function field(form:HTMLFormElement,label:string,name:string,value='',type='text',placeholder='') {
  const wrap=el('label',label);const input=el('input');input.name=name;input.type=type;input.value=value;input.placeholder=placeholder;input.autocomplete='off';input.spellcheck=false;input.setAttribute('autocapitalize','none');wrap.append(input);form.append(wrap);return input;
}
function select(form:HTMLFormElement,label:string,name:string,options:[string,string][],value:string) {
  const wrap=el('label',label);const s=el('select');s.name=name;for(const [key,text] of options){const o=el('option',text);o.value=key;s.append(o)}s.value=value;wrap.append(s);form.append(wrap);return s;
}
function check(form:HTMLFormElement,label:string,name:string,value:boolean) {const wrap=el('label',undefined,'check');const input=el('input');input.type='checkbox';input.name=name;input.checked=value;wrap.append(input,document.createTextNode(label));form.append(wrap);return input}
async function render() {
  const generation=++renderGeneration;
  releaseLoginView();releaseLoginView=()=>{};
  if(busy) cancelConnection();
  app.replaceChildren();const header=el('header');const logo=el('img',undefined,'mark');logo.src=new URL('../assets/icon-ios.svg',import.meta.url).href;logo.alt='';logo.width=44;logo.height=44;logo.draggable=false;header.append(logo,el('div','VelaTerm','brand'));const accountButton=link('','#/account');accountButton.className='account-button';accountButton.innerHTML='<svg width=22 height=22 viewBox="0 0 24 24" fill=none stroke=currentColor stroke-width=1.7 aria-hidden=true><circle cx=12 cy=8 r=4></circle><path d="M4 22v-2a8 8 0 0 1 16 0v2"></path></svg>';accountButton.title=t('mobile.accountAndLogin');accountButton.setAttribute('aria-label',t('mobile.accountAndLogin'));header.append(accountButton);app.append(header);
  const notice=el('p');notice.setAttribute('role','status');notice.setAttribute('aria-live','polite');
  try {records=(await Remote.list()).connections} catch(error) {app.append(el('h1',t('mobile.connectionService')),el('p',Capacitor.isNativePlatform()?String(error):t('mobile.nativeOnly')));records=[]}
  if(generation!==renderGeneration)return;
  const footer=el('footer');footer.append(notice,el('p',t('mobile.managedRemotely'),'hint'),el('p',t('mobile.buildInfo',__MOBILE_VERSION__,buildTime()),'hint build-info'));
  const route=routeFromHash(location.hash);
  if(route.page==='notifications') {
    app.append(link('‹ '+t('mobile.backConnections'),'#/'),el('h1',t('mobile.pushTitle')),notificationSettings(records));
  } else if(route.page==='remote' || route.page==='account') {
    app.append(link('‹ '+t('mobile.backConnections'),'#/'),el('h1',route.page==='remote'?t('mobile.myDevices'):t('mobile.account')));
    const panel=el('section',undefined,'remote-account-card');app.append(panel);
    if(route.page==='account') app.append(link(t('mobile.pushTitle'),'#/notifications'));
    let linked=false;let accountKnown=false;let pending=false;
    try {
      const result=await Remote.account({action:'status'});
      if(generation!==renderGeneration)return;
      linked=!!result.linked;pending=!!result.pending;accountKnown=true;
      if(linked) {
        panel.append(el('h2',result.account?.displayName ?? 'VelaTerm'),el('p',t('mobile.signedInHint'),'muted'));
        if(route.page==='account') panel.append(button(t('mobile.manageAccount'),()=>void Remote.account({action:'open'}).catch(e=>status(String(e),true))),button(t('mobile.signOut'),()=>void Remote.account({action:'logout'}).then(()=>{accountLogin.reset();return render()}).catch(e=>status(String(e),true))),link(t('mobile.viewMyDevices'),'#/remote'));
        else {
          const list=el('div',undefined,'remote-client-list');app.append(list);
          const refresh=async()=>{
            try {
              const {devices=[]}=await Remote.account({action:'devices'});
              if(!list.isConnected)return;list.replaceChildren();
              if(!devices.length)list.append(el('p',t('mobile.noDevices')));
              for(const device of devices){
                const card=el('section',undefined,'remote-client');card.append(el('h2',device.name),el('p',t(device.online?'mobile.online':'mobile.offline'),'muted'));
                if(!device.access.length){card.append(el('p',t('mobile.deviceNotSharing'),'muted'));list.append(card);continue;}
                const scopes=el('ul',undefined,'remote-client-shares');
                for(const scope of device.access)scopes.append(el('li',scope.scope==='machine'?t('mobile.scopeMachine'):scope.name===scope.scope?t(scope.scope==='project'?'mobile.scopeProject':'mobile.scopeSession'):scope.name));
                card.append(scopes);
                if(!device.online || !device.sharing){card.append(el('p',t(device.online?'mobile.sharingNotReady':'mobile.deviceOffline'),'muted'));list.append(card);continue;}
                const open=button(t('mobile.viewShared'),async()=>{
                  open.disabled=true;
                  try{await Remote.account({action:'open',deviceId:device.id})}catch(error){status(String(error),true)}finally{open.disabled=false}
                });
                card.append(open);list.append(card);
              }
            }catch(error){if(list.isConnected){list.replaceChildren(el('p',t('mobile.devicesUnavailable')));status(String(error),true);}}
            if(list.isConnected)setTimeout(()=>void refresh(),5000);
          };void refresh();
        }
      }
    }catch(error){panel.append(el('p',t('mobile.accountUnavailable')),button(t('common.retry'),()=>void render()));notice.textContent=String(error);notice.classList.add('error');}
    if(!linked && accountKnown) {
      panel.append(el('h2',t('mobile.signInTitle')),el('p',t('mobile.signInHint')));
      const signIn=button(t('mobile.signIn'),()=>void accountLogin.start(),'primary');
      const progress=el('p');progress.setAttribute('role','status');progress.setAttribute('aria-live','polite');
      const checkLogin=button(t('mobile.checkSignIn'),()=>void accountLogin.check());
      panel.append(signIn,progress,checkLogin);
      const update=()=>{
        signIn.disabled=accountLogin.active;signIn.textContent=t(accountLogin.active?'mobile.waitingSignIn':'mobile.signIn');
        progress.textContent=accountLogin.state.message?t(accountLogin.state.message):accountLogin.state.error??'';progress.hidden=!progress.textContent;
        progress.classList.toggle('error',accountLogin.state.phase==='error');checkLogin.hidden=!accountLogin.active;
        if(accountLogin.state.phase==='linked')void render();
      };
      releaseLoginView=accountLogin.subscribe(update);
      if(!pending && !accountLogin.active)accountLogin.reset();
      update();if(pending)void accountLogin.resume();
    }
  } else if(route.page==='list') {
    app.append(el('h1',t('mobile.workspaceTitle')),el('p',t('mobile.workspaceHint'),'muted'));
    const actions=el('nav',undefined,'actions');actions.append(link(t('mobile.newSsh'),'#/connections/new?mode=ssh'),link(t('mobile.newUrl'),'#/connections/new?mode=url'),link(t('mobile.remote'),'#/remote'),link(t('mobile.scanToConnect'),'#/connections/new?mode=url&scan=1'));app.append(actions);
    if(!records.length) app.append(el('section',t('mobile.noConnections'),'empty'));
    for(const row of records) {
      const card=el('article',undefined,'connection-card');
      const open=button('',()=>void connect(row.id!),'connection-open');open.dataset.connect='true';
      let address=row.mode==='ssh'?`${row.username}@${row.host}:${row.port}`:row.url ?? '';
      if(row.mode==='url'){try{const url=new URL(address);address=url.origin+url.pathname}catch{/* Keep an unparseable legacy address so the user can edit it. */}}
      open.append(el('span',row.name,'connection-name'),el('span',address,'connection-address muted'),el('span',t('mobile.tapToConnect'),'connection-action'));
      card.append(open);
      if(row.hasWebPassword)card.append(el('p',t('mobile.webPasswordSaved'),'saved-secret hint'));
      const controls=el('div',undefined,'controls');controls.append(el('span',row.mode.toUpperCase(),'badge'),link(t('common.edit'),editHref(row.id!)),link(t('mobile.copyConnection'),copyHref(row.id!)),button(t('common.delete'),()=>{
        const dialog=el('dialog');dialog.append(el('h2',t('mobile.deleteConnectionTitle')),el('p',t('mobile.deleteConnectionConfirm',row.name)),button(t('common.cancel'),()=>dialog.remove()),button(t('common.delete'),()=>{void Remote.remove({id:row.id!}).then(()=>{dialog.remove();return render()}).catch(e=>status(String(e),true))}));app.append(dialog);dialog.showModal();
      }));card.append(controls);app.append(card);
    }
  } else {
    const sourceId=route.copyFromId ?? route.id;
    const row=records.find(r=>r.id===sourceId);if(sourceId && !row){app.append(el('p',t('mobile.connectionMissing')),link(t('mobile.backConnections'),'#/'));return}
    const mode=row?.mode ?? route.mode;
    app.append(link('‹ '+t('mobile.backConnections'),'#/'),el('h1',t(route.copyFromId?'mobile.copyConnection':row?'mobile.editConnection':mode==='ssh'?'mobile.addSshHost':'mobile.addUrlConnection')));
    if(route.copyFromId)app.append(el('p',t('mobile.copyConnectionHint'),'hint'));
    const form=el('form');const nameInput=field(form,t('mobile.connectionName'),'name',row?.name);nameInput.required=true;
    if(mode==='url') {
      const urlInput=field(form,t('mobile.serviceUrl'),'url',row?.url,'url','https://your-server.example');urlInput.required=true;
      const scan=button(t('mobile.scanToFill'),()=>void scanURL());scan.className='scan-button';form.append(scan);
      async function scanURL() {
        if(scanning || busy)return;
        scanning=true;scan.disabled=true;form.querySelectorAll<HTMLButtonElement>('button[type=submit]').forEach(b=>b.disabled=true);
        status(t('mobile.openingCamera'));
        try {
          const result=await Remote.scanURL();
          if(!form.isConnected)return;
          if('cancelled' in result){status(t('mobile.scanCancelled'));return}
          urlInput.value=result.url;if(!nameInput.value.trim())nameInput.value=result.name;
          status(t('mobile.scanDone'));
        } catch(error) {if(form.isConnected)status(Capacitor.isNativePlatform()?String(error):t('mobile.scanNativeOnly'),true)}
        finally {scanning=false;scan.disabled=false;form.querySelectorAll<HTMLButtonElement>('button[type=submit]').forEach(b=>b.disabled=busy)}
      }
      if(route.scan) {
        history.replaceState(null,'','#/connections/new?mode=url');
        setTimeout(()=>{if(form.isConnected)void scanURL()},0);
      }
      field(form,t('mobile.webPasswordOptional'),'webPassword','','password',t(row?'mobile.keepPassword':'mobile.webPasswordLater'));
      form.append(el('p',t(row?.hasWebPassword?'mobile.webPasswordSavedHint':'mobile.webPasswordStorageHint'),'hint'));
    } else {
      field(form,t('mobile.sshHost'),'host',row?.host,'text',t('mobile.sshHostPlaceholder')).required=true;
      field(form,t('mobile.sshPort'),'port',String(row?.port ?? 22),'number').required=true;
      field(form,t('mobile.username'),'username',row?.username).required=true;
      const auth=select(form,t('mobile.authMethod'),'auth',[['password',t('mobile.authPassword')],['key',t(Capacitor.getPlatform()==='android'?'mobile.authKeyAndroid':'mobile.authKey')]],row?.auth ?? 'password');
      const password=field(form,t('mobile.sshPassword'),'password','','password',row?t('mobile.keepPassword'):'');
      const keyWrap=el('label',t('mobile.privateKey'));const key=el('textarea');key.name='privateKey';key.rows=6;key.spellcheck=false;key.placeholder=t(row?'mobile.keepPrivateKey':'mobile.pastePrivateKey');keyWrap.append(key);form.append(keyWrap);
      const pass=field(form,t('mobile.passphraseOptional'),'passphrase','','password',row?t('mobile.keepPassphrase'):'');
      if(row?.hasSecret)form.append(el('p',t('mobile.sshSecretSavedHint'),'hint'));
      const updateAuth=()=>{password.parentElement!.hidden=auth.value!=='password';keyWrap.hidden=pass.parentElement!.hidden=auth.value!=='key'};auth.onchange=updateAuth;updateAuth();
      const service=select(form,t('mobile.remoteService'),'service',[['auto',t('mobile.serviceAuto')],['manual',t('mobile.serviceManual')]],row?.service ?? 'auto');
      const remotePort=field(form,t('mobile.remotePort'),'remotePort',row?.remotePort?String(row.remotePort):'','number');
      const webPassword=field(form,t('mobile.webPasswordOptional'),'webPassword','','password',row?t('mobile.keepPassword'):'');
      if(row?.hasWebPassword)form.append(el('p',t('mobile.webPasswordAutoHint'),'hint'));
      const prepare=check(form,t('mobile.prepareService'),'prepare',row?.prepare ?? false);
      const detail=el('p',t('mobile.prepareServiceHint'),'hint');form.append(detail);
      const updateService=()=>{remotePort.parentElement!.hidden=webPassword.parentElement!.hidden=service.value!=='manual';prepare.parentElement!.hidden=detail.hidden=service.value!=='auto'};service.onchange=updateService;updateService();
    }
    const saves=el('div',undefined,'save-actions');
    const save=el('button',t('mobile.saveConnection'));save.type='submit';saves.append(save);
    if(mode==='url'){const open=el('button',t('mobile.saveAndConnect'),'primary');open.type='submit';open.value='connect';saves.append(open)}
    if(route.copyFromId)saves.append(link(t('common.cancel'),'#/'));
    form.append(saves);
    form.onsubmit=e=>{e.preventDefault();if(busy || scanning)return;const openAfter=(e.submitter as HTMLButtonElement | null)?.value==='connect';const values=new FormData(form);const data:Connection={...(!route.copyFromId && row?{id:row.id}:{}),name:String(values.get('name')??'').trim(),mode};
      if(mode==='url') data.url=String(values.get('url')).trim();
      else Object.assign(data,{host:String(values.get('host')).trim(),port:Number(values.get('port')),username:String(values.get('username')).trim(),auth:values.get('auth'),service:values.get('service'),remotePort:Number(values.get('remotePort')),prepare:values.get('prepare')==='on'});
      for(const name of ['password','privateKey','passphrase','webPassword'] as const){const value=String(values.get(name)??'');if(value || !row)data[name]=value}
      lock(true);void Remote.save({connection:data,...(route.copyFromId?{copyFromId:route.copyFromId}:{})}).then(async result=>{lock(false);history.pushState(null,'','#/');await render();if(openAfter)await connect(result.connection.id!);else if(route.copyFromId && records.some(r=>r.id===result.connection.id && r.id===sourceId))status(t('mobile.copyConnectionReused'))}).catch(e=>{lock(false);status(String(e),true)});
    };app.append(form);
  }
  if(generation===renderGeneration)app.append(footer);
}
window.addEventListener('hashchange',()=>void render());
document.addEventListener('visibilitychange',()=>{if(!document.hidden)void accountLogin.check()});
window.addEventListener('focus',()=>void accountLogin.check());
if(Capacitor.isNativePlatform()) void Remote.addListener('state',event=>{
  if(event.phase==='disconnected' && !busy && !scanning && routeFromHash(location.hash).page==='list')void render().then(()=>status(t(phases.disconnected)));
  else {const phase=phases[event.phase];status(phase?t(phase):event.phase,event.phase==='error')}
});
const initialized = initI18n().then(()=>{document.documentElement.lang=getLocale();return render()});
if(Capacitor.isNativePlatform()) void Remote.addListener('notificationOpen',async event=>{
  try {
    await notificationNavigation.open(event, {
      cancelConnection: () => { connectionAttempt.cancel(); releaseConnectionLoading(); lock(false); },
      disconnect: () => Remote.disconnect(),
      showConnections: async () => { await initialized; history.pushState(null,'','#/'); await render(); },
      connect: target => connect(target.id, target.sessionId),
    });
  } catch(error) {status(String(error),true)}
});
