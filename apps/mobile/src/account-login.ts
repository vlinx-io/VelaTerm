import type { I18nKey } from '../../../src/i18n';
export interface LoginResult {linked?:boolean;pending?:boolean}
export type LoginPhase='idle'|'starting'|'waiting'|'retrying'|'linked'|'error';
/** `message` is an i18n key the view translates; `error` carries raw error text from the native side. */
export interface LoginState {phase:LoginPhase;message:I18nKey|'';error?:string}
type AccountRequest=(action:'login'|'poll')=>Promise<LoginResult>;

/** Keep authorization polling alive when the user leaves the account page or returns from the browser. */
export class AccountLogin {
  state:LoginState={phase:'idle',message:''};
  private generation=0;
  private timer:ReturnType<typeof setTimeout>|undefined;
  private polling=false;
  private listeners=new Set<(state:LoginState)=>void>();
  private request:AccountRequest;
  private retryDelay:number;
  constructor(request:AccountRequest,retryDelay=2000) {this.request=request;this.retryDelay=retryDelay}
  get active() {return ['starting','waiting','retrying'].includes(this.state.phase)}
  subscribe(listener:(state:LoginState)=>void) {this.listeners.add(listener);return ()=>{this.listeners.delete(listener)}}
  private update(phase:LoginPhase,message:I18nKey|''='',error?:string) {this.state={phase,message,error};for(const listener of this.listeners)listener(this.state)}
  reset() {this.generation++;clearTimeout(this.timer);this.timer=undefined;this.polling=false;this.update('idle')}
  async start() {
    if(this.active)return;
    this.reset();const generation=this.generation;this.update('starting','mobile.loginOpening');
    try {
      await this.request('login');
      if(generation!==this.generation)return;
      this.update('waiting','mobile.loginFinishInBrowser');
      await this.check();
    }catch(error){if(generation===this.generation)this.update('error','',error instanceof Error?error.message:String(error))}
  }
  resume() {
    if(!this.active)this.update('waiting','mobile.loginChecking');
    return this.check();
  }
  async check() {
    if(!this.active || this.state.phase==='starting' || this.polling)return;
    clearTimeout(this.timer);this.timer=undefined;this.polling=true;
    const generation=this.generation;
    try {
      const result=await this.request('poll');
      if(generation!==this.generation)return;
      if(result.linked){this.update('linked','mobile.loginSuccess');return;}
      this.update('waiting','mobile.loginWaiting');
    }catch(error){
      if(generation!==this.generation)return;
      const code=(error as {code?:string})?.code;
      if(code==='ACCOUNT_LOGIN_EXPIRED' || code==='ACCOUNT_AUTH_REQUIRED') {
        this.update('error','mobile.loginExpired');return;
      }
      this.update('retrying','mobile.loginRetrying');
    }finally{
      if(generation===this.generation){
        this.polling=false;
        if(this.active)this.timer=setTimeout(()=>void this.check(),this.retryDelay);
      }
    }
  }
}
