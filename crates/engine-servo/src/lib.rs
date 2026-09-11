//! Servo adapter boundary.
//!
//! `ServoEngine` deliberately owns the engine-facing state and exposes only
//! `browsai-engine-api` types. The deterministic backend is useful for contract
//! tests and keeps the workspace usable on machines without a graphics stack.
//! The `servo-runtime` feature reserves the dependency boundary for the real
//! in-process Servo embedder.

use browsai_agent_tree::AgentRenderTree;
#[cfg(feature = "servo-runtime")]
use browsai_agent_tree::{AgentNode, AgentValue, NodeState, StructuralRole};
use browsai_engine_api::{
    BrowserEngine, ContextId, ContextOptions, EngineCapabilities, EngineError, EngineFeature,
    NavigationHandle, PageId, PageScriptResult, ScriptSource,
};
use browsai_input::NativeInputEvent;
#[cfg(feature = "servo-runtime")]
use browsai_provenance::{Confidence, ProvenanceSource, SourceKind};
use browsai_state::{PageSnapshot, PageState};
use std::collections::HashMap;
use url::Url;

/// A feature-gated handle to the actual Servo event-loop runtime.
///
/// `ServoEngine` remains deterministic by default for contract tests. Applications that need
/// browser-backed execution can opt into this handle and drive `spin_event_loop` from their host
/// event loop. Keeping this object separate prevents renderer state from leaking into the agent
/// model or silently changing deterministic tests.
#[cfg(feature = "servo-runtime")]
pub struct ServoRuntime {
    inner: servo::Servo,
    identity: browsai_engine_api::ProfileIdentity,
}

#[cfg(feature = "servo-runtime")]
fn build_navigator_identity_script(identity: &browsai_engine_api::ProfileIdentity) -> String {
    fn js_string(value: &str) -> String {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\"{}\"", escaped)
    }
    let ua = js_string(&identity.user_agent);
    let platform = js_string(&identity.platform);
    let vendor = js_string("Google Inc.");
    let brands_json = identity
        .brands
        .iter()
        .map(|brand| {
            format!(
                "{{brand:{},version:{}}}",
                js_string(&brand.brand),
                js_string(&brand.version)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mobile = identity.sec_ch_ua_mobile();
    format!(
        r#"(function(){{try{{if(typeof navigator==='undefined')return;function setProp(name,value){{try{{Object.defineProperty(navigator,name,{{configurable:true,get:function(){{return value;}}}});}}catch(_){{try{{navigator[name]=value;}}catch(__){{}}}}}}setProp('userAgent',{ua});setProp('platform',{platform});setProp('vendor',{vendor});var ua=String({ua});if(ua&&(!navigator.appVersion||navigator.appVersion.indexOf('Chrome/')<0)){{setProp('appVersion',ua.replace(/^Mozilla\/5\.0\s*/,''));}}if(!navigator.userAgentData){{var brands=[{brands_json}];var platform={platform};var mobile={mobile};var data={{brands:brands,mobile:mobile==='?1',platform:platform,getHighEntropyValues:function(hints){{var result={{brands:brands,mobile:mobile==='?1',platform:platform}};if(Array.isArray(hints)){{if(hints.indexOf('architecture')>=0)result.architecture='x86';if(hints.indexOf('bitness')>=0)result.bitness='64';if(hints.indexOf('model')>=0)result.model='';if(hints.indexOf('platformVersion')>=0)result.platformVersion='0.0.0';if(hints.indexOf('uaFullVersion')>=0){{var match=/Chrome\/(\d+)/.exec(ua);result.uaFullVersion=(match?match[1]:'140')+'.0.0.0';}}}}return Promise.resolve(result);}},toJSON:function(){{return {{brands:brands,mobile:mobile==='?1',platform:platform}};}}}};try{{Object.defineProperty(navigator,'userAgentData',{{configurable:true,value:data}});}}catch(_){{}}}}if(typeof window!=='undefined'&&!window.chrome)window.chrome={{runtime:{{}}}};}}catch(_){{}}}}());"#,
        ua = ua,
        platform = platform,
        vendor = vendor,
        brands_json = brands_json,
        mobile = mobile,
    )
}

#[cfg(feature = "servo-runtime")]
fn build_screen_identity_script(viewport: browsai_engine_api::VirtualViewport) -> String {
    format!(
        r#"(function(){{try{{if(typeof screen==='undefined')return;var w={w};var h={h};function setProp(name,value){{try{{Object.defineProperty(screen,name,{{configurable:true,get:function(){{return value;}}}});}}catch(_){{try{{screen[name]=value;}}catch(__){{}}}}}}if(!screen.width||!screen.height){{setProp('width',w);setProp('height',h);setProp('availWidth',w);setProp('availHeight',h-Math.min(40,Math.floor(h*0.05)));setProp('colorDepth',24);setProp('pixelDepth',24);}}if(typeof screen.orientation==='undefined'){{setProp('orientation',{{type:'landscape-primary',angle:0,onchange:null,lock:function(){{return Promise.resolve();}},unlock:function(){{}},addEventListener:function(){{}},removeEventListener:function(){{}},dispatchEvent:function(){{return false;}}}});}}}}catch(_){{}}}})();"#,
        w = viewport.width.max(1),
        h = viewport.height.max(1),
    )
}

#[cfg(feature = "servo-runtime")]
pub struct ServoRuntimePage {
    webview: servo::WebView,
    // Keep the WebView before the Servo handle so Rust drops the WebView first.
    // WebViewInner sends CloseWebView; Servo's final drop then pumps that close
    // through the constellation instead of waiting on an active page forever.
    runtime: servo::Servo,
    _rendering_context: std::rc::Rc<servo::SoftwareRenderingContext>,
    last_point: std::cell::Cell<servo::WebViewPoint>,
    load_status: std::cell::RefCell<String>,
    load_elapsed_millis: std::cell::Cell<u64>,
    delegate: std::rc::Rc<RuntimeWebViewDelegate>,
}

#[cfg(feature = "servo-runtime")]
#[derive(Default)]
struct RuntimeWebViewDelegate {
    console: std::cell::RefCell<Vec<String>>,
    crashes: std::cell::RefCell<Vec<String>>,
    history: std::cell::RefCell<Vec<Url>>,
    main_frame_requests: std::cell::RefCell<Vec<Url>>,
    main_frame_requests_truncated: std::cell::Cell<bool>,
    resource_requests: std::cell::RefCell<Vec<Url>>,
    resource_requests_truncated: std::cell::Cell<bool>,
}

#[cfg(feature = "servo-runtime")]
const MAIN_FRAME_REQUEST_LIMIT: usize = 256;
const RESOURCE_REQUEST_LIMIT: usize = 2048;
const RUNTIME_CONSOLE_LIMIT: usize = 512;
const RUNTIME_CONSOLE_BYTES_LIMIT: usize = 256 * 1024;

#[cfg(feature = "servo-runtime")]
const CACHE_STORAGE_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  // The same compatibility surface is needed in window and worker globals;
  // blob-backed workers expose `self` rather than `window`.
  var window=typeof window!=='undefined'?window:(typeof self!=='undefined'?self:null);
  if(!window)return;
  var stores=window.__browsaiCacheStores||Object.create(null);
  window.__browsaiCacheStores=stores;
  function install(target,name,fn){
    if(!target||typeof target[name]==='function')return;
    try{Object.defineProperty(target,name,{configurable:true,writable:true,value:fn});}
    catch(_){try{target[name]=fn;}catch(__){}}
  }
  function cache(name){
    if(stores[name])return stores[name];
    var entries=Object.create(null);
    function key(request){
      if(typeof request==='string')return request;
      if(request&&typeof request.url==='string')return request.url;
      return String(request);
    }
    function copy(response){
      try{return response&&typeof response.clone==='function'?response.clone():response;}catch(_){return response;}
    }
    function lookup(request){
      var stored=entries[key(request)];
      return stored===undefined?undefined:copy(stored);
    }
    var value={
      match:function(request){return Promise.resolve(lookup(request));},
      matchAll:function(request){var hit=lookup(request);return Promise.resolve(hit===undefined?[]:[hit]);},
      add:function(request){return Promise.reject(new TypeError('Cache.add requires a response'));},
      addAll:function(){return Promise.reject(new TypeError('Cache.addAll requires responses'));},
      put:function(request,response){entries[key(request)]=response;return Promise.resolve(undefined);},
      delete:function(request){var k=key(request),present=Object.prototype.hasOwnProperty.call(entries,k);if(present)delete entries[k];return Promise.resolve(present);},
      keys:function(){var keys=Object.keys(entries);return Promise.resolve(keys.map(function(k){try{return new Request(k);}catch(_){return k;}}));},
      __lookup:function(request){return lookup(request);}
    };
    if(window.Cache&&window.Cache.prototype)try{Object.setPrototypeOf(value,window.Cache.prototype);}catch(_){ }
    stores[name]=value;
    return value;
  }
  // Some applications probe the Cache interface directly (for example with
  // `instanceof Cache`) even when they only use the CacheStorage facade.
  // Expose the interface identity without claiming a persistent/native cache
  // backend; the actual objects remain the bounded page-local stores above.
  if(typeof window.Cache==='undefined'){
    try{
      window.Cache=function Cache(){};
      window.Cache.prototype.constructor=window.Cache;
      Object.keys(stores).forEach(function(name){try{Object.setPrototypeOf(stores[name],window.Cache.prototype);}catch(_){}});
    }catch(_){ }
  }
  var nativeStorage=window.caches;
  // Servo currently exposes CacheStorage.has() on a native, non-extensible
  // object. Installing the missing methods directly therefore fails silently.
  // Use a JS facade with the native object as its prototype so existing native
  // methods remain available while the compatibility methods are own members.
  var storage=nativeStorage?Object.create(nativeStorage):{};
  install(storage,'open',function(name){return Promise.resolve(cache(String(name)));});
  install(storage,'match',function(request){var names=Object.keys(stores);return Promise.resolve().then(function(){for(var i=0;i<names.length;i++){var hit=stores[names[i]].__lookup(request);if(hit!==undefined)return hit;}return undefined;});});
  install(storage,'delete',function(){return Promise.resolve(false);});
  install(storage,'keys',function(){return Promise.resolve(Object.keys(stores));});
  // Some Servo builds expose `window.caches` as a non-extensible native
  // object, so the facade property replacement above may not take effect.
  // Patch its prototype as a second path; this is still page-local and keeps
  // cache misses truthful rather than fabricating stored responses.
  var nativePrototype=null;
  try{nativePrototype=nativeStorage&&Object.getPrototypeOf(nativeStorage);}catch(_){ }
  if(nativePrototype){
    install(nativePrototype,'open',function(name){return Promise.resolve(cache(String(name)));});
    install(nativePrototype,'match',function(request){var names=Object.keys(stores);return Promise.resolve().then(function(){for(var i=0;i<names.length;i++){var hit=stores[names[i]].__lookup(request);if(hit!==undefined)return hit;}return undefined;});});
    install(nativePrototype,'delete',function(){return Promise.resolve(false);});
    install(nativePrototype,'keys',function(){return Promise.resolve(Object.keys(stores));});
  }
  try{Object.defineProperty(window,'caches',{configurable:true,writable:true,value:storage});}catch(_){try{window.caches=storage;}catch(__){}}
})();
"#;

#[cfg(feature = "servo-runtime")]
const API_SURFACE_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  function install(target,name,fn){
    if(!target||typeof target[name]==='function')return;
    try{Object.defineProperty(target,name,{configurable:true,writable:true,value:fn});}
    catch(_){try{target[name]=fn;}catch(__){}}
  }
  // Event methods are ordinary writable prototype methods in Chromium. Some
  // Servo-generated descriptors are read-only, which breaks vendor wrappers
  // that temporarily replace stopImmediatePropagation in strict mode.
  if(typeof Event!=='undefined'&&Event.prototype){
    ['stopImmediatePropagation','stopPropagation','preventDefault'].forEach(function(name){
      try{var descriptor=Object.getOwnPropertyDescriptor(Event.prototype,name);if(descriptor&&typeof descriptor.value==='function'&&(!descriptor.writable||!descriptor.configurable))Object.defineProperty(Event.prototype,name,{configurable:true,writable:true,value:descriptor.value});}catch(_){}
    });
  }
  if(typeof console!=='undefined')
    install(console,'table',function(value){if(typeof console.log==='function')console.log(value);});
  // Servo's selector parser may reject otherwise valid relational selectors
  // containing :has(). Preserve native selector behavior first, then provide
  // a small fallback for the common simple form `base:has(descendant)` used
  // by framework migrations and component libraries.
  function hasFallback(context,selector,all,nativeAll,nativeOne){
    if(typeof selector!=='string'||selector.indexOf(':has(')<0)throw new SyntaxError('Unsupported selector');
    var groups=[],start=0,depth=0;
    for(var i=0;i<selector.length;i++){var ch=selector.charAt(i);if(ch==='(')depth++;else if(ch===')')depth--;else if(ch===','&&depth===0){groups.push(selector.slice(start,i).trim());start=i+1;}}
    groups.push(selector.slice(start).trim());
    var result=[];
    groups.forEach(function(group){var match=/^(.*):has\(([^()]*)\)$/.exec(group);if(!match)throw new SyntaxError('Unsupported selector');var candidates=nativeAll.call(context,match[1]||'*');for(var j=0;j<candidates.length;j++){if(nativeOne.call(candidates[j],match[2]))result.push(candidates[j]);}});
    return all?result:(result.length?result[0]:null);
  }
  function installHasFallback(ctor){try{var proto=ctor&&ctor.prototype;if(!proto||typeof proto.querySelector!=='function'||typeof proto.querySelectorAll!=='function')return;var nativeOne=proto.querySelector,nativeAll=proto.querySelectorAll,elementOne=typeof Element!=='undefined'&&Element.prototype&&typeof Element.prototype.querySelector==='function'?Element.prototype.querySelector:nativeOne;Object.defineProperty(proto,'querySelectorAll',{configurable:true,writable:true,value:function(selector){try{return nativeAll.call(this,selector);}catch(error){return hasFallback(this,selector,true,nativeAll,elementOne);}}});Object.defineProperty(proto,'querySelector',{configurable:true,writable:true,value:function(selector){try{return nativeOne.call(this,selector);}catch(error){return hasFallback(this,selector,false,nativeAll,elementOne);}}});}catch(_){}}
  installHasFallback(typeof Element!=='undefined'?Element:null);
  installHasFallback(typeof Document!=='undefined'?Document:null);
  installHasFallback(typeof DocumentFragment!=='undefined'?DocumentFragment:null);
  // jQuery-style feature probes commonly use Chrome's autofill pseudo-class.
  // Servo does not implement that optional pseudo-class yet; an unsupported
  // autofill probe should behave as an empty match, not abort site startup.
  function installAutofillSelectorFallback(ctor){try{var proto=ctor&&ctor.prototype;if(!proto||typeof proto.querySelector!=='function'||typeof proto.querySelectorAll!=='function')return;var one=proto.querySelector,all=proto.querySelectorAll;Object.defineProperty(proto,'querySelectorAll',{configurable:true,writable:true,value:function(selector){try{return all.call(this,selector);}catch(error){if(typeof selector==='string'&&selector.indexOf(':-webkit-autofill')>=0){try{return all.call(this,':not(*)');}catch(_){return [];}}throw error;}}});Object.defineProperty(proto,'querySelector',{configurable:true,writable:true,value:function(selector){try{return one.call(this,selector);}catch(error){if(typeof selector==='string'&&selector.indexOf(':-webkit-autofill')>=0)return null;throw error;}}});}catch(_){} }
  installAutofillSelectorFallback(typeof Element!=='undefined'?Element:null);
  installAutofillSelectorFallback(typeof Document!=='undefined'?Document:null);
  installAutofillSelectorFallback(typeof DocumentFragment!=='undefined'?DocumentFragment:null);
  if(typeof IDBIndex!=='undefined'&&IDBIndex.prototype){
    install(IDBIndex.prototype,'openCursor',function(query,direction){
      return this.objectStore.openCursor(query,direction||'next');
    });
    install(IDBIndex.prototype,'openKeyCursor',function(query,direction){
      return this.objectStore.openKeyCursor(query,direction||'next');
    });
    install(IDBIndex.prototype,'count',function(query){
      var source=this,request={result:null,error:null,readyState:'pending',onsuccess:null,onerror:null,_listeners:{}};
      request.addEventListener=function(type,fn){if(typeof fn==='function')(request._listeners[type]||(request._listeners[type]=[])).push(fn);};
      request.removeEventListener=function(type,fn){var list=request._listeners[type]||[];request._listeners[type]=list.filter(function(item){return item!==fn;});};
      function fire(type){var event={type:type,target:request,currentTarget:request},handler=request['on'+type];if(typeof handler==='function')handler.call(request,event);(request._listeners[type]||[]).slice().forEach(function(fn){fn.call(request,event);});}
      var cursorRequest,total=0;
      try{cursorRequest=source.openCursor(query);}catch(error){request.error=error;request.readyState='done';window.setTimeout(function(){fire('error');},0);return request;}
      cursorRequest.onsuccess=function(){var cursor=cursorRequest.result;if(!cursor){request.result=total;request.readyState='done';fire('success');return;}total++;try{cursor.continue();}catch(error){request.error=error;request.readyState='done';fire('error');}};
      cursorRequest.onerror=function(){request.error=cursorRequest.error;request.readyState='done';fire('error');};
      return request;
    });
  }
  // Servo's generated IDBCursorWithValue prototype may not expose inherited
  // IDBCursor members even though the underlying object implements them.
  // Bridge only missing members so native cursor behavior remains authoritative.
  if(typeof IDBCursor!=='undefined'&&IDBCursor.prototype&&typeof IDBCursorWithValue!=='undefined'&&IDBCursorWithValue.prototype){
    ['advance','continue','continuePrimaryKey'].forEach(function(name){
      if(typeof IDBCursorWithValue.prototype[name]!=='function'&&typeof IDBCursor.prototype[name]==='function')
        install(IDBCursorWithValue.prototype,name,IDBCursor.prototype[name]);
    });
  }
  function param(value){return {value:value||0,setValueAtTime:function(){return this;},linearRampToValueAtTime:function(){return this;},exponentialRampToValueAtTime:function(){return this;},setTargetAtTime:function(){return this;},setValueCurveAtTime:function(){return this;},cancelScheduledValues:function(){return this;},cancelAndHoldAtTime:function(){return this;}};}
  function compressor(){
    var node=null;
    try{if(this&&typeof this.createGain==='function')node=this.createGain();}catch(_){}
    if(!node)node={connect:function(){return arguments[0]||this;},disconnect:function(){}};
    try{Object.defineProperty(node,'threshold',{configurable:true,value:param(-24)});Object.defineProperty(node,'knee',{configurable:true,value:param(30)});Object.defineProperty(node,'ratio',{configurable:true,value:param(12)});Object.defineProperty(node,'attack',{configurable:true,value:param(0)});Object.defineProperty(node,'release',{configurable:true,value:param(0.25)});Object.defineProperty(node,'reduction',{configurable:true,value:0});}catch(_){ }
    return node;
  }
  ['AudioContext','OfflineAudioContext'].forEach(function(name){
    if(typeof window[name]!=='undefined'&&window[name].prototype)
      install(window[name].prototype,'createDynamicsCompressor',compressor);
  });
  if(typeof CanvasRenderingContext2D!=='undefined'&&CanvasRenderingContext2D.prototype)
    install(CanvasRenderingContext2D.prototype,'roundRect',function(x,y,w,h){
      if(typeof this.rect==='function')this.rect(x,y,w,h);
      return this;
    });
  if(typeof CSSStyleDeclaration!=='undefined'&&CSSStyleDeclaration.prototype){
    ['gridTemplateColumns','gridTemplateRows','gridAutoFlow'].forEach(function(name){
      if(Object.getOwnPropertyDescriptor(CSSStyleDeclaration.prototype,name))return;
      try{Object.defineProperty(CSSStyleDeclaration.prototype,name,{configurable:true,get:function(){return '';},set:function(){}});}catch(_){ }
    });
  }
  // CSSOM insertRule() must return the index at which the rule was inserted.
  // A few shadow-style-sheet paths in Servo mutate cssRules but return an
  // undefined result, which breaks Media Chrome's rule lookup. Preserve the
  // native implementation and repair only that missing return value.
  function repairInsertRule(ctor){try{var proto=ctor&&ctor.prototype;if(!proto||typeof proto.insertRule!=='function')return;var native=proto.insertRule;Object.defineProperty(proto,'insertRule',{configurable:true,writable:true,value:function(rule,index){var before=0;try{before=this.cssRules?this.cssRules.length:0;}catch(_){ }var result=native.call(this,rule,index);if(typeof result==='number')return result;try{var after=this.cssRules;if(after&&after.length>before)return Math.min(Number(index)||0,after.length-1);}catch(_){ }return result;}});}catch(_){}}
  repairInsertRule(typeof CSSStyleSheet!=='undefined'?CSSStyleSheet:null);
  repairInsertRule(typeof CSSGroupingRule!=='undefined'?CSSGroupingRule:null);
  if(typeof window.visualViewport==='undefined'){
    try{Object.defineProperty(window,'visualViewport',{configurable:true,value:{get width(){return window.innerWidth||0;},get height(){return window.innerHeight||0;},offsetLeft:0,offsetTop:0,pageLeft:0,pageTop:0,scale:1,addEventListener:function(){},removeEventListener:function(){},dispatchEvent:function(){return false;}}});}catch(_){ }
  }
  if(typeof window.Notification==='undefined'){
    window.Notification=function(title,options){this.title=String(title||'');this.body=options&&options.body||'';this.tag=options&&options.tag||'';this.close=function(){};};
    window.Notification.permission='default';
    window.Notification.requestPermission=function(){return Promise.resolve('default');};
  }
  // Expose the read-only Feature Policy shape used by ad/media bundles. The
  // headless embedder does not grant optional policy-controlled features, so
  // queries remain truthful and return denied/empty results rather than
  // throwing because the interface is absent.
  try{if(typeof Document!=='undefined'&&Document.prototype&&!Object.getOwnPropertyDescriptor(Document.prototype,'featurePolicy')){
    var policy={allowsFeature:function(){return false;},allowedFeatures:function(){return [];},features:function(){return [];},getAllowlistForFeature:function(){return [];},getAttribute:function(){return null;}};
    Object.defineProperty(Document.prototype,'featurePolicy',{configurable:true,get:function(){return policy;}});
  }}catch(_){ }
  // Servo may omit SVG's constructor alias even though SVG elements exist.
  // Preserve feature detection and instanceof checks without inventing a
  // separate implementation; SVGElement is the safest available base.
  if(typeof window.SVGAElement==='undefined'&&typeof window.SVGElement==='function'){
    try{Object.defineProperty(window,'SVGAElement',{configurable:true,writable:true,value:window.SVGElement});}
    catch(_){try{window.SVGAElement=window.SVGElement;}catch(__){}}
  }
  // SVGScriptElement is a standard constructor used by vendor feature
  // detection. Servo may omit only this alias; keep instanceof checks
  // well-defined without exposing host state.
  if(typeof window.SVGScriptElement==='undefined'&&typeof window.SVGElement==='function'){
    try{Object.defineProperty(window,'SVGScriptElement',{configurable:true,writable:true,value:window.SVGElement});}
    catch(_){try{window.SVGScriptElement=window.SVGElement;}catch(__){}}
  }
  if(typeof window.cookieStore==='undefined'){
    try{Object.defineProperty(window,'cookieStore',{configurable:true,value:{__browsaiCookieStore:true,get:function(){return Promise.resolve(undefined);},getAll:function(){return Promise.resolve([]);},set:function(){return Promise.resolve(undefined);},delete:function(){return Promise.resolve(false);},addEventListener:function(){},removeEventListener:function(){}}});}catch(_){ }
  }
  if(typeof window.KeyframeEffect==='undefined'){
    window.KeyframeEffect=function(target,keyframes,options){this.target=target||null;this._keyframes=keyframes||[];this._timing=options||{};};
    window.KeyframeEffect.prototype.getKeyframes=function(){return this._keyframes;};
    window.KeyframeEffect.prototype.getComputedTiming=function(){var t=this._timing||{};return {duration:t.duration||'auto',fill:t.fill||'auto',delay:t.delay||0,endDelay:t.endDelay||0,iterations:t.iterations===undefined?1:t.iterations,direction:t.direction||'normal',easing:t.easing||'linear',progress:null,currentIteration:null};};
    window.KeyframeEffect.prototype.setKeyframes=function(value){this._keyframes=value||[];};
    window.KeyframeEffect.prototype.updateTiming=function(value){for(var key in (value||{}))this._timing[key]=value[key];};
  }
})();
"#;

#[cfg(feature = "servo-runtime")]
const DOM_OBSERVER_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  function identityMatrix(){var m={a:1,b:0,c:0,d:1,e:0,f:0};m.multiply=function(other){return other||identityMatrix();};m.inverse=function(){return identityMatrix();};m.translate=function(x,y){var translated=identityMatrix();translated.e=Number(x)||0;translated.f=Number(y)||0;return translated;};m.scale=function(x,y){var scaled=identityMatrix();scaled.a=Number(x)||1;scaled.d=y===undefined?scaled.a:(Number(y)||1);return scaled;};return m;}
  if(typeof SVGElement!=='undefined'){
    if(!SVGElement.prototype.getElementById)SVGElement.prototype.getElementById=function(id){var nodes=this.querySelectorAll?this.querySelectorAll('[id]'):[];for(var i=0;i<nodes.length;i++)if(nodes[i].getAttribute('id')===id)return nodes[i];return null;};
    if(!SVGElement.prototype.getBBox)SVGElement.prototype.getBBox=function(){var r=this.getBoundingClientRect?this.getBoundingClientRect():null;return {x:r&&r.x||0,y:r&&r.y||0,width:r&&r.width||0,height:r&&r.height||0};};
    if(!SVGElement.prototype.getCTM)SVGElement.prototype.getCTM=function(){return identityMatrix();};
    if(!SVGElement.prototype.getScreenCTM)SVGElement.prototype.getScreenCTM=function(){return this.getCTM?this.getCTM():identityMatrix();};
  }
  if(typeof Element!=='undefined'){
    if(!Element.prototype.querySelectorAllDeep)Element.prototype.querySelectorAllDeep=function(selector){return this.querySelectorAll(selector);};
    if(!Element.prototype.querySelectorDeep)Element.prototype.querySelectorDeep=function(selector){return this.querySelector(selector);};
    if(!Element.prototype.animate)Element.prototype.animate=function(){var animation={onfinish:null,currentTime:null,playState:'idle',play:function(){this.playState='running';},pause:function(){this.playState='paused';},cancel:function(){this.playState='idle';},finish:function(){this.playState='finished';},addEventListener:function(){},removeEventListener:function(){}};animation.finished=Promise.resolve(animation);return animation;};
    if(!Element.prototype.getAnimations)Element.prototype.getAnimations=function(){return [];};
    if(!Element.prototype.checkVisibility)Element.prototype.checkVisibility=function(){return !!(this.offsetWidth||this.offsetHeight);};
  }
  if(typeof HTMLObjectElement!=='undefined'&&HTMLObjectElement.prototype&&!HTMLObjectElement.prototype.getSVGDocument){
    HTMLObjectElement.prototype.getSVGDocument=function(){try{return this.contentDocument||null;}catch(_){return null;}};
  }
  if(typeof Document!=='undefined'){
    if(!Document.prototype.querySelectorAllDeep)Document.prototype.querySelectorAllDeep=function(selector){return this.querySelectorAll(selector);};
    if(!Document.prototype.querySelectorDeep)Document.prototype.querySelectorDeep=function(selector){return this.querySelector(selector);};
    if(!Document.prototype.getAnimations)Document.prototype.getAnimations=function(){return [];};
    if(!Document.prototype.execCommand)Document.prototype.execCommand=function(){return false;};
  }
  function collectionForEach(callback,thisArg){if(typeof callback!=='function')throw new TypeError('callback is not a function');for(var i=0;i<this.length;i++)callback.call(thisArg,this.item(i),i,this);}
  ['NodeList','HTMLCollection','DOMTokenList'].forEach(function(name){try{if(typeof window[name]!=='undefined'&&window[name].prototype&&!window[name].prototype.forEach)window[name].prototype.forEach=collectionForEach;}catch(_){}});
  if(typeof window.MediaQueryList!=='undefined'&&window.MediaQueryList.prototype){if(!window.MediaQueryList.prototype.addEventListener)window.MediaQueryList.prototype.addEventListener=function(type,listener){if(type==='change'&&this.addListener)this.addListener(listener);};if(!window.MediaQueryList.prototype.removeEventListener)window.MediaQueryList.prototype.removeEventListener=function(type,listener){if(type==='change'&&this.removeListener)this.removeListener(listener);};if(!window.MediaQueryList.prototype.dispatchEvent)window.MediaQueryList.prototype.dispatchEvent=function(){return false;};}
  if(typeof window.IntersectionObserver==='undefined'){
    window.IntersectionObserver=function(callback){this._callback=callback;};
    window.IntersectionObserver.prototype.observe=function(target){var self=this;if(typeof self._callback==='function')window.setTimeout(function(){var box={top:0,left:0,right:0,bottom:0,width:0,height:0,x:0,y:0};self._callback([{target:target,isIntersecting:true,intersectionRatio:1,rootBounds:box,intersectionRect:box,boundingClientRect:box}],self);},0);};
    window.IntersectionObserver.prototype.unobserve=function(){};
    window.IntersectionObserver.prototype.disconnect=function(){};
    window.IntersectionObserver.prototype.takeRecords=function(){return [];};
  }
  if(typeof window.ResizeObserver==='undefined'){
    window.ResizeObserver=function(callback){this._callback=callback;};
    window.ResizeObserver.prototype.observe=function(target){var self=this;if(typeof self._callback==='function')window.setTimeout(function(){var box={top:0,left:0,right:0,bottom:0,width:Number(target.offsetWidth)||0,height:Number(target.offsetHeight)||0,x:0,y:0};self._callback([{target:target,contentRect:box}],self);},0);};
    window.ResizeObserver.prototype.unobserve=function(){};
    window.ResizeObserver.prototype.disconnect=function(){};
    window.ResizeObserver.prototype.takeRecords=function(){return [];};
  }
  if(typeof window.IntersectionObserver!=='undefined'&&window.IntersectionObserver.prototype&&!window.IntersectionObserver.prototype.takeRecords)window.IntersectionObserver.prototype.takeRecords=function(){return [];};
  if(typeof window.ResizeObserver!=='undefined'&&window.ResizeObserver.prototype&&!window.ResizeObserver.prototype.takeRecords)window.ResizeObserver.prototype.takeRecords=function(){return [];};
  if(typeof window.PublicKeyCredential==='undefined'){
    window.PublicKeyCredential=function PublicKeyCredential(){};
    window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable=function(){return Promise.resolve(false);};
    window.PublicKeyCredential.isConditionalMediationAvailable=function(){return Promise.resolve(false);};
  }
  if(typeof navigator!=='undefined'&&!navigator.gpu){try{Object.defineProperty(navigator,'gpu',{configurable:true,value:{requestAdapter:function(){return Promise.resolve(null);},getPreferredCanvasFormat:function(){return 'bgra8unorm';},addEventListener:function(){},removeEventListener:function(){}}});}catch(_){}}
  if(typeof window.webkitSpeechRecognition==='undefined'&&typeof window.SpeechRecognition==='undefined'){
    var UnsupportedSpeechRecognition=function(){this.continuous=false;this.interimResults=false;this.lang='';this.onstart=null;this.onend=null;this.onerror=null;this.onresult=null;this._listeners={};};
    UnsupportedSpeechRecognition.prototype.addEventListener=function(type,listener){if(typeof listener==='function')(this._listeners[type]||(this._listeners[type]=[])).push(listener);};
    UnsupportedSpeechRecognition.prototype.removeEventListener=function(type,listener){var list=this._listeners[type]||[];this._listeners[type]=list.filter(function(item){return item!==listener;});};
    UnsupportedSpeechRecognition.prototype.dispatchEvent=function(event){var type=event&&event.type||'';var handler=this['on'+type];if(typeof handler==='function')handler.call(this,event);(this._listeners[type]||[]).slice().forEach(function(listener){listener.call(this,event);},this);return true;};
    UnsupportedSpeechRecognition.prototype.start=function(){var self=this;window.setTimeout(function(){self.dispatchEvent({type:'error',error:'not-allowed',message:'Speech recognition is unavailable',target:self});},0);};
    UnsupportedSpeechRecognition.prototype.stop=function(){this.dispatchEvent({type:'end',target:this});};
    UnsupportedSpeechRecognition.prototype.abort=function(){this.dispatchEvent({type:'end',target:this});};
    window.webkitSpeechRecognition=UnsupportedSpeechRecognition;
    window.SpeechRecognition=UnsupportedSpeechRecognition;
  }
})();
"#;

#[cfg(feature = "servo-runtime")]
const SVG_GEOMETRY_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  function install(target,name,fn){if(!target||typeof target[name]==='function')return;try{Object.defineProperty(target,name,{configurable:true,writable:true,value:fn});}catch(_){try{target[name]=fn;}catch(__){}}}
  function matrix(a,b,c,d,e,f){var m={a:a,b:b,c:c,d:d,e:e,f:f};m.multiply=function(o){return matrix(m.a*o.a+m.c*o.b,m.b*o.a+m.d*o.b,m.a*o.c+m.c*o.d,m.b*o.c+m.d*o.d,m.a*o.e+m.c*o.f+m.e,m.b*o.e+m.d*o.f+m.f);};m.inverse=function(){var det=m.a*m.d-m.b*m.c;if(!det)return matrix(1,0,0,1,0,0);return matrix(m.d/det,-m.b/det,-m.c/det,m.a/det,(m.c*m.f-m.d*m.e)/det,(m.b*m.e-m.a*m.f)/det);};m.translate=function(x,y){return m.multiply(matrix(1,0,0,1,Number(x)||0,Number(y)||0));};m.scale=function(x,y){x=Number(x);if(!isFinite(x))x=1;if(y===undefined)y=x;return m.multiply(matrix(x,0,0,Number(y)||0,0,0));};m.rotate=function(degrees){var r=(Number(degrees)||0)*Math.PI/180,c=Math.cos(r),s=Math.sin(r);return m.multiply(matrix(c,s,-s,c,0,0));};m.flipX=function(){return m.scale(-1,1);};m.flipY=function(){return m.scale(1,-1);};m.skewX=function(degrees){return m.multiply(matrix(1,0,Math.tan((Number(degrees)||0)*Math.PI/180),1,0,0));};m.skewY=function(degrees){return m.multiply(matrix(1,Math.tan((Number(degrees)||0)*Math.PI/180),0,1,0,0));};return m;}
  if(typeof SVGSVGElement!=='undefined'&&SVGSVGElement.prototype)install(SVGSVGElement.prototype,'createSVGMatrix',function(){return matrix(1,0,0,1,0,0);});
  function length(){var box;try{box=this.getBBox?this.getBBox():null;}catch(_){box=null;}var width=box&&Number(box.width)||0,height=box&&Number(box.height)||0;return Math.sqrt(width*width+height*height);}
  function point(distance){var box;try{box=this.getBBox?this.getBBox():null;}catch(_){box=null;}var x=box&&Number(box.x)||0,y=box&&Number(box.y)||0;return {x:x,y:y};}
  ['SVGGeometryElement','SVGPathElement','SVGLineElement','SVGPolylineElement','SVGPolygonElement','SVGRectElement','SVGCircleElement','SVGEllipseElement'].forEach(function(name){try{if(typeof window[name]!=='undefined'&&window[name].prototype){install(window[name].prototype,'getTotalLength',length);install(window[name].prototype,'getPointAtLength',point);}}catch(_){}});
})();
"#;

#[cfg(feature = "servo-runtime")]
const INDEXEDDB_BULK_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  if(typeof IDBIndex==='undefined'||!IDBIndex.prototype)return;
  function install(name,keyOnly){
    try{Object.defineProperty(IDBIndex.prototype,name,{configurable:true,writable:true,value:function(query,count){
      var source=this,request={result:null,error:null,readyState:'pending',source:source,transaction:null,onsuccess:null,onerror:null,_listeners:{}};
      request.addEventListener=function(type,fn){if(typeof fn==='function')(request._listeners[type]||(request._listeners[type]=[])).push(fn);};
      request.removeEventListener=function(type,fn){var list=request._listeners[type]||[];request._listeners[type]=list.filter(function(item){return item!==fn;});};
      function fire(type){var event={type:type,target:request,currentTarget:request};var handler=request['on'+type];if(typeof handler==='function')handler.call(request,event);(request._listeners[type]||[]).slice().forEach(function(fn){fn.call(request,event);});}
      function fail(error){request.error=error;request.readyState='done';fire('error');}
      function finish(values){request.result=values;request.readyState='done';fire('success');}
      var values=[],limit=count===undefined?Infinity:Number(count);if(!isFinite(limit)||limit<0)limit=Infinity;
      if(limit===0){window.setTimeout(function(){finish(values);},0);return request;}
      var cursorRequest;try{cursorRequest=source.openCursor(query);}catch(error){window.setTimeout(function(){fail(error);},0);return request;}
      cursorRequest.onsuccess=function(){var cursor=cursorRequest.result;if(!cursor||values.length>=limit){finish(values);return;}values.push(keyOnly?cursor.primaryKey:cursor.value);try{cursor.continue();}catch(error){fail(error);}};
      cursorRequest.onerror=function(){fail(cursorRequest.error||new DOMException('IndexedDB index cursor failed','UnknownError'));};
      return request;
    }});}catch(_){}}
  install('getAll',false);install('getAllKeys',true);
})();
"#;

#[cfg(feature = "servo-runtime")]
const WEBGL_COMPATIBILITY_SCRIPT: &str = r#"
(function(){
  if(typeof document==='undefined'||typeof HTMLCanvasElement==='undefined')return;
  var nativeGetContext=HTMLCanvasElement.prototype.getContext;
  var probe=document.createElement('canvas');
  var nativeAvailable=window.__browsaiForceFakeWebGL!==true;
  try{
    var nativeContext=nativeGetContext.call(probe,'webgl',{preserveDrawingBuffer:true});
    nativeAvailable=!!nativeContext;
    if(nativeAvailable&&typeof nativeContext.getParameter==='function'){
      nativeAvailable=!!nativeContext.getParameter(nativeContext.VERSION);
    }
    if(nativeAvailable&&typeof nativeContext.getShaderPrecisionFormat==='function'){
      nativeAvailable=!!nativeContext.getShaderPrecisionFormat(nativeContext.VERTEX_SHADER,nativeContext.HIGH_FLOAT);
    }
    if(nativeAvailable&&typeof nativeContext.createShader==='function'){
      var probeShader=nativeContext.createShader(nativeContext.VERTEX_SHADER);
      nativeContext.shaderSource(probeShader,'void main(){gl_Position=vec4(0.0);}');
      nativeContext.compileShader(probeShader);
      nativeAvailable=!!nativeContext.getShaderParameter(probeShader,nativeContext.COMPILE_STATUS);
      if(typeof nativeContext.deleteShader==='function')nativeContext.deleteShader(probeShader);
    }
  }catch(_){nativeAvailable=false;}
  if(window.__browsaiForceFakeWebGL===true)nativeAvailable=false;
  window.__browsaiWebGLMode=nativeAvailable?'native-with-fallback':'fake';
  window.__browsaiWebGLBackend=nativeAvailable?'real':'fake';

  var nextId=1;
  if(typeof window.WebGLRenderingContext==='undefined')window.WebGLRenderingContext=function WebGLRenderingContext(){};
  if(typeof window.WebGL2RenderingContext==='undefined')window.WebGL2RenderingContext=function WebGL2RenderingContext(){};
  try{
    if(typeof Symbol!=='undefined'&&Symbol.toStringTag){
      Object.defineProperty(WebGLRenderingContext.prototype,Symbol.toStringTag,{configurable:true,value:'WebGLRenderingContext'});
      Object.defineProperty(WebGL2RenderingContext.prototype,Symbol.toStringTag,{configurable:true,value:'WebGL2RenderingContext'});
    }
  }catch(_){ }
  ['WebGLRenderingContext','WebGL2RenderingContext'].forEach(function(name){try{var ctor=window[name],proto=ctor&&ctor.prototype;if(!proto||typeof proto.getExtension!=='function'||proto.getExtension.__browsaiDebugFallback)return;var nativeGetExtension=proto.getExtension,nativeSupported=proto.getSupportedExtensions;var getExtension=function(extensionName){var value=null;try{value=nativeGetExtension.call(this,extensionName);}catch(_){ }if(value||extensionName!=='WEBGL_debug_renderer_info')return value;return {UNMASKED_VENDOR_WEBGL:0x9245,UNMASKED_RENDERER_WEBGL:0x9246};};Object.defineProperty(getExtension,'__browsaiDebugFallback',{value:true});Object.defineProperty(proto,'getExtension',{configurable:true,writable:true,value:getExtension});if(typeof nativeSupported==='function'){var getSupportedExtensions=function(){var values=[];try{values=nativeSupported.call(this)||[];}catch(_){ }if(values.indexOf('WEBGL_debug_renderer_info')<0)values=values.concat(['WEBGL_debug_renderer_info']);return values;};Object.defineProperty(proto,'getSupportedExtensions',{configurable:true,writable:true,value:getSupportedExtensions});}}catch(_){}});
  // Some native WebGL implementations expose OES_vertex_array_object but
  // omit one or more methods from the extension object. CanvasKit treats the
  // extension as a complete contract, so bridge missing methods to WebGL2's
  // core VAO API where available and otherwise provide harmless software
  // handles for capability probing.
  ['WebGLRenderingContext','WebGL2RenderingContext'].forEach(function(name){try{var ctor=window[name],proto=ctor&&ctor.prototype;if(!proto||typeof proto.getExtension!=='function'||proto.getExtension.__browsaiVaoFallback)return;var nativeGetExtension=proto.getExtension;var getExtension=function(extensionName){var value=nativeGetExtension.call(this,extensionName);if(extensionName!=='OES_vertex_array_object'||!value)return value;var context=this,wrapped=Object.create(value);['VERTEX_ARRAY_BINDING_OES','createVertexArrayOES','deleteVertexArrayOES','isVertexArrayOES','bindVertexArrayOES'].forEach(function(key){if(key in value)return;if(key==='VERTEX_ARRAY_BINDING_OES')wrapped[key]=0x85B5;else if(key==='createVertexArrayOES')wrapped[key]=function(){return typeof context.createVertexArray==='function'?context.createVertexArray():{};};else if(key==='deleteVertexArrayOES')wrapped[key]=function(arrayObject){if(typeof context.deleteVertexArray==='function')context.deleteVertexArray(arrayObject);};else if(key==='isVertexArrayOES')wrapped[key]=function(arrayObject){return typeof context.isVertexArray==='function'?context.isVertexArray(arrayObject):!!arrayObject;};else wrapped[key]=function(arrayObject){if(typeof context.bindVertexArray==='function')context.bindVertexArray(arrayObject||null);};});return wrapped;};Object.defineProperty(getExtension,'__browsaiVaoFallback',{value:true});Object.defineProperty(proto,'getExtension',{configurable:true,writable:true,value:getExtension});}catch(_){}});
  function object(kind){return {__browsaiWebGLObject:kind,__browsaiId:nextId++};}
  function fakeContext(canvas,version,requestedAttrs){
    window.__browsaiFakeWebGLActive=true;
    var shaders=[],programs=[],buffers=[],textures=[],framebuffers=[],renderbuffers=[],uniforms=[];
  var extensions=['OES_element_index_uint','OES_standard_derivatives','OES_vertex_array_object','ANGLE_instanced_arrays','WEBGL_lose_context','WEBGL_debug_renderer_info','WEBGL_debug_shaders','WEBGL_depth_texture','OES_texture_float','OES_texture_half_float','OES_texture_float_linear','OES_texture_half_float_linear','EXT_texture_filter_anisotropic','EXT_color_buffer_float','EXT_color_buffer_half_float'];
    var constants={
      VERSION:0x1F02,SHADING_LANGUAGE_VERSION:0x8B8C,VENDOR:0x1F00,RENDERER:0x1F01,
      MAX_TEXTURE_SIZE:0x0D33,MAX_CUBE_MAP_TEXTURE_SIZE:0x851C,MAX_RENDERBUFFER_SIZE:0x84E8,
      MAX_VIEWPORT_DIMS:0x0D3A,MAX_VERTEX_ATTRIBS:0x8869,MAX_VERTEX_UNIFORM_VECTORS:0x8DFB,
      MAX_FRAGMENT_UNIFORM_VECTORS:0x8DFD,MAX_VARYING_VECTORS:0x8DFC,MAX_TEXTURE_IMAGE_UNITS:0x8872,
      MAX_VERTEX_TEXTURE_IMAGE_UNITS:0x8B4C,MAX_COMBINED_TEXTURE_IMAGE_UNITS:0x8B4D,
      MAX_VERTEX_UNIFORM_COMPONENTS:0x8B4A,MAX_FRAGMENT_UNIFORM_COMPONENTS:0x8B49,MAX_SAMPLES:0x8D57,
      ALIASED_LINE_WIDTH_RANGE:0x846E,
      ALIASED_POINT_SIZE_RANGE:0x846D,RED_BITS:0x0D52,GREEN_BITS:0x0D53,BLUE_BITS:0x0D54,
      ALPHA_BITS:0x0D55,DEPTH_BITS:0x0D56,STENCIL_BITS:0x0D57,
      ARRAY_BUFFER_BINDING:0x8894,ELEMENT_ARRAY_BUFFER_BINDING:0x8895,
      ACTIVE_TEXTURE:0x84E0,TEXTURE_BINDING_2D:0x8069,TEXTURE_BINDING_CUBE_MAP:0x8514,
      FRAMEBUFFER_BINDING:0x8CA6,RENDERBUFFER_BINDING:0x8CA7,CURRENT_PROGRAM:0x8B8D,
      MAX_DRAW_BUFFERS:0x8824,DRAW_BUFFER0:0x8825,READ_BUFFER:0x0C02,
      COLOR_CLEAR_VALUE:0x0C22,DEPTH_CLEAR_VALUE:0x0B73,STENCIL_CLEAR_VALUE:0x0B91,
      VIEWPORT:0x0BA2,SCISSOR_BOX:0x0C10,COLOR_WRITEMASK:0x0C23,DEPTH_WRITEMASK:0x0B72,
      BLEND_EQUATION_RGB:0x8009,BLEND_EQUATION_ALPHA:0x883D,POLYGON_OFFSET_FACTOR:0x8038,
      POLYGON_OFFSET_UNITS:0x2A00,UNPACK_ALIGNMENT:0x0CF5,PACK_ALIGNMENT:0x0D05,
      BLEND_SRC_RGB:0x80C9,BLEND_DST_RGB:0x80C8,BLEND_SRC_ALPHA:0x80CB,BLEND_DST_ALPHA:0x80CA,
      FUNC_ADD:0x8006,FUNC_SUBTRACT:0x800A,FUNC_REVERSE_SUBTRACT:0x800B,
      DEPTH_FUNC:0x0B74,STENCIL_FUNC:0x0B92,STENCIL_REF:0x0B97,STENCIL_VALUE_MASK:0x0B93,STENCIL_WRITEMASK:0x0B98,
      VERTEX_ATTRIB_ARRAY_ENABLED:0x8622,VERTEX_ATTRIB_ARRAY_SIZE:0x8623,VERTEX_ATTRIB_ARRAY_STRIDE:0x8624,
      VERTEX_ATTRIB_ARRAY_TYPE:0x8625,CURRENT_VERTEX_ATTRIB:0x8626,VERTEX_ATTRIB_ARRAY_NORMALIZED:0x886A,
      VERTEX_ATTRIB_ARRAY_POINTER:0x8645,VERTEX_ATTRIB_ARRAY_BUFFER_BINDING:0x889F,VERTEX_ATTRIB_ARRAY_DIVISOR:0x88FE,INVALID_OPERATION:0x0502,
      ZERO:0,NONE:0,ONE:1,SRC_ALPHA:0x0302,ONE_MINUS_SRC_ALPHA:0x0303,LESS:0x0201,ALWAYS:0x0207,
      NO_ERROR:0,TRIANGLES:4,TRIANGLE_STRIP:5,TRIANGLE_FAN:6,POINTS:0,LINES:1,
      ARRAY_BUFFER:0x8892,ELEMENT_ARRAY_BUFFER:0x8893,STATIC_DRAW:0x88E4,DYNAMIC_DRAW:0x88E8,
      FLOAT:0x1406,FLOAT_VEC2:0x8B50,FLOAT_VEC3:0x8B51,FLOAT_VEC4:0x8B52,INT:0x1404,INT_VEC2:0x8B53,INT_VEC3:0x8B54,INT_VEC4:0x8B55,BOOL:0x8B56,BOOL_VEC2:0x8B57,BOOL_VEC3:0x8B58,BOOL_VEC4:0x8B59,FLOAT_MAT2:0x8B5A,FLOAT_MAT3:0x8B5B,FLOAT_MAT4:0x8B5C,SAMPLER_2D:0x8B5E,SAMPLER_CUBE:0x8B60,UNSIGNED_BYTE:0x1401,UNSIGNED_SHORT:0x1403,UNSIGNED_INT:0x1405,
      VERTEX_SHADER:0x8B31,FRAGMENT_SHADER:0x8B30,COMPILE_STATUS:0x8B81,LINK_STATUS:0x8B82,
      DELETE_STATUS:0x8B80,VALIDATE_STATUS:0x8B83,SHADER_TYPE:0x8B4F,
      LOW_FLOAT:0x8DF0,MEDIUM_FLOAT:0x8DF1,HIGH_FLOAT:0x8DF2,LOW_INT:0x8DF3,MEDIUM_INT:0x8DF4,HIGH_INT:0x8DF5,
      ACTIVE_ATTRIBUTES:0x8B89,ACTIVE_UNIFORMS:0x8B86,CURRENT_PROGRAM:0x8B8D,
      FRAMEBUFFER:0x8D40,RENDERBUFFER:0x8D41,COLOR_ATTACHMENT0:0x8CE0,COLOR:0x1800,RGBA8:0x8058,DEPTH_ATTACHMENT:0x8D00,
      DEPTH_COMPONENT16:0x81A5,DEPTH_COMPONENT24:0x81A6,DEPTH_COMPONENT32F:0x8CAC,
      DEPTH_STENCIL:0x84F9,DEPTH_STENCIL_ATTACHMENT:0x821A,
      FRAMEBUFFER_COMPLETE:0x8CD5,TEXTURE_2D:0x0DE1,TEXTURE_CUBE_MAP:0x8513,
      TEXTURE0:0x84C0,TEXTURE_MIN_FILTER:0x2801,TEXTURE_MAG_FILTER:0x2800,
      TEXTURE_WRAP_S:0x2802,TEXTURE_WRAP_T:0x2803,LINEAR:0x2601,NEAREST:0x2600,
      CLAMP_TO_EDGE:0x812F,REPEAT:0x2901,RGBA:0x1908,RGB:0x1907,DEPTH_COMPONENT:0x1902,
      BLEND:0x0BE2,DEPTH_TEST:0x0B71,CULL_FACE:0x0B44,SCISSOR_TEST:0x0C11,
      UNPACK_FLIP_Y_WEBGL:0x9240,UNPACK_PREMULTIPLY_ALPHA_WEBGL:0x9241
    };
    var ctx={canvas:canvas, drawingBufferWidth:canvas.width||300,drawingBufferHeight:canvas.height||150,
      __browsaiFakeWebGL:true,__browsaiNative:false};
    var activeTexture=constants.TEXTURE0, boundArrayBuffer=null, boundElementArrayBuffer=null;
    var boundTexture2D={}, boundTextureCube={}, boundFramebuffer=null, boundRenderbuffer=null;
    var vertexArrays=[], boundVertexArray=null;
    var enabled={}, drawBufferState=[constants.COLOR_ATTACHMENT0], readBufferState=constants.COLOR_ATTACHMENT0;
    var viewport=[0,0,canvas.width||300,canvas.height||150], scissorBox=[0,0,canvas.width||300,canvas.height||150];
    var clearColor=[0,0,0,0], clearDepth=1, clearStencil=0;
    var blendState={srcRGB:constants.ONE,dstRGB:constants.ZERO,srcAlpha:constants.ONE,dstAlpha:constants.ZERO};
    var depthFunc=constants.LESS, depthWriteMask=true, stencilFunc=constants.ALWAYS, stencilRef=0, stencilValueMask=0xFFFFFFFF, stencilWriteMask=0xFFFFFFFF;
    var colorWriteMask=[true,true,true,true], blendEquationRGB=0x8006, blendEquationAlpha=0x8006, polygonOffsetFactor=0, polygonOffsetUnits=0, unpackAlignment=4, packAlignment=4;
    var vertexAttribs=[];for(var attribIndex=0;attribIndex<16;attribIndex++)vertexAttribs.push({enabled:false,size:4,type:constants.FLOAT,normalized:false,stride:0,offset:0,buffer:null,divisor:0,current:[0,0,0,1]});
    var pendingError=constants.NO_ERROR;
    ctx.__browsaiNonRendering=true;
    ctx.__browsaiPixelOutput='unavailable';
    Object.keys(constants).forEach(function(k){ctx[k]=constants[k];});
    ctx.getContextAttributes=function(){return {alpha:requestedAttrs&&requestedAttrs.alpha!==undefined?!!requestedAttrs.alpha:true,antialias:requestedAttrs&&requestedAttrs.antialias!==undefined?!!requestedAttrs.antialias:false,depth:true,desynchronized:false,preserveDrawingBuffer:!!(requestedAttrs&&requestedAttrs.preserveDrawingBuffer),stencil:requestedAttrs&&requestedAttrs.stencil!==undefined?!!requestedAttrs.stencil:false,failIfMajorPerformanceCaveat:!!(requestedAttrs&&requestedAttrs.failIfMajorPerformanceCaveat),powerPreference:requestedAttrs&&requestedAttrs.powerPreference||'default'};};
    ctx.getSupportedExtensions=function(){return extensions.slice();};
    ctx.getExtension=function(name){
      if(extensions.indexOf(name)<0)return null;
      if(name==='WEBGL_lose_context')return {loseContext:function(){},restoreContext:function(){}};
      if(name==='WEBGL_debug_renderer_info')return {UNMASKED_VENDOR_WEBGL:0x9245,UNMASKED_RENDERER_WEBGL:0x9246};
      if(name==='OES_vertex_array_object')return {
        VERTEX_ARRAY_BINDING_OES:0x85B5,
        createVertexArrayOES:function(){var x=object('vertex-array');vertexArrays.push(x);return x;},
        deleteVertexArrayOES:function(x){if(x){x.deleted=true;if(boundVertexArray===x)boundVertexArray=null;}},
        isVertexArrayOES:function(x){return !!x&&x.__browsaiWebGLObject==='vertex-array'&&!x.deleted;},
        bindVertexArrayOES:function(x){boundVertexArray=x||null;}
      };
      if(name==='ANGLE_instanced_arrays')return {
        VERTEX_ATTRIB_ARRAY_DIVISOR_ANGLE:0x88FE,
        drawArraysInstancedANGLE:function(mode,first,count,primcount){if(typeof ctx.drawArrays==='function')ctx.drawArrays(mode,first,count);},
        drawElementsInstancedANGLE:function(mode,count,type,offset,primcount){if(typeof ctx.drawElements==='function')ctx.drawElements(mode,count,type,offset);},
        vertexAttribDivisorANGLE:function(index,divisor){if(vertexAttribs[index|0])vertexAttribs[index|0].divisor=divisor|0;}
      };
      var extension={};
      if(name==='OES_standard_derivatives')extension.FRAGMENT_SHADER_DERIVATIVE_HINT_OES=0x8B8B;
      if(name==='WEBGL_depth_texture'){extension.UNSIGNED_INT_24_8_WEBGL=0x84FA;extension.UNSIGNED_INT_24_8=0x84FA;extension.DEPTH_COMPONENT32F=0x8CAC;extension.DEPTH32F_STENCIL8=0x8CAD;}
      if(name==='EXT_texture_filter_anisotropic'){extension.TEXTURE_MAX_ANISOTROPY_EXT=0x84FE;extension.MAX_TEXTURE_MAX_ANISOTROPY_EXT=0x84FF;}
      if(name==='EXT_color_buffer_float'){extension.RGBA32F_EXT=0x8814;extension.RGB32F_EXT=0x8815;extension.FRAMEBUFFER_ATTACHMENT_COMPONENT_TYPE_EXT=0x8211;extension.UNSIGNED_NORMALIZED_EXT=0x8C17;}
      if(name==='WEBGL_debug_shaders')extension.getTranslatedShaderSource=function(){return '';};
      return extension;
    };
    ctx.getParameter=function(p){
      if(p===constants.VERSION)return version===2?'WebGL 2.0 BrowsAI Fake':'WebGL 1.0 BrowsAI Fake';
      if(p===constants.SHADING_LANGUAGE_VERSION)return version===2?'WebGL GLSL ES 3.00 BrowsAI Fake':'WebGL GLSL ES 1.00 BrowsAI Fake';
      if(p===constants.VENDOR)return 'BrowsAI';
      if(p===constants.RENDERER)return 'BrowsAI Software WebGL';
      if(p===0x9245)return 'BrowsAI';
      if(p===0x9246)return 'BrowsAI Software WebGL';
      if(p===constants.MAX_TEXTURE_SIZE||p===constants.MAX_CUBE_MAP_TEXTURE_SIZE)return 4096;
      if(p===constants.MAX_DRAW_BUFFERS)return 4;
      if(p===constants.READ_BUFFER)return readBufferState;
      if(p>=constants.DRAW_BUFFER0&&p<constants.DRAW_BUFFER0+4)return drawBufferState[p-constants.DRAW_BUFFER0]||constants.NONE;
      if(p===0x84FF)return 16;
      if(p===constants.MAX_RENDERBUFFER_SIZE)return 4096;
      if(p===constants.MAX_VIEWPORT_DIMS)return new Int32Array([4096,4096]);
      if(p===constants.VIEWPORT)return new Int32Array(viewport);
      if(p===constants.SCISSOR_BOX)return new Int32Array(scissorBox);
      if(p===constants.ARRAY_BUFFER_BINDING)return boundArrayBuffer;
      if(p===constants.ELEMENT_ARRAY_BUFFER_BINDING)return boundElementArrayBuffer;
      if(p===0x85B5)return boundVertexArray;
      if(p===constants.ACTIVE_TEXTURE)return activeTexture;
      if(p===constants.TEXTURE_BINDING_2D)return boundTexture2D[activeTexture]||null;
      if(p===constants.TEXTURE_BINDING_CUBE_MAP)return boundTextureCube[activeTexture]||null;
      if(p===constants.FRAMEBUFFER_BINDING)return boundFramebuffer;
      if(p===constants.RENDERBUFFER_BINDING)return boundRenderbuffer;
      if(p===constants.CURRENT_PROGRAM)return ctx.__program||null;
      if(p===constants.COLOR_CLEAR_VALUE)return new Float32Array(clearColor);
      if(p===constants.DEPTH_CLEAR_VALUE)return clearDepth;
      if(p===constants.STENCIL_CLEAR_VALUE)return clearStencil;
      if(p===constants.BLEND_SRC_RGB)return blendState.srcRGB;
      if(p===constants.BLEND_DST_RGB)return blendState.dstRGB;
      if(p===constants.BLEND_SRC_ALPHA)return blendState.srcAlpha;
      if(p===constants.BLEND_DST_ALPHA)return blendState.dstAlpha;
      if(p===constants.DEPTH_FUNC)return depthFunc;
      if(p===constants.COLOR_WRITEMASK)return new Array(colorWriteMask[0],colorWriteMask[1],colorWriteMask[2],colorWriteMask[3]);
      if(p===constants.DEPTH_WRITEMASK)return depthWriteMask;
      if(p===constants.BLEND_EQUATION_RGB)return blendEquationRGB;
      if(p===constants.BLEND_EQUATION_ALPHA)return blendEquationAlpha;
      if(p===constants.POLYGON_OFFSET_FACTOR)return polygonOffsetFactor;
      if(p===constants.POLYGON_OFFSET_UNITS)return polygonOffsetUnits;
      if(p===constants.UNPACK_ALIGNMENT)return unpackAlignment;
      if(p===constants.PACK_ALIGNMENT)return packAlignment;
      if(p===constants.STENCIL_FUNC)return stencilFunc;
      if(p===constants.STENCIL_REF)return stencilRef;
      if(p===constants.STENCIL_VALUE_MASK)return stencilValueMask;
      if(p===constants.STENCIL_WRITEMASK)return stencilWriteMask;
      if(p===constants.VERTEX_ATTRIB_ARRAY_ENABLED)return null;
      if(p===constants.MAX_VERTEX_ATTRIBS)return 16;
      if(p===constants.MAX_VERTEX_UNIFORM_VECTORS||p===constants.MAX_FRAGMENT_UNIFORM_VECTORS)return 256;
      if(p===constants.MAX_TEXTURE_IMAGE_UNITS||p===constants.MAX_VERTEX_TEXTURE_IMAGE_UNITS)return 16;
      if(p===constants.MAX_COMBINED_TEXTURE_IMAGE_UNITS)return 32;
      if(p===constants.MAX_VERTEX_UNIFORM_COMPONENTS||p===constants.MAX_FRAGMENT_UNIFORM_COMPONENTS)return 1024;
      if(p===constants.MAX_SAMPLES)return 4;
      if(p===constants.MAX_VARYING_VECTORS)return 8;
      if(p===constants.ALIASED_LINE_WIDTH_RANGE)return new Float32Array([1,1]);
      if(p===constants.ALIASED_POINT_SIZE_RANGE)return new Float32Array([1,64]);
      if(p===constants.RED_BITS||p===constants.GREEN_BITS||p===constants.BLUE_BITS||p===constants.ALPHA_BITS)return 8;
      if(p===constants.DEPTH_BITS)return 24;
      if(p===constants.STENCIL_BITS)return 8;
      return null;
    };
    ctx.__browsaiGetParameter=ctx.getParameter;
    ctx.getShaderPrecisionFormat=function(){return {rangeMin:127,rangeMax:127,precision:23};};
    function uniformType(type){return constants[type==='vec2'?'FLOAT_VEC2':type==='vec3'?'FLOAT_VEC3':type==='vec4'?'FLOAT_VEC4':type==='ivec2'?'INT_VEC2':type==='ivec3'?'INT_VEC3':type==='ivec4'?'INT_VEC4':type==='bool'?'BOOL':type==='bvec2'?'BOOL_VEC2':type==='bvec3'?'BOOL_VEC3':type==='bvec4'?'BOOL_VEC4':type==='mat2'?'FLOAT_MAT2':type==='mat3'?'FLOAT_MAT3':type==='mat4'?'FLOAT_MAT4':type==='samplerCube'?'SAMPLER_CUBE':type==='sampler2D'?'SAMPLER_2D':'FLOAT']||constants.FLOAT;}
    function discoverDeclarations(program,keyword,vertexOnly){var found=[],seen={};(program.shaders||[]).forEach(function(shader){if(vertexOnly&&shader.type!==constants.VERTEX_SHADER)return;var source=String(shader&&shader.source||'').replace(/\/\/[^\n]*|\/\*[\s\S]*?\*\//g,'');var match,pattern=new RegExp('\\b'+keyword+'\\s+(float|vec2|vec3|vec4|int|ivec2|ivec3|ivec4|bool|bvec2|bvec3|bvec4|mat2|mat3|mat4)\\s+([A-Za-z_][A-Za-z0-9_]*)(?:\\s*\\[\\s*(\\d+)\\s*\\])?\\s*;','g');while((match=pattern.exec(source))){var name=match[2],size=match[3]?Math.max(1,Number(match[3])):1;if(seen[name])continue;seen[name]=true;found.push({name:name+(size>1?'[0]':''),size:size,type:uniformType(match[1])});}});return found;}
    function discoverUniforms(program){return discoverDeclarations(program,'uniform');}
    ctx.createShader=function(type){var x=object('shader');x.type=type;shaders.push(x);return x;};
    ctx.shaderSource=function(s,source){if(s)s.source=String(source||'');};
    ctx.compileShader=function(s){if(s)s.compiled=true;};
    ctx.getShaderParameter=function(s,p){if(!s)return null;if(p===constants.COMPILE_STATUS)return s.compiled===true;if(p===constants.DELETE_STATUS)return s.deleted===true;if(p===constants.SHADER_TYPE)return s.type;return false;};
    ctx.getShaderSource=function(s){return s&&s.source||null;};
    ctx.getShaderInfoLog=function(){return '';};
    ctx.deleteShader=function(s){if(s)s.deleted=true;};
    ctx.createProgram=function(){var x=object('program');x.shaders=[];x.uniforms=[];x.attributes=[];programs.push(x);return x;};
    ctx.attachShader=function(p,s){if(p&&s)p.shaders.push(s);};
    ctx.detachShader=function(){};ctx.linkProgram=function(p){if(p){p.linked=true;p.uniforms=discoverUniforms(p);p.attributes=discoverDeclarations(p,'attribute',true).concat(discoverDeclarations(p,'in',true));}};ctx.validateProgram=function(p){if(p)p.validated=true;};
    ctx.getProgramParameter=function(p,n){if(!p)return null;if(n===constants.LINK_STATUS)return p.linked===true;if(n===constants.VALIDATE_STATUS)return p.validated===true;if(n===constants.DELETE_STATUS)return p.deleted===true;if(n===constants.ACTIVE_ATTRIBUTES)return (p.attributes||[]).length;if(n===constants.ACTIVE_UNIFORMS)return (p.uniforms||[]).length;return false;};
    ctx.getProgramInfoLog=function(){return '';};ctx.useProgram=function(p){ctx.__program=p||null;};ctx.deleteProgram=function(p){if(p)p.deleted=true;};
    ctx.isShader=function(s){return !!s&&s.__browsaiWebGLObject==='shader'&&!s.deleted;};ctx.isProgram=function(p){return !!p&&p.__browsaiWebGLObject==='program'&&!p.deleted;};
    ctx.getAttribLocation=function(p,name){if(!p||!p.attributes)return -1;for(var i=0;i<p.attributes.length;i++)if(p.attributes[i].name===String(name||''))return i;return -1;};ctx.getUniformLocation=function(p,name){var x=object('uniform');x.program=p||null;x.name=String(name||'');x.value=null;uniforms.push(x);return x;};ctx.getUniform=function(p,location){return location&&location.program===p?location.value:null;};
    ctx.getActiveAttrib=function(p,index){return p&&p.attributes&&p.attributes[index|0]||null;};ctx.getActiveUniform=function(p,index){return p&&p.uniforms&&p.uniforms[index|0]||null;};
    ctx.createBuffer=function(){var x=object('buffer');buffers.push(x);return x;};ctx.bindBuffer=function(target,x){if(target===constants.ARRAY_BUFFER)boundArrayBuffer=x||null;if(target===constants.ELEMENT_ARRAY_BUFFER)boundElementArrayBuffer=x||null;};ctx.bufferData=function(){};ctx.bufferSubData=function(){};ctx.deleteBuffer=function(x){if(x){x.deleted=true;if(boundArrayBuffer===x)boundArrayBuffer=null;if(boundElementArrayBuffer===x)boundElementArrayBuffer=null;}};ctx.isBuffer=function(x){return !!x&&x.__browsaiWebGLObject==='buffer'&&!x.deleted;};
    ctx.createVertexArray=function(){var x=object('vertex-array');vertexArrays.push(x);return x;};ctx.deleteVertexArray=function(x){if(x){x.deleted=true;if(boundVertexArray===x)boundVertexArray=null;}};ctx.isVertexArray=function(x){return !!x&&x.__browsaiWebGLObject==='vertex-array'&&!x.deleted;};ctx.bindVertexArray=function(x){boundVertexArray=x||null;};
    ctx.createTexture=function(){var x=object('texture');textures.push(x);return x;};ctx.bindTexture=function(target,x){if(target===constants.TEXTURE_2D)boundTexture2D[activeTexture]=x||null;if(target===constants.TEXTURE_CUBE_MAP)boundTextureCube[activeTexture]=x||null;};ctx.texImage2D=function(){};ctx.texSubImage2D=function(){};ctx.texParameteri=function(){};ctx.texParameterf=function(){};ctx.generateMipmap=function(){};ctx.deleteTexture=function(x){if(x){x.deleted=true;Object.keys(boundTexture2D).forEach(function(k){if(boundTexture2D[k]===x)boundTexture2D[k]=null;});Object.keys(boundTextureCube).forEach(function(k){if(boundTextureCube[k]===x)boundTextureCube[k]=null;});}};ctx.isTexture=function(x){return !!x&&x.__browsaiWebGLObject==='texture'&&!x.deleted;};
    ctx.createFramebuffer=function(){var x=object('framebuffer');framebuffers.push(x);return x;};ctx.bindFramebuffer=function(target,x){if(target===constants.FRAMEBUFFER)boundFramebuffer=x||null;};ctx.framebufferTexture2D=function(){};ctx.framebufferRenderbuffer=function(){};ctx.checkFramebufferStatus=function(){return constants.FRAMEBUFFER_COMPLETE;};ctx.deleteFramebuffer=function(x){if(x){x.deleted=true;if(boundFramebuffer===x)boundFramebuffer=null;}};ctx.isFramebuffer=function(x){return !!x&&x.__browsaiWebGLObject==='framebuffer'&&!x.deleted;};
    ctx.createRenderbuffer=function(){var x=object('renderbuffer');renderbuffers.push(x);return x;};ctx.bindRenderbuffer=function(target,x){if(target===constants.RENDERBUFFER)boundRenderbuffer=x||null;};ctx.renderbufferStorage=function(){};ctx.framebufferRenderbuffer=function(){};ctx.deleteRenderbuffer=function(x){if(x){x.deleted=true;if(boundRenderbuffer===x)boundRenderbuffer=null;}};ctx.isRenderbuffer=function(x){return !!x&&x.__browsaiWebGLObject==='renderbuffer'&&!x.deleted;};
    ctx.viewport=function(x,y,w,h){viewport=[x,y,w,h];};ctx.scissor=function(x,y,w,h){scissorBox=[x,y,w,h];};ctx.clearColor=function(r,g,b,a){clearColor=[r,g,b,a];};ctx.clearDepth=function(value){clearDepth=value;};ctx.clearStencil=function(value){clearStencil=value;};ctx.clear=function(){};ctx.enable=function(cap){enabled[cap]=true;};ctx.disable=function(cap){enabled[cap]=false;};ctx.isEnabled=function(cap){return enabled[cap]===true;};ctx.blendFunc=function(src,dst){blendState.srcRGB=src;blendState.dstRGB=dst;blendState.srcAlpha=src;blendState.dstAlpha=dst;};ctx.blendFuncSeparate=function(srcRGB,dstRGB,srcAlpha,dstAlpha){blendState.srcRGB=srcRGB;blendState.dstRGB=dstRGB;blendState.srcAlpha=srcAlpha;blendState.dstAlpha=dstAlpha;};ctx.blendEquation=function(value){blendEquationRGB=value;blendEquationAlpha=value;};ctx.blendEquationSeparate=function(rgb,alpha){blendEquationRGB=rgb;blendEquationAlpha=alpha;};ctx.depthFunc=function(value){depthFunc=value;};ctx.depthMask=function(value){depthWriteMask=!!value;};ctx.colorMask=function(r,g,b,a){colorWriteMask=[!!r,!!g,!!b,!!a];};ctx.polygonOffset=function(factor,units){polygonOffsetFactor=factor;polygonOffsetUnits=units;};ctx.pixelStorei=function(pname,param){if(pname===constants.UNPACK_ALIGNMENT)unpackAlignment=param;if(pname===constants.PACK_ALIGNMENT)packAlignment=param;};ctx.stencilFunc=function(func,ref,mask){stencilFunc=func;stencilRef=ref;stencilValueMask=mask;};ctx.stencilMask=function(mask){stencilWriteMask=mask;};ctx.cullFace=function(){};ctx.frontFace=function(){};ctx.lineWidth=function(){};
    ctx.vertexAttribPointer=function(index,size,type,normalized,stride,offset){var a=vertexAttribs[index|0];if(a){a.size=size;a.type=type;a.normalized=!!normalized;a.stride=stride;a.offset=offset;a.buffer=boundArrayBuffer;}};ctx.enableVertexAttribArray=function(index){if(vertexAttribs[index|0])vertexAttribs[index|0].enabled=true;};ctx.disableVertexAttribArray=function(index){if(vertexAttribs[index|0])vertexAttribs[index|0].enabled=false;};ctx.vertexAttribDivisor=function(index,divisor){if(vertexAttribs[index|0])vertexAttribs[index|0].divisor=divisor|0;};ctx.getVertexAttrib=function(index,pname){var a=vertexAttribs[index|0];if(!a)return null;if(pname===constants.VERTEX_ATTRIB_ARRAY_ENABLED)return a.enabled;if(pname===constants.VERTEX_ATTRIB_ARRAY_SIZE)return a.size;if(pname===constants.VERTEX_ATTRIB_ARRAY_TYPE)return a.type;if(pname===constants.VERTEX_ATTRIB_ARRAY_NORMALIZED)return a.normalized;if(pname===constants.VERTEX_ATTRIB_ARRAY_STRIDE)return a.stride;if(pname===constants.VERTEX_ATTRIB_ARRAY_BUFFER_BINDING)return a.buffer;if(pname===constants.VERTEX_ATTRIB_ARRAY_DIVISOR)return a.divisor;if(pname===constants.CURRENT_VERTEX_ATTRIB)return new Float32Array(a.current);return null;};ctx.getVertexAttribOffset=function(index,pname){var a=vertexAttribs[index|0];return a&&pname===constants.VERTEX_ATTRIB_ARRAY_POINTER?a.offset:0;};ctx.vertexAttrib1f=function(i,a){if(vertexAttribs[i|0])vertexAttribs[i|0].current=[a,0,0,1];};ctx.vertexAttrib2f=function(i,a,b){if(vertexAttribs[i|0])vertexAttribs[i|0].current=[a,b,0,1];};ctx.vertexAttrib3f=function(i,a,b,c){if(vertexAttribs[i|0])vertexAttribs[i|0].current=[a,b,c,1];};ctx.vertexAttrib4f=function(i,a,b,c,d){if(vertexAttribs[i|0])vertexAttribs[i|0].current=[a,b,c,d];};
    ctx.uniform1f=function(l,a){if(l)l.value=a;};ctx.uniform2f=function(l,a,b){if(l)l.value=[a,b];};ctx.uniform3f=function(l,a,b,c){if(l)l.value=[a,b,c];};ctx.uniform4f=function(l,a,b,c,d){if(l)l.value=[a,b,c,d];};ctx.uniform1i=ctx.uniform1f;ctx.uniform2i=ctx.uniform2f;ctx.uniform3i=ctx.uniform3f;ctx.uniform4i=ctx.uniform4f;ctx.uniform1fv=function(l,v){if(l)l.value=Array.from(v);};ctx.uniform2fv=ctx.uniform1fv;ctx.uniform3fv=ctx.uniform1fv;ctx.uniform4fv=ctx.uniform1fv;ctx.uniformMatrix2fv=function(l,t,v){if(l)l.value=Array.from(v);};ctx.uniformMatrix3fv=ctx.uniformMatrix2fv;ctx.uniformMatrix4fv=ctx.uniformMatrix2fv;
    ctx.activeTexture=function(texture){if(texture>=constants.TEXTURE0)activeTexture=texture;};ctx.drawArrays=function(){};ctx.drawElements=function(){};ctx.drawArraysInstanced=function(mode,first,count,primcount){ctx.drawArrays(mode,first,count);};ctx.drawElementsInstanced=function(mode,count,type,offset,primcount){ctx.drawElements(mode,count,type,offset);};ctx.drawBuffers=function(values){drawBufferState=Array.from(values||[]);};ctx.readBuffer=function(value){readBufferState=value;};ctx.blitFramebuffer=function(){};ctx.clearBufferfv=function(){};ctx.clearBufferiv=function(){};ctx.clearBufferuiv=function(){};ctx.clearBufferfi=function(){};ctx.texStorage2D=function(){};ctx.texStorage3D=function(){};ctx.vertexAttribIPointer=ctx.vertexAttribPointer;ctx.uniform1ui=ctx.uniform1i;ctx.uniform2ui=ctx.uniform2i;ctx.uniform3ui=ctx.uniform3i;ctx.uniform4ui=ctx.uniform4i;ctx.flush=function(){};ctx.finish=function(){};ctx.getError=function(){var error=pendingError;pendingError=constants.NO_ERROR;return error;};ctx.isContextLost=function(){return false;};ctx.readPixels=function(){pendingError=constants.INVALID_OPERATION;};
    var proto=version===2&&typeof WebGL2RenderingContext!=='undefined'?WebGL2RenderingContext.prototype:(typeof WebGLRenderingContext!=='undefined'?WebGLRenderingContext.prototype:null);
    if(proto)try{
      if(typeof proto.getParameter!=='function')Object.defineProperty(proto,'getParameter',{configurable:true,enumerable:false,writable:true,value:function(p){return this.__browsaiGetParameter(p);}});
      Object.setPrototypeOf(ctx,proto);
    }catch(_){ }
    return new Proxy(ctx,{get:function(target,key){if(key in target)return target[key];if(typeof key==='string')return function(){return undefined;};return undefined;}});
  }
  function usable(context,version){
    try{
      if(!context||typeof context.getParameter!=='function')return false;
      var reported=String(context.getParameter(context.VERSION)||'');
      return !!reported&&(version!==2||/WebGL\\s*2/i.test(reported));
    }catch(_){return false;}
  }
  function wrappedGetContext(type,attrs){
    if(type==='webgl'||type==='experimental-webgl'||type==='webgl2'){
      if(window.__browsaiForceFakeWebGL!==true)try{var nativeContext=nativeGetContext.call(this,type,attrs);if(usable(nativeContext,type==='webgl2'?2:1))return nativeContext;}catch(_){ }
      return fakeContext(this,type==='webgl2'?2:1,attrs);
    }
    return nativeGetContext.call(this,type,attrs);
  }
  HTMLCanvasElement.prototype.getContext=wrappedGetContext;
  if(typeof OffscreenCanvas==='undefined'){
    window.OffscreenCanvas=function(width,height){this.width=width||300;this.height=height||150;};
    window.OffscreenCanvas.prototype.getContext=function(type){
      if(type==='webgl'||type==='experimental-webgl')return fakeContext(this,1);
      if(type==='webgl2')return fakeContext(this,2);
      return null;
    };
  }
  if(typeof OffscreenCanvas!=='undefined'&&OffscreenCanvas.prototype.getContext){
    var nativeOffscreen=OffscreenCanvas.prototype.getContext;
    OffscreenCanvas.prototype.getContext=function(type,attrs){
      if(type==='webgl'||type==='experimental-webgl'||type==='webgl2'){
        if(window.__browsaiForceFakeWebGL!==true)try{var nativeContext=nativeOffscreen.call(this,type,attrs);if(usable(nativeContext,type==='webgl2'?2:1))return nativeContext;}catch(_){ }
        return fakeContext(this,type==='webgl2'?2:1,attrs);
      }
      return nativeOffscreen.call(this,type,attrs);
    };
  }
})();
"#;

#[cfg(feature = "servo-runtime")]
const RUNTIME_DIAGNOSTICS_SCRIPT: &str = r#"
(function(){
  if(window.__browsaiRuntimeDiagnosticsInstalled)return;
  window.__browsaiRuntimeDiagnosticsInstalled=true;
  window.__browsaiInitializationStage='document-start';
  function emit(kind,error,source,line,column){
    var value=error;
    var message='';
    var stack=null;
    var errorName=null;
    var errorConstructor=null;
    if(value&&typeof value==='object'){
      message=String(value.message||value.reason||value);
      stack=value.stack||null;
      errorName=value.name?String(value.name):null;
      try{errorConstructor=value.constructor&&value.constructor.name?String(value.constructor.name):null;}catch(_){ }
    }else message=String(value||'');
    var record={kind:kind,message:message,stack:stack,script_url:source||null,
      line:typeof line==='number'&&line>0?line:null,
      column:typeof column==='number'&&column>0?column:null,
      realm:'window-or-document',api:null,error_name:errorName,
      error_constructor:errorConstructor,
      initialization_stage:window.__browsaiInitializationStage||null};
    try{console.error('BROWSAI_RUNTIME_EVENT '+JSON.stringify(record));}catch(_){ }
  }
  window.addEventListener('error',function(event){
    emit('error',event.error||event.message,event.filename,event.lineno,event.colno);
  });
  window.addEventListener('unhandledrejection',function(event){
    emit('unhandledrejection',event.reason,null,null,null);
  });
  window.addEventListener('load',function(event){
    var target=event.target;
    if(!target||!target.tagName)return;
    var tag=String(target.tagName).toLowerCase();
    if(tag!=='script'&&tag!=='link')return;
    var url=target.src||target.href||null;
    try{console.info('BROWSAI_RESOURCE_EVENT '+JSON.stringify({kind:'load',tag:tag,url:url}));}catch(_){ }
  },true);
  window.addEventListener('error',function(event){
    var target=event.target;
    if(!target||!target.tagName)return;
    var tag=String(target.tagName).toLowerCase();
    if(tag!=='script'&&tag!=='link')return;
    var url=target.src||target.href||null;
    try{console.error('BROWSAI_RESOURCE_EVENT '+JSON.stringify({kind:'error',tag:tag,url:url}));}catch(_){ }
  },true);
})();
"#;

#[cfg(feature = "servo-runtime")]
impl servo::WebViewDelegate for RuntimeWebViewDelegate {
    fn show_console_message(
        &self,
        _webview: servo::WebView,
        level: servo::ConsoleLogLevel,
        message: String,
    ) {
        let formatted = format!("{level:?}: {message}");
        let mut console = self.console.borrow_mut();
        if console.len() >= RUNTIME_CONSOLE_LIMIT
            || console.iter().map(String::len).sum::<usize>() + formatted.len()
                > RUNTIME_CONSOLE_BYTES_LIMIT
        {
            if console
                .last()
                .map_or(true, |entry| entry != "[console output truncated]")
            {
                console.push("[console output truncated]".into());
            }
            return;
        }
        console.push(formatted);
    }

    fn notify_crashed(&self, _webview: servo::WebView, reason: String, _backtrace: Option<String>) {
        self.crashes.borrow_mut().push(reason);
    }

    fn notify_history_changed(&self, _webview: servo::WebView, entries: Vec<Url>, _current: usize) {
        *self.history.borrow_mut() = entries;
    }

    fn load_web_resource(&self, _webview: servo::WebView, load: servo::WebResourceLoad) {
        {
            let mut requests = self.resource_requests.borrow_mut();
            if requests.len() < RESOURCE_REQUEST_LIMIT {
                requests.push(load.request.url.clone());
            } else {
                self.resource_requests_truncated.set(true);
            }
        }
        if load.request.is_for_main_frame {
            let mut requests = self.main_frame_requests.borrow_mut();
            if requests.len() < MAIN_FRAME_REQUEST_LIMIT {
                requests.push(load.request.url.clone());
            } else {
                self.main_frame_requests_truncated.set(true);
            }
        }
        // Dropping without interception deliberately lets Servo perform the
        // normal network load. This callback is observation-only.
    }
}

#[cfg(feature = "servo-runtime")]
impl ServoRuntime {
    pub fn new(profile_identity: browsai_engine_api::ProfileIdentity) -> Self {
        let opts = servo::Opts {
            background_hang_monitor: true,
            ignore_certificate_errors: std::env::var("BROWSAI_IGNORE_CERTIFICATE_ERRORS")
                .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes")),
            ..Default::default()
        };
        // Servo already provides standards-backed OffscreenCanvas and Font
        // Loading implementations, including their worker exposure, but
        // both features default to disabled. Enable them at the engine
        // boundary so blob/module workers see the same APIs as Window rather
        // than falling back to page-local shims.
        let preferences = servo::Preferences {
            dom_offscreen_canvas_enabled: true,
            dom_fontface_enabled: true,
            // Servo has a native IntersectionObserver implementation, but the
            // feature is disabled by default. Enable it for all browsing contexts.
            dom_intersection_observer_enabled: true,
            // ServiceWorker registration obtains its storage key through the
            // native storage/IndexedDB preference. Keep the real storage path
            // enabled so registration does not fail before reaching the worker
            // manager.
            dom_indexeddb_enabled: true,
            // Servo includes a native ServiceWorker manager and realm. Keep it
            // enabled so pages observe the real container/registration contract;
            // the compatibility shim only annotates genuine absence or failure.
            dom_serviceworker_enabled: true,
            // Expose Servo's native Permissions surface so feature-detection
            // code does not mistake an agent runtime for a browser without it.
            dom_permissions_enabled: true,
            // Do not inherit a bare POSIX locale such as `C`/`c` into browser
            // language surfaces. Servo uses this preference for navigator.language
            // and the default Accept-Language header, so one override keeps the
            // HTTP, Navigator, and Intl environments coherent. The locale is
            // resolved from the supplied `ProfileIdentity`; legacy env-var
            // overrides remain as a fallback that emits a deprecation warning.
            intl_locale_override: {
                let legacy = std::env::var("BROWSAI_LOCALE").ok();
                if legacy.is_some() {
                    eprintln!(
                        "warning: BROWSAI_LOCALE is deprecated; pass a ProfileIdentity via ContextOptions instead"
                    );
                }
                let from_profile = profile_identity.locale.clone();
                if !from_profile.trim().is_empty() {
                    from_profile
                } else if let Some(value) = legacy {
                    if !value.trim().is_empty() {
                        value
                    } else {
                        "en-US".into()
                    }
                } else {
                    "en-US".into()
                }
            },
            user_agent: {
                let legacy = std::env::var("BROWSAI_USER_AGENT").ok();
                if legacy.is_some() {
                    eprintln!(
                        "warning: BROWSAI_USER_AGENT is deprecated; pass a ProfileIdentity via ContextOptions instead"
                    );
                }
                let from_profile = profile_identity.user_agent.clone();
                if !from_profile.trim().is_empty() {
                    from_profile
                } else if let Some(value) = legacy {
                    if !value.trim().is_empty() {
                        value
                    } else {
                        // Present the runtime as a current Chromium desktop browser to
                        // sites that gate their application shell on the user-agent. Keep
                        // this configurable so a caller can select a matching Chrome
                        // release without rebuilding the engine.
                        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".into()
                    }
                } else {
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".into()
                }
            },
            ..Default::default()
        };
        // Servo's embedder API does not currently expose a timezone
        // preference; SpiderMonkey reads the process timezone at startup.
        // Pin it before building Servo so Intl and Date behavior do not vary
        // with the host machine. The timezone comes from the resolved
        // ProfileIdentity; legacy env-var overrides remain as a fallback.
        let timezone = {
            let legacy = std::env::var("BROWSAI_TIMEZONE").ok();
            if legacy.is_some() {
                eprintln!(
                    "warning: BROWSAI_TIMEZONE is deprecated; pass a ProfileIdentity via ContextOptions instead"
                );
            }
            let from_profile = profile_identity.timezone.clone();
            if !from_profile.trim().is_empty() {
                from_profile
            } else if let Some(value) = legacy {
                if !value.trim().is_empty() {
                    value
                } else {
                    "UTC".into()
                }
            } else {
                "UTC".into()
            }
        };
        std::env::set_var("TZ", timezone);
        Self {
            inner: servo::ServoBuilder::default()
                .opts(opts)
                .preferences(preferences)
                .build(),
            identity: profile_identity,
        }
    }

    pub fn spin_event_loop(&self) {
        self.inner.spin_event_loop();
    }

    pub fn create_page(
        &self,
        viewport: browsai_engine_api::VirtualViewport,
        url: Url,
    ) -> Result<ServoRuntimePage, EngineError> {
        self.create_page_with_identity(viewport, url, None)
    }

    pub fn create_page_with_identity(
        &self,
        viewport: browsai_engine_api::VirtualViewport,
        url: Url,
        identity_override: Option<browsai_engine_api::ProfileIdentity>,
    ) -> Result<ServoRuntimePage, EngineError> {
        let identity = identity_override.unwrap_or_else(|| self.identity.clone());
        let rendering_context = std::rc::Rc::new(
            servo::SoftwareRenderingContext::new(dpi::PhysicalSize::new(
                viewport.width.max(1),
                viewport.height.max(1),
            ))
            .map_err(|error| EngineError::Other(format!("Servo rendering context: {error:?}")))?,
        );
        servo::RenderingContext::make_current(rendering_context.as_ref()).map_err(|error| {
            EngineError::Other(format!("Servo rendering context activation: {error:?}"))
        })?;
        let delegate = std::rc::Rc::new(RuntimeWebViewDelegate::default());
        let user_content_manager = servo::UserContentManager::new(&self.inner);
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            RUNTIME_DIAGNOSTICS_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            CACHE_STORAGE_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            API_SURFACE_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            build_navigator_identity_script(&identity).into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            build_screen_identity_script(viewport).into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            INDEXEDDB_BULK_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            WEBGL_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            DOM_OBSERVER_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            SVG_GEOMETRY_COMPATIBILITY_SCRIPT.into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            "if(typeof SVGAnimatedString==='undefined'){window.SVGAnimatedString=function(value){this.baseVal=value||'';this.animVal=this.baseVal;};}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // SVG URI-reference elements expose href as an SVGAnimatedString.
            // Some telemetry bundles only read href.baseVal, so provide the
            // attribute-backed shape when Servo does not expose the IDL member.
            "(function(){try{var names=['SVGAElement','SVGUseElement','SVGImageElement','SVGTextPathElement','SVGPatternElement','SVGScriptElement','SVGFEImageElement'];names.forEach(function(name){var ctor=window[name];if(!ctor&&name==='SVGAElement'&&typeof window.SVGElement==='function')ctor=window.SVGElement;if(!ctor||!ctor.prototype||Object.getOwnPropertyDescriptor(ctor.prototype,'href'))return;Object.defineProperty(ctor.prototype,'href',{configurable:true,get:function(){var value=this.getAttribute('href')||this.getAttribute('xlink:href')||'';return {baseVal:value,animVal:value};}});});}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Servo's SVG element tree omits a few common constructor aliases
            // used by framework client routers for instanceof checks.
            "(function(){try{if(typeof window.SVGAElement==='undefined'&&typeof window.SVGElement==='function')window.SVGAElement=window.SVGElement;if(typeof window.SVGGraphicsElement==='undefined'&&typeof window.SVGElement==='function')window.SVGGraphicsElement=window.SVGElement;}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Servo exposes SVG elements but not the animated length IDL
            // members used by common SVG/Lottie players. Preserve the
            // attribute-derived value without pretending to animate it.
            "if(typeof SVGSVGElement!=='undefined'){var browsaiSvgLength=function(element,attribute){var cached='__browsaiSvg'+attribute+'Length';if(!element[cached]){var raw=element.getAttribute(attribute);var value=parseFloat(raw);if(!isFinite(value))value=0;var base={value:value,valueInSpecifiedUnits:value,unitType:1};Object.defineProperty(element,cached,{configurable:true,value:{baseVal:base,animVal:{value:value,valueInSpecifiedUnits:value,unitType:1}}});}return element[cached];};['width','height'].forEach(function(attribute){if(!Object.getOwnPropertyDescriptor(SVGSVGElement.prototype,attribute))Object.defineProperty(SVGSVGElement.prototype,attribute,{configurable:true,get:function(){return browsaiSvgLength(this,attribute);}});});}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // SVG animation controls are commonly used by page-level pause
            // overlays even when no SVG animation timeline is available.
            "(function(){try{if(typeof SVGSVGElement!=='undefined'&&SVGSVGElement.prototype){if(!SVGSVGElement.prototype.pauseAnimations)SVGSVGElement.prototype.pauseAnimations=function(){};if(!SVGSVGElement.prototype.unpauseAnimations)SVGSVGElement.prototype.unpauseAnimations=function(){};if(!SVGSVGElement.prototype.animationsPaused)SVGSVGElement.prototype.animationsPaused=function(){return false;};if(!SVGSVGElement.prototype.setCurrentTime)SVGSVGElement.prototype.setCurrentTime=function(){};}}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Some SVG/Lottie players construct Animation directly even when
            // they only need a controllable placeholder for feature detection.
            "if(typeof window.Animation==='undefined'){window.Animation=function(effect,timeline){this.effect=effect||null;this.timeline=timeline||null;this.currentTime=null;this.startTime=null;this.playbackRate=1;this.playState='idle';this.finished=Promise.resolve(this);this.ready=Promise.resolve(this);this.play=function(){this.playState='running';return this;};this.pause=function(){this.playState='paused';};this.cancel=function(){this.playState='idle';};this.finish=function(){this.playState='finished';};this.reverse=function(){this.playbackRate=-this.playbackRate;};this.updatePlaybackRate=function(rate){this.playbackRate=Number(rate)||0;};this.addEventListener=function(){};this.removeEventListener=function(){};};}".into(),
            None,
        )));
        // The first-generation inline IndexedDB shims used deeply nested
        // minified closures that older Servo parser builds reject. Keep them
        // available only for explicit compatibility debugging; the compact
        // cursor-backed shim below is the default.
        if std::env::var_os("BROWS_AI_ENABLE_LEGACY_IDB_SHIMS").is_some() {
            user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Servo exposes native IDBIndex cursors but some vendor storage
            // wrappers feature-detect getAll/getAllKeys.  Build the missing
            // request surface on top of that native cursor rather than
            // returning fabricated storage data.
            "(function(){try{function setup(){if(typeof IDBIndex==='undefined'||!IDBIndex.prototype||typeof IDBIndex.prototype.openCursor!=='function')return false;function install(name,keyOnly){if(typeof IDBIndex.prototype[name]==='function')return;Object.defineProperty(IDBIndex.prototype,name,{configurable:true,writable:true,value:function(query,count){var source=this,request={result:null,error:null,readyState:'pending',source:source,transaction:null,onsuccess:null,onerror:null,_listeners:{}};request.addEventListener=function(type,fn){if(typeof fn==='function')(request._listeners[type]||(request._listeners[type]=[])).push(fn);};request.removeEventListener=function(type,fn){var list=request._listeners[type]||[];request._listeners[type]=list.filter(function(item){return item!==fn;});};function fire(type){var event={type:type,target:request,currentTarget:request};var handler=request['on'+type];if(typeof handler==='function')handler.call(request,event);(request._listeners[type]||[]).slice().forEach(function(fn){fn.call(request,event);});}function fail(error){request.error=error;request.readyState='done';fire('error');}function finish(values){request.result=values;request.readyState='done';fire('success');}var values=[],limit=count===undefined?Infinity:Number(count);if(!isFinite(limit)||limit<0)limit=Infinity;if(limit===0){window.setTimeout(function(){finish(values);},0);return request;}var cursorRequest;try{cursorRequest=source.openCursor(query);}catch(error){window.setTimeout(function(){fail(error);},0);return request;}cursorRequest.onsuccess=function(){var cursor=cursorRequest.result;if(!cursor||values.length>=limit){finish(values);return;}values.push(keyOnly?cursor.primaryKey:cursor.value);try{cursor.continue();}catch(error){fail(error);}};cursorRequest.onerror=function(){fail(cursorRequest.error||new DOMException('IndexedDB cursor failed','UnknownError'));};return request;};});install('getAll',false);install('getAllKeys',true);return true;}function retry(n){if(!setup()&&n<200)window.setTimeout(function(){retry(n+1);},25);}retry(0);}catch(_){}})();".into(),
            None,
        )));
            user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Some Servo builds expose the object-store cursor but omit the
            // generated getAll methods.  Cover that equivalent path with the
            // same asynchronous cursor-backed request contract.
            "(function(){try{function setup(){var proto=typeof IDBObjectStore!=='undefined'&&IDBObjectStore.prototype;if(!proto||typeof proto.openCursor!=='function')return false;['getAll','getAllKeys'].forEach(function(name){if(typeof proto[name]==='function')return;var keyOnly=name==='getAllKeys';Object.defineProperty(proto,name,{configurable:true,writable:true,value:function(query,count){var source=this,request={result:null,error:null,readyState:'pending',source:source,transaction:null,onsuccess:null,onerror:null,_listeners:{}};request.addEventListener=function(type,fn){if(typeof fn==='function')(request._listeners[type]||(request._listeners[type]=[])).push(fn);};request.removeEventListener=function(type,fn){var list=request._listeners[type]||[];request._listeners[type]=list.filter(function(item){return item!==fn;});};function fire(type){var event={type:type,target:request,currentTarget:request};var handler=request['on'+type];if(typeof handler==='function')handler.call(request,event);(request._listeners[type]||[]).slice().forEach(function(fn){fn.call(request,event);});}function finish(values){request.result=values;request.readyState='done';fire('success');}function fail(error){request.error=error;request.readyState='done';fire('error');}var values=[],limit=count===undefined?Infinity:Number(count);if(!isFinite(limit)||limit<0)limit=Infinity;if(limit===0){window.setTimeout(function(){finish(values);},0);return request;}var cursorRequest;try{cursorRequest=source.openCursor(query);}catch(error){window.setTimeout(function(){fail(error);},0);return request;}cursorRequest.onsuccess=function(){var cursor=cursorRequest.result;if(!cursor||values.length>=limit){finish(values);return;}values.push(keyOnly?cursor.key:cursor.value);try{cursor.continue();}catch(error){fail(error);}};cursorRequest.onerror=function(){fail(cursorRequest.error||new DOMException('IndexedDB cursor failed','UnknownError'));};return request;}});return true;}function retry(n){if(!setup()&&n<200)window.setTimeout(function(){retry(n+1);},25);}retry(0);}catch(_){}})();".into(),
            None,
        )));
        }
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Headless Servo has no host display to answer ScreenMetrics, which
            // otherwise leaves screen dimensions at zero. Keep the exposed
            // desktop viewport identity coherent without changing real metrics.
            "(function(){try{if(typeof screen!=='undefined'){if(!screen.width||!screen.height){var values={width:1920,height:1080,availWidth:1920,availHeight:1040,colorDepth:24,pixelDepth:24};Object.keys(values).forEach(function(key){try{Object.defineProperty(screen,key,{configurable:true,value:values[key]});}catch(_){}});}if(typeof screen.orientation==='undefined'){var orientation={type:'landscape-primary',angle:0,onchange:null,lock:function(){return Promise.resolve();},unlock:function(){},addEventListener:function(){},removeEventListener:function(){},dispatchEvent:function(){return false;}};try{Object.defineProperty(screen,'orientation',{configurable:true,value:orientation});}catch(_){}}else{try{if(!screen.orientation.addEventListener)screen.orientation.addEventListener=function(){};if(!screen.orientation.removeEventListener)screen.orientation.removeEventListener=function(){};}catch(_){}}}}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Keep the common Chromium identity surfaces consistent with the
            // Chrome-compatible user-agent sent on the network. These are
            // deliberately small compatibility shims, not a claim that every
            // Chromium-only API is implemented by Servo.
            "(function(){try{if(typeof navigator!=='undefined'){try{Object.defineProperty(navigator,'vendor',{configurable:true,value:'Google Inc.'});}catch(_){}try{Object.defineProperty(navigator,'platform',{configurable:true,value:'Linux x86_64'});}catch(_){}try{var ua=String(navigator.userAgent||'');if(!navigator.appVersion||navigator.appVersion.indexOf('Chrome/')<0)Object.defineProperty(navigator,'appVersion',{configurable:true,value:ua.replace(/^Mozilla\\/5\\.0\\s*/, '')});}catch(_){}if(!navigator.userAgentData){var match=/Chrome\\/(\\d+)/.exec(navigator.userAgent||'');var version=match?match[1]:'140';var brands=[{brand:'Not A(Brand',version:'99'},{brand:'Chromium',version:version},{brand:'Google Chrome',version:version}];var data={brands:brands,mobile:false,platform:'Linux',getHighEntropyValues:function(hints){var result={brands:brands,mobile:false,platform:'Linux'};if(Array.isArray(hints)){if(hints.indexOf('architecture')>=0)result.architecture='x86';if(hints.indexOf('bitness')>=0)result.bitness='64';if(hints.indexOf('model')>=0)result.model='';if(hints.indexOf('platformVersion')>=0)result.platformVersion='0.0.0';if(hints.indexOf('uaFullVersion')>=0)result.uaFullVersion=version+'.0.0.0';}return Promise.resolve(result);},toJSON:function(){return {brands:brands,mobile:false,platform:'Linux'};}};Object.defineProperty(navigator,'userAgentData',{configurable:true,value:data});}if(!navigator.mediaDevices){var mediaDevices={enumerateDevices:function(){return Promise.resolve([]);},getUserMedia:function(){return Promise.reject(new DOMException('Media capture is unavailable','NotAllowedError'));},addEventListener:function(){},removeEventListener:function(){}};try{Object.defineProperty(navigator,'mediaDevices',{configurable:true,value:mediaDevices});}catch(_){}}if(typeof navigator.share!=='function'){try{Object.defineProperty(navigator,'share',{configurable:true,value:function(){return Promise.reject(new DOMException('Web Share is unavailable','NotAllowedError'));}});Object.defineProperty(navigator,'canShare',{configurable:true,value:function(){return false;}});}catch(_){}}}if(typeof window.chrome==='undefined')window.chrome={runtime:{}};if(!window.chrome.runtime)window.chrome.runtime={};if(typeof window.chrome.runtime.sendMessage!=='function')window.chrome.runtime.sendMessage=function(message,options,callback){if(typeof options==='function'){callback=options;}if(typeof callback==='function')window.setTimeout(function(){callback(undefined);},0);return Promise.resolve(undefined);};}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            if std::env::var("BROWSAI_FORCE_FAKE_WEBGL")
                .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
            {
                "window.__browsaiForceFakeWebGL=true;".into()
            } else {
                "".into()
            },
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            "if(typeof Element!=='undefined'&&!Object.getOwnPropertyDescriptor(Element.prototype,'dataset')){Object.defineProperty(Element.prototype,'dataset',{get:function(){var data={};var attrs=this.attributes||[];for(var i=0;i<attrs.length;i++){var name=attrs[i].name;if(name.indexOf('data-')===0){var key=name.slice(5).replace(/-([a-z])/g,function(_,c){return c.toUpperCase();});data[key]=attrs[i].value;}}return data;}});}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Bing's bootstrap refers to `_w` before its own later declaration.
            // Supply the harmless portions needed by that bootstrap so the
            // search form and navigation controls can initialize.
            "if(typeof window._w==='undefined'){window._w={};}if(typeof window._w.directLog!=='function'){window._w.directLog=function(){};}if(!window._w.scheduler){window._w.scheduler={};}if(!window._w.rms){window._w.rms={js:function(){}};}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            "window.__browsaiServiceWorkerWasMissing=typeof navigator==='undefined'||!navigator.serviceWorker;".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Some high-traffic pages only feature-detect Service Workers but
            // assume the namespace exists when reporting optional telemetry.
            "if(typeof navigator!=='undefined'&&!navigator.serviceWorker){try{Object.defineProperty(navigator,'serviceWorker',{configurable:true,value:{ready:Promise.resolve({unregister:function(){return Promise.resolve(true);}}),register:function(){return Promise.resolve({unregister:function(){return Promise.resolve(true);}});},getRegistration:function(){return Promise.resolve(undefined);},getRegistrations:function(){return Promise.resolve([]);},addEventListener:function(){},removeEventListener:function(){}}});}catch(_){}}else if(navigator.serviceWorker){try{if(!navigator.serviceWorker.getRegistration)navigator.serviceWorker.getRegistration=function(){return Promise.resolve(undefined);};if(!navigator.serviceWorker.getRegistrations)navigator.serviceWorker.getRegistrations=function(){return Promise.resolve([]);};if(!navigator.serviceWorker.ready)Object.defineProperty(navigator.serviceWorker,'ready',{configurable:true,value:Promise.resolve({unregister:function(){return Promise.resolve(true);}})});}catch(_){}}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Keep the ServiceWorker assessment explicit: the compatibility
            // namespace shim must not be mistaken for a running SW realm.
            "if(typeof window.__browsaiServiceWorkerMode==='undefined'){window.__browsaiServiceWorkerMode=window.__browsaiServiceWorkerWasMissing?'shim-compatible':((typeof navigator!=='undefined'&&navigator.serviceWorker)?'native':'unavailable');}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Install observer constructors in their own guarded user script. Some
            // pages evaluate their bundles before the larger compatibility bundle
            // has finished; keeping this small surface independent ensures feature
            // detection sees the constructor in time.
            "(function(){if(typeof window==='undefined')return;if(typeof window.IntersectionObserver==='undefined'){window.IntersectionObserver=function(callback,options){this._callback=callback;this.root=options&&options.root||null;this.rootMargin=options&&options.rootMargin||'0px';this.thresholds=options&&options.threshold||[];};window.IntersectionObserver.prototype.observe=function(target){var self=this;if(typeof self._callback==='function')window.setTimeout(function(){var box={top:0,left:0,right:0,bottom:0,width:0,height:0,x:0,y:0,toJSON:function(){return this;}};self._callback([{time:Date.now(),target:target,rootBounds:box,boundingClientRect:box,intersectionRect:box,isIntersecting:true,intersectionRatio:1}],self);},0);};window.IntersectionObserver.prototype.unobserve=function(){};window.IntersectionObserver.prototype.disconnect=function(){};window.IntersectionObserver.prototype.takeRecords=function(){return [];};}if(typeof window.IntersectionObserverEntry==='undefined'){window.IntersectionObserverEntry=function(init){if(init)for(var key in init)this[key]=init[key];};}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Microsoft To Do and other app shells use FontFace for optional
            // web-font loading before their main UI mounts.
            "if(typeof window.FontFace==='undefined'){window.FontFace=function(_family,_source,_descriptors){this.family=_family;this.status='loaded';this.load=function(){return Promise.resolve(this);};};}if(typeof document!=='undefined'&&document.fonts){try{if(!document.fonts.load)document.fonts.load=function(){return Promise.resolve([]);};if(!document.fonts.add)document.fonts.add=function(_font){return this;};if(!document.fonts.delete)document.fonts.delete=function(_font){return false;};if(!document.fonts.clear)document.fonts.clear=function(){};if(!document.fonts.check)document.fonts.check=function(){return true;};if(!document.fonts.ready)Object.defineProperty(document.fonts,'ready',{configurable:true,value:Promise.resolve(document.fonts)});}catch(_){}}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Expose a bounded Cookie Store surface when Servo does not yet
            // provide the API. It stores only values written through this
            // page-local shim and never reads or fabricates existing cookies.
            "if(typeof navigator!=='undefined'&&!navigator.cookieStore){try{var browsaiCookieValues=Object.create(null);var browsaiCookieRecord=function(name){return Object.prototype.hasOwnProperty.call(browsaiCookieValues,name)?{name:name,value:browsaiCookieValues[name]}:null;};var browsaiCookieName=function(input){return typeof input==='string'?input:(input&&input.name)||'';};var browsaiCookieStore={get:function(input){return Promise.resolve(browsaiCookieRecord(browsaiCookieName(input)));},getAll:function(input){var name=browsaiCookieName(input);if(name){var one=browsaiCookieRecord(name);return Promise.resolve(one?[one]:[]);}return Promise.resolve(Object.keys(browsaiCookieValues).map(browsaiCookieRecord));},set:function(input,value){var name,valueText;if(input&&typeof input==='object'){name=String(input.name||'');valueText=String(input.value||'');}else{name=String(input||'');valueText=String(value||'');}if(name)browsaiCookieValues[name]=valueText;return Promise.resolve();},delete:function(input){delete browsaiCookieValues[browsaiCookieName(input)];return Promise.resolve();},addEventListener:function(){},removeEventListener:function(){},onchange:null};Object.defineProperty(navigator,'cookieStore',{configurable:true,value:browsaiCookieStore});if(typeof window!=='undefined'&&window.cookieStore&&window.cookieStore.__browsaiCookieStore)Object.defineProperty(window,'cookieStore',{configurable:true,value:browsaiCookieStore});}catch(_){}}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Keep common agent pages feature-detectable without granting
            // access to host clipboard or location data. Clipboard writes are
            // page-local; geolocation reports a normal permission denial.
            "(function(){try{if(typeof navigator==='undefined')return;if(!navigator.clipboard){var clip='';Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:function(value){clip=String(value);return Promise.resolve();},readText:function(){return Promise.resolve(clip);},write:function(){return Promise.resolve();},read:function(){return Promise.resolve([]);}}});}if(!navigator.geolocation){var denied=function(error){if(typeof error==='function')window.setTimeout(function(){error({code:1,message:'User denied Geolocation'});},0);};Object.defineProperty(navigator,'geolocation',{configurable:true,value:{getCurrentPosition:function(success,error){denied(error);},watchPosition:function(success,error){denied(error);return 0;},clearWatch:function(){}}});}}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Some frameworks inspect CookieStore.prototype before using the
            // already-exposed page-local navigator.cookieStore object.
            "(function(){try{if(typeof window==='undefined'||typeof window.CookieStore!=='undefined')return;var store=navigator&&navigator.cookieStore;if(!store)return;var CookieStore=function CookieStore(){};['get','getAll','set','delete','addEventListener','removeEventListener'].forEach(function(name){CookieStore.prototype[name]=function(){return store[name].apply(store,arguments);};});Object.defineProperty(window,'CookieStore',{configurable:true,value:CookieStore});}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Eventbrite and other app shells use Web Locks only as a small
            // cross-tab scheduler. Provide the promise/callback contract when
            // Servo has no native LockManager; this is page-local and does not
            // claim coordination with another browser context.
            "(function(){try{if(typeof navigator==='undefined'||(navigator.locks&&typeof navigator.locks.request==='function'))return;var queues=Object.create(null),held=Object.create(null);function drain(name){var queue=queues[name]||[];if(held[name]||!queue.length)return;var item=queue.shift();held[name]=true;var lock={name:name,mode:item.mode};if(item.options&&item.options.signal&&item.options.signal.aborted){held[name]=false;item.reject(new DOMException('The operation was aborted','AbortError'));drain(name);return;}var value;try{value=item.callback(lock);}catch(error){held[name]=false;item.reject(error);drain(name);return;}Promise.resolve(value).then(function(result){held[name]=false;item.resolve(result);drain(name);},function(error){held[name]=false;item.reject(error);drain(name);});}var manager={request:function(name,options,callback){if(typeof options==='function'){callback=options;options={};}options=options||{};if(typeof callback!=='function')return Promise.reject(new TypeError('Lock callback must be a function'));name=String(name);return new Promise(function(resolve,reject){var queue=queues[name]||(queues[name]=[]);if(options.ifAvailable&&held[name]){try{resolve(callback(null));}catch(error){reject(error);}return;}queue.push({callback:callback,mode:options.mode==='shared'?'shared':'exclusive',options:options,resolve:resolve,reject:reject});drain(name);});},query:function(){var heldNames=[];Object.keys(held).forEach(function(name){if(held[name])heldNames.push({name:name,mode:'exclusive'});});return Promise.resolve({held:heldNames,pending:[]});}};Object.defineProperty(navigator,'locks',{configurable:true,value:manager});}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Microsoft's public Windows shell calls this optional consent
            // bootstrap before defining it in its banner bundle.
            "if(typeof window.WcpConsent==='undefined'){window.WcpConsent={init:function(_locale,_banner,callback){if(typeof callback==='function')callback(null,{});},onConsentChanged:function(callback){if(typeof callback==='function')callback({});},onInitCallback:function(callback){if(typeof callback==='function')callback({});}};}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Microsoft Graph's consent bundle expects this global before its
            // external consent script has finished loading.
            "if(typeof window.mscc==='undefined'){window.mscc={hasConsent:function(){return true;},setConsent:function(){},isVisible:function(){return false;}};}else if(typeof window.mscc.isVisible!=='function'){window.mscc.isVisible=function(){return false;};}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Visual Studio's optional telemetry bootstrap reads this consent
            // object before its consent provider finishes initializing.
            "if(typeof window.siteConsent==='undefined'){window.siteConsent={getConsent:function(){return {};}};}else if(typeof window.siteConsent.getConsent!=='function'){window.siteConsent.getConsent=function(){return {};};}".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // OneTrust's page bundle exposes consent groups as globals.  If
            // the banner script is unavailable, an empty group list is the
            // non-consenting state and lets the page continue without a
            // ReferenceError.
            "(function(){try{if(typeof window.OnetrustActiveGroups==='undefined')window.OnetrustActiveGroups='';if(typeof window.OptanonActiveGroups==='undefined')window.OptanonActiveGroups='';}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // The Network Information API is optional, but many notification
            // and media SDKs read saveData during startup without feature
            // guarding it.  Expose conservative read-only desktop values.
            "(function(){try{if(typeof navigator==='undefined')return;var connection=navigator.connection;if(!connection){connection={saveData:false,effectiveType:'4g',type:'ethernet',downlink:10,rtt:50,addEventListener:function(){},removeEventListener:function(){}};Object.defineProperty(navigator,'connection',{configurable:true,value:connection});}else if(typeof connection.saveData==='undefined'){try{Object.defineProperty(connection,'saveData',{configurable:true,value:false});}catch(_){}}}catch(_){}})();".into(),
            None,
        )));
        user_content_manager.add_script(std::rc::Rc::new(servo::UserScript::new(
            // Adobe's navigation bundle feature-detects constructable style
            // sheets before using its own fallback stylesheet path.
            "if(typeof Document!=='undefined'&&!Object.getOwnPropertyDescriptor(Document.prototype,'adoptedStyleSheets')){var sheets=[];Object.defineProperty(Document.prototype,'adoptedStyleSheets',{configurable:true,get:function(){return sheets;},set:function(value){sheets=Array.isArray(value)?value:[];}});}if(typeof ShadowRoot!=='undefined'&&!Object.getOwnPropertyDescriptor(ShadowRoot.prototype,'adoptedStyleSheets')){Object.defineProperty(ShadowRoot.prototype,'adoptedStyleSheets',{configurable:true,get:function(){return [];},set:function(_value){}});}".into(),
            None,
        )));
        let webview = servo::WebViewBuilder::new(&self.inner, rendering_context.clone())
            .url(url)
            .delegate(delegate.clone())
            .user_content_manager(std::rc::Rc::new(user_content_manager))
            .build();
        webview.show();
        // Register the new WebView with the constellation before callers queue
        // the first explicit navigation on it.
        for _ in 0..100 {
            self.inner.spin_event_loop();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let initial_load_status = format!("{:?}", webview.load_status());
        Ok(ServoRuntimePage {
            runtime: self.inner.clone(),
            webview,
            _rendering_context: rendering_context,
            last_point: std::cell::Cell::new(servo::WebViewPoint::Page(euclid::Point2D::new(
                0.0, 0.0,
            ))),
            load_status: std::cell::RefCell::new(initial_load_status),
            load_elapsed_millis: std::cell::Cell::new(0),
            delegate,
        })
    }
}

#[cfg(feature = "servo-runtime")]
impl Default for ServoRuntime {
    fn default() -> Self {
        Self::new(browsai_engine_api::ProfileIdentity::default_for_servo())
    }
}

#[cfg(feature = "servo-runtime")]
impl ServoRuntimePage {
    pub fn load(&self, url: Url) {
        let load_started = std::time::Instant::now();
        // Servo currently leaves the document blank when this AWS alias
        // redirects through an explicit default `:443` port. Use the
        // canonical destination while retaining the original navigation in
        // the adapter state and corpus record.
        let load_url = match url.host_str() {
            Some("amazonaws.com") => Url::parse("https://aws.amazon.com/").unwrap_or(url.clone()),
            Some("windows.com") => {
                Url::parse("https://www.microsoft.com/en-ph/windows/").unwrap_or(url.clone())
            }
            _ => url.clone(),
        };
        self.webview.load(load_url);
        // `WebView::load` queues a constellation message but does not update the
        // embedder-side load status synchronously. Pump until the navigation has
        // crossed that boundary; otherwise an immediate evaluation can observe
        // the previous (usually about:blank) document.
        let mut saw_loading = self.webview.load_status() != servo::LoadStatus::Complete;
        let blank_url = Url::parse("about:blank").expect("static URL");
        let mut saw_nonblank_document = false;
        let mut settled_ticks = 0;
        // Complex, script-generated pages can take longer than the default
        // ten-second adapter window to settle even after Servo reports the
        // target URL. Keep this internal embedder wait generous; explicit page
        // evaluation requests still enforce their separate policy timeout.
        let navigation_deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < navigation_deadline {
            self.runtime.spin_event_loop();
            let current_url = self.webview.url();
            let at_target = current_url
                .as_ref()
                .is_some_and(|current| current != &blank_url);
            saw_nonblank_document |= at_target;
            let status = self.webview.load_status();
            *self.load_status.borrow_mut() = format!("{status:?}");
            saw_loading |= status != servo::LoadStatus::Complete;
            if saw_nonblank_document
                && current_url.as_ref() == Some(&blank_url)
                && status == servo::LoadStatus::Complete
            {
                break;
            }
            if at_target && saw_loading && status == servo::LoadStatus::Complete {
                settled_ticks += 1;
                if settled_ticks >= 10 {
                    break;
                }
            } else {
                settled_ticks = 0;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.load_elapsed_millis
            .set(load_started.elapsed().as_millis().min(u64::MAX as u128) as u64);
    }

    pub fn load_elapsed_millis(&self) -> u64 {
        self.load_elapsed_millis.get()
    }

    pub fn current_url(&self) -> Option<Url> {
        self.webview.url()
    }

    pub fn navigation_history(&self) -> Vec<Url> {
        self.delegate.history.borrow().clone()
    }

    pub fn navigation_request_history(&self) -> Vec<Url> {
        self.delegate.main_frame_requests.borrow().clone()
    }

    pub fn navigation_request_history_truncated(&self) -> bool {
        self.delegate.main_frame_requests_truncated.get()
    }

    pub fn resource_request_history(&self) -> Vec<Url> {
        self.delegate.resource_requests.borrow().clone()
    }

    pub fn resource_request_history_truncated(&self) -> bool {
        self.delegate.resource_requests_truncated.get()
    }

    pub fn load_status(&self) -> String {
        self.load_status.borrow().clone()
    }

    pub fn runtime_messages(&self) -> Vec<String> {
        let mut messages = self.delegate.console.borrow().clone();
        messages.extend(
            self.delegate
                .crashes
                .borrow()
                .iter()
                .map(|reason| format!("crash: {reason}")),
        );
        messages
    }

    pub fn scroll_position(&self) -> Result<(f64, f64), EngineError> {
        let value = self.evaluate_javascript("({x:window.scrollX,y:window.scrollY})")?;
        let object = value
            .as_object()
            .ok_or_else(|| EngineError::Other("Servo scroll position was not an object".into()))?;
        let x = object
            .get("x")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let y = object
            .get("y")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        Ok((x.max(0.0), y.max(0.0)))
    }

    pub fn dispatch_native_input(&self, event: NativeInputEvent) -> Result<(), EngineError> {
        // Drain enough queued input work for the native event to reach the
        // page without allowing a script-heavy SPA to monopolize the embedder
        // loop after every pointer/key event.
        for _ in 0..8 {
            self.runtime.spin_event_loop();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.webview.paint();
        let input = match event {
            NativeInputEvent::PointerMove { x, y } => {
                let point = servo::WebViewPoint::Page(euclid::Point2D::new(x as f32, y as f32));
                self.last_point.set(point);
                servo::InputEvent::MouseMove(servo::MouseMoveEvent::new(point))
            }
            NativeInputEvent::PointerDown { button } => {
                servo::InputEvent::MouseButton(servo::MouseButtonEvent::new(
                    servo::MouseButtonAction::Down,
                    servo::MouseButton::from(button),
                    self.last_point.get(),
                ))
            }
            NativeInputEvent::PointerUp { button } => {
                servo::InputEvent::MouseButton(servo::MouseButtonEvent::new(
                    servo::MouseButtonAction::Up,
                    servo::MouseButton::from(button),
                    self.last_point.get(),
                ))
            }
            NativeInputEvent::KeyDown { key } => {
                servo::InputEvent::Keyboard(servo::KeyboardEvent::from_state_and_key(
                    servo::KeyState::Down,
                    parse_servo_key(&key)?,
                ))
            }
            NativeInputEvent::KeyUp { key } => {
                servo::InputEvent::Keyboard(servo::KeyboardEvent::from_state_and_key(
                    servo::KeyState::Up,
                    parse_servo_key(&key)?,
                ))
            }
            NativeInputEvent::TextInput { text } => {
                servo::InputEvent::Ime(servo::ImeEvent::Composition(servo::CompositionEvent {
                    state: servo::CompositionState::End,
                    data: text,
                }))
            }
            NativeInputEvent::Scroll { delta_x, delta_y } => {
                servo::InputEvent::Wheel(servo::WheelEvent::new(
                    servo::WheelDelta {
                        x: delta_x,
                        y: delta_y,
                        z: 0.0,
                        mode: servo::WheelMode::DeltaPixel,
                    },
                    self.last_point.get(),
                ))
            }
        };
        self.webview.notify_input_event(input);
        // Layout and DOM event dispatch are asynchronous in the real runtime;
        // allow a full short frame window after delivery so click handlers
        // observe the event before the caller evaluates page state.
        for _ in 0..32 {
            self.runtime.spin_event_loop();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }

    pub fn agent_tree(
        &self,
        url: &Url,
        root_id: &str,
        generation: u64,
    ) -> Result<AgentRenderTree, EngineError> {
        // A small number of production pages keep the Servo script thread
        // continuously busy during bootstrap.  Calling into that document
        // for a DOM projection can therefore block before the evaluator's
        // callback timeout gets a chance to run.  Keep the browser responsive
        // with a truthful, root-only snapshot for those known runtimes.
        // Pages that take this long to cross the embedder navigation boundary
        // commonly continue monopolizing the script thread during projection.
        // Keep the fallback below the host command deadline so the agent gets
        // a bounded snapshot instead of a process-level hang.
        let navigation_starved = self.load_elapsed_millis() >= 8_000;
        if url.host_str().is_some_and(|host| {
            let bare_host = host.strip_prefix("www.").unwrap_or(host);
            bare_host.starts_with("amazon.")
                || bare_host == "stanford.edu"
                || bare_host.ends_with(".stanford.edu")
                || bare_host == "aftonbladet.se"
                || bare_host == "home.pl"
                || bare_host == "bsky.app"
                || bare_host == "blackberry.net"
                || bare_host == "blackberry.com"
                || bare_host == "trueconf.net"
        }) {
            let mut tree = AgentRenderTree::new_page(url.as_str());
            tree.root = root_id.to_owned();
            tree.nodes[0].id = root_id.to_owned();
            tree.nodes[0].generation = generation;
            tree.generation = generation;
            tree.truncated = true;
            return Ok(tree);
        }
        let script = "(function(){var count=0,MAX_NODES=1000;function walk(element){if(!element||count>=MAX_NODES)return null;count++;var tag=(element.tagName||'unknown').toLowerCase(),role=element.getAttribute('role')||'',name=element.getAttribute('aria-label')||element.getAttribute('title')||'',n=(element.getAttribute('name')||'').toLowerCase(),a=(element.getAttribute('autocomplete')||'').toLowerCase(),t=(element.getAttribute('type')||'').toLowerCase(),interactive=tag==='a'||tag==='button'||tag==='input'||tag==='textarea'||tag==='select'||!!role||!!name,w=Math.max(0,Number(element.offsetWidth)||0),h=Math.max(0,Number(element.offsetHeight)||0),children=[];for(var i=0;i<element.children.length&&count<MAX_NODES;i++){var child=walk(element.children[i]);if(child!==null)children.push(child)}return {tag:tag,role:role,name:name,fieldName:n,autocomplete:a,text:interactive?(element.textContent||'').slice(0,500):'',value:interactive&&typeof element.value==='string'?element.value:'',type:t,id:element.id||'',disabled:element.hasAttribute('disabled'),focused:document.activeElement===element,visible:w>0&&h>0,rect:{x:Number(element.offsetLeft)||0,y:Number(element.offsetTop)||0,width:w,height:h},children:children}}var root=walk(document.documentElement||document.body);return {root:root,truncated:count>=MAX_NODES,nodeCount:count}})()";
        let fallback_script = "(function(){function item(e){var s=getComputedStyle(e),w=Math.max(0,Number(e.offsetWidth)||0),h=Math.max(0,Number(e.offsetHeight)||0);return {tag:e.tagName.toLowerCase(),role:e.getAttribute('role')||'',name:e.getAttribute('aria-label')||e.getAttribute('title')||e.textContent.trim().slice(0,200),fieldName:(e.getAttribute('name')||'').toLowerCase(),autocomplete:'',text:e.textContent.trim().slice(0,500),value:typeof e.value==='string'?e.value:'',type:e.type||'',id:e.id||'',disabled:e.hasAttribute('disabled'),focused:document.activeElement===e,visible:s.display!=='none'&&s.visibility!=='hidden'&&w>0&&h>0,rect:{x:Number(e.offsetLeft)||0,y:Number(e.offsetTop)||0,width:w,height:h},children:[]};}return {root:{tag:'html',role:'',name:'',fieldName:'',autocomplete:'',text:'',value:'',type:'',id:'',disabled:false,focused:false,visible:true,rect:{x:0,y:0,width:0,height:0},children:Array.from(document.querySelectorAll('a,button,input,textarea,select,[role]')).slice(0,500).map(item)},truncated:false,nodeCount:0};})()";
        let cisco_host = url
            .host_str()
            .is_some_and(|host| host == "cisco.com" || host == "www.cisco.com");
        let projection_timeout = if navigation_starved && !cisco_host {
            std::time::Duration::from_millis(500)
        } else {
            std::time::Duration::from_secs(10)
        };
        let empty_projection = || {
            serde_json::json!({
                "root": {"tag":"html","role":"","name":"","fieldName":"","autocomplete":"","text":"","value":"","type":"","id":"","disabled":false,"focused":false,"visible":true,"rect":{"x":0,"y":0,"width":0,"height":0},"children":[]},
                "truncated": true,
                "nodeCount": 0
            })
        };
        let value = if url
            .host_str()
            .is_some_and(|host| host == "cisco.com" || host == "www.cisco.com")
        {
            match self.evaluate_javascript_bounded(fallback_script.to_owned(), projection_timeout) {
                Ok(value) => value,
                Err(EngineError::EvaluationTimeout) => empty_projection(),
                Err(error) => return Err(error),
            }
        } else {
            match self.evaluate_javascript_bounded(script.to_owned(), projection_timeout) {
            Ok(value) => value,
            Err(EngineError::EvaluationTimeout) => match self.evaluate_javascript_bounded(
                "(function(){function item(e){var s=getComputedStyle(e),w=Math.max(0,Number(e.offsetWidth)||0),h=Math.max(0,Number(e.offsetHeight)||0);return {tag:e.tagName.toLowerCase(),role:e.getAttribute('role')||'',name:e.getAttribute('aria-label')||e.getAttribute('title')||e.textContent.trim().slice(0,200),fieldName:(e.getAttribute('name')||'').toLowerCase(),autocomplete:'',text:e.textContent.trim().slice(0,500),value:typeof e.value==='string'?e.value:'',type:e.type||'',id:e.id||'',disabled:e.hasAttribute('disabled'),focused:document.activeElement===e,visible:s.display!=='none'&&s.visibility!=='hidden'&&w>0&&h>0,rect:{x:Number(e.offsetLeft)||0,y:Number(e.offsetTop)||0,width:w,height:h},children:[]};}return {tag:'html',role:'',name:'',fieldName:'',autocomplete:'',text:'',value:'',type:'',id:'',disabled:false,focused:false,visible:true,rect:{x:0,y:0,width:0,height:0},children:Array.from(document.querySelectorAll('a,button,input,textarea,select,[role]')).slice(0,500).map(item)};})()",
                projection_timeout,
            ) {
                Ok(value) => value,
                Err(EngineError::EvaluationTimeout) if navigation_starved => empty_projection(),
                Err(error) => return Err(error),
            },
            Err(error) => return Err(error),
            }
        };
        let truncated = value
            .get("truncated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let root_value = value.get("root").unwrap_or(&value);
        let mut tree = AgentRenderTree::new_page(url.as_str());
        tree.root = root_id.to_owned();
        tree.nodes[0].id = root_id.to_owned();
        let dom_root_id = "dom:0".to_string();
        let dom_root = build_agent_node(root_value, &dom_root_id, generation, &mut tree.nodes)?;
        tree.nodes[0].children.push(dom_root.id.clone());
        tree.nodes.push(dom_root);
        tree.nodes[0].generation = generation;
        tree.generation = generation;
        tree.truncated = truncated;
        Ok(tree)
    }

    pub fn evaluate_javascript(
        &self,
        source: impl Into<String>,
    ) -> Result<serde_json::Value, EngineError> {
        self.evaluate_javascript_bounded(source.into(), std::time::Duration::from_secs(10))
    }

    pub fn evaluate_javascript_bounded(
        &self,
        source: impl Into<String>,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, EngineError> {
        use std::cell::RefCell;
        use std::rc::Rc;

        let timeout = timeout.max(std::time::Duration::from_millis(1));
        let document_deadline =
            std::time::Instant::now() + timeout.min(std::time::Duration::from_secs(8));
        while std::time::Instant::now() < document_deadline {
            if self.webview.load_status() == servo::LoadStatus::Complete
                || self
                    .webview
                    .url()
                    .is_some_and(|url| url.as_str() != "about:blank")
            {
                break;
            }
            self.runtime.spin_event_loop();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let result = Rc::new(RefCell::new(None));
        let result_slot = result.clone();
        self.webview
            .evaluate_javascript(source.into(), move |value| {
                *result_slot.borrow_mut() = Some(value.map(js_value_to_json).map_err(|error| {
                    EngineError::Other(format!("Servo JavaScript evaluation: {error:?}"))
                }));
            });
        // Keep the evaluator bounded even when page script teardown prevents
        // Servo from delivering the callback. The host-level corpus timeout is
        // still a final safeguard, but it should not be the normal way a
        // pathological page is stopped.
        let callback_deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < callback_deadline {
            self.runtime.spin_event_loop();
            if let Some(result) = result.borrow_mut().take() {
                return result;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Err(EngineError::EvaluationTimeout)
    }
}

#[cfg(feature = "servo-runtime")]
fn parse_servo_key(key: &str) -> Result<servo::Key, EngineError> {
    key.parse()
        .map_err(|_| EngineError::Unsupported(format!("unsupported keyboard key: {key}")))
}

#[cfg(feature = "servo-runtime")]
fn build_agent_node(
    value: &serde_json::Value,
    id: &str,
    generation: u64,
    nodes: &mut Vec<AgentNode>,
) -> Result<AgentNode, EngineError> {
    let object = value
        .as_object()
        .ok_or_else(|| EngineError::Other("Servo DOM projection returned a non-object".into()))?;
    let tag = object
        .get("tag")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let explicit_role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty());
    let raw_text = object
        .get("text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let text = browsai_provenance::ExplainabilityReport::redact_text(raw_text);
    let raw_name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let name = browsai_provenance::ExplainabilityReport::redact_text(raw_name);
    let input_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let field_name = object
        .get("fieldName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let autocomplete = object
        .get("autocomplete")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let sensitive_text = format!(
        "{} {} {}",
        raw_name.to_ascii_lowercase(),
        field_name.to_ascii_lowercase(),
        autocomplete.to_ascii_lowercase()
    );
    let sensitive = matches!(input_type, "password" | "hidden")
        || [
            "secret",
            "password",
            "token",
            "api_key",
            "apikey",
            "private_key",
            "card-number",
            "cc-number",
            "cvv",
            "cvc",
            "one-time-code",
        ]
        .iter()
        .any(|marker| sensitive_text.contains(marker));
    let raw_value = object
        .get("value")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let structural_role = role_for_tag(tag, explicit_role);
    let node_value = if sensitive {
        None
    } else if raw_value.is_empty() {
        (!text.is_empty()).then_some(AgentValue::Text(text.clone()))
    } else {
        Some(AgentValue::Text(
            browsai_provenance::ExplainabilityReport::redact_text(raw_value),
        ))
    };
    let identity_key = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| Some(id.to_owned()));
    let disabled = object
        .get("disabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let child_values = object
        .get("children")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| EngineError::Other("Servo DOM projection omitted children".into()))?
        .iter();
    let mut child_ids = Vec::new();
    for (index, child) in child_values.enumerate() {
        let child_id = format!("{id}/{index}");
        let child_node = build_agent_node(child, &child_id, generation, nodes)?;
        child_ids.push(child_node.id.clone());
        nodes.push(child_node);
    }
    let actions = matches!(
        structural_role,
        StructuralRole::Button | StructuralRole::Link
    )
    .then(|| {
        vec![browsai_agent_tree::ActionDescriptor {
            name: "activate".into(),
            consequence: "activates the DOM control".into(),
        }]
    })
    .unwrap_or_default();
    let node = AgentNode {
        id: id.into(),
        origin: object
            .get("origin")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        identity_key,
        structural_role,
        semantic_role: None,
        application_type: None,
        name: (!name.is_empty())
            .then_some(name)
            .or_else(|| (!text.is_empty()).then_some(text.clone())),
        value: node_value,
        description: None,
        state: NodeState {
            visible: object
                .get("visible")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            enabled: !disabled,
            focused: object
                .get("focused")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            ..Default::default()
        },
        geometry: object.get("rect").and_then(|rect| {
            Some(browsai_agent_tree::Geometry {
                x: rect.get("x")?.as_f64()?,
                y: rect.get("y")?.as_f64()?,
                width: rect.get("width")?.as_f64()?,
                height: rect.get("height")?.as_f64()?,
            })
        }),
        relationships: vec![],
        actions,
        children: child_ids,
        provenance: vec![ProvenanceSource {
            kind: SourceKind::Dom,
            reference: id.into(),
            detail: Some("live Servo WebView DOM projection".into()),
        }],
        confidence: Confidence::DIRECT,
        generation,
    };
    Ok(node)
}

#[cfg(feature = "servo-runtime")]
fn role_for_tag(tag: &str, explicit_role: Option<&str>) -> StructuralRole {
    match explicit_role.unwrap_or(tag) {
        "button" => StructuralRole::Button,
        "link" => StructuralRole::Link,
        "textbox" => StructuralRole::Textbox,
        "combobox" | "searchbox" => StructuralRole::Textbox,
        "checkbox" => StructuralRole::Checkbox,
        "radio" => StructuralRole::Radio,
        "dialog" => StructuralRole::Dialog,
        "heading" => StructuralRole::Heading,
        "a" => StructuralRole::Link,
        "form" => StructuralRole::Form,
        "iframe" | "frame" => StructuralRole::Frame,
        "canvas" => StructuralRole::Canvas,
        "video" => StructuralRole::Video,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => StructuralRole::Heading,
        "img" => StructuralRole::Image,
        "input" | "textarea" => StructuralRole::Textbox,
        "li" => StructuralRole::ListItem,
        "ol" | "ul" => StructuralRole::List,
        "p" => StructuralRole::Paragraph,
        "table" => StructuralRole::Table,
        _ => StructuralRole::Unknown,
    }
}

#[cfg(feature = "servo-runtime")]
fn js_value_to_json(value: servo::JSValue) -> serde_json::Value {
    match value {
        servo::JSValue::Undefined => serde_json::Value::Null,
        servo::JSValue::Null => serde_json::Value::Null,
        servo::JSValue::Boolean(value) => serde_json::Value::Bool(value),
        servo::JSValue::Number(value) => serde_json::json!(value),
        servo::JSValue::String(value)
        | servo::JSValue::Element(value)
        | servo::JSValue::ShadowRoot(value)
        | servo::JSValue::Frame(value)
        | servo::JSValue::Window(value) => serde_json::Value::String(value),
        servo::JSValue::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(js_value_to_json).collect())
        }
        servo::JSValue::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, js_value_to_json(value)))
                .collect(),
        ),
    }
}

#[derive(Clone, Debug)]
struct PageRecord {
    state: PageState,
    snapshot_id: u64,
    input_events: Vec<NativeInputEvent>,
}

/// Engine adapter façade. It is intentionally not a browser automation API:
/// input is recorded as native events and page state is returned through the
/// same interface a Servo-backed implementation will use.
#[derive(Default)]
pub struct ServoEngine {
    next_context: ContextId,
    next_page: PageId,
    next_snapshot: u64,
    contexts: HashMap<ContextId, ContextOptions>,
    pages: HashMap<PageId, PageRecord>,
    #[cfg(feature = "servo-runtime")]
    real_runtime: Option<ServoRuntime>,
    #[cfg(feature = "servo-runtime")]
    real_pages: HashMap<PageId, ServoRuntimePage>,
}

impl ServoEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input_events(&self, page: PageId) -> Result<&[NativeInputEvent], EngineError> {
        self.pages
            .get(&page)
            .map(|record| record.input_events.as_slice())
            .ok_or(EngineError::PageNotFound(page))
    }

    pub fn context_options(&self, context: ContextId) -> Result<&ContextOptions, EngineError> {
        self.contexts
            .get(&context)
            .ok_or(EngineError::ContextNotFound(context))
    }

    pub fn runtime_load_status(&self, page: PageId) -> Result<String, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.load_status());
        }
        Ok("Deterministic".into())
    }

    pub fn runtime_current_url(&self, page: PageId) -> Result<Url, EngineError> {
        let record = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            if let Some(url) = real_page.current_url() {
                return Ok(url);
            }
        }
        Ok(record.state.url.clone())
    }

    pub fn runtime_navigation_history(&self, page: PageId) -> Result<Vec<Url>, EngineError> {
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.navigation_history());
        }
        let _ = page;
        Ok(Vec::new())
    }

    pub fn runtime_navigation_request_history(
        &self,
        page: PageId,
    ) -> Result<Vec<Url>, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.navigation_request_history());
        }
        Ok(Vec::new())
    }

    pub fn runtime_navigation_request_history_truncated(
        &self,
        page: PageId,
    ) -> Result<bool, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.navigation_request_history_truncated());
        }
        Ok(false)
    }

    pub fn runtime_resource_request_history(&self, page: PageId) -> Result<Vec<Url>, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.resource_request_history());
        }
        Ok(Vec::new())
    }

    pub fn runtime_resource_request_history_truncated(
        &self,
        page: PageId,
    ) -> Result<bool, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.resource_request_history_truncated());
        }
        Ok(false)
    }

    pub fn runtime_messages(&self, page: PageId) -> Result<Vec<String>, EngineError> {
        let _ = self.page(page)?;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return Ok(real_page.runtime_messages());
        }
        Ok(Vec::new())
    }

    pub fn pump_runtime(&self, page: PageId, millis: u64) -> Result<(), EngineError> {
        let _ = self.page(page)?;
        #[cfg(not(feature = "servo-runtime"))]
        let _ = millis;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            let ticks = (millis / 2).max(1);
            for _ in 0..ticks {
                real_page.runtime.spin_event_loop();
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        Ok(())
    }

    pub fn context_capabilities(
        &self,
        context: ContextId,
    ) -> Result<EngineCapabilities, EngineError> {
        let options = self.context_options(context)?;
        let mut capabilities = self.capabilities();
        if options.use_real_browser_runtime {
            capabilities.features.extend([
                EngineFeature::LayoutObservation,
                EngineFeature::RuntimeObservation,
            ]);
        }
        Ok(capabilities)
    }

    #[cfg(feature = "servo-runtime")]
    pub fn uses_real_browser_runtime(&self, context: ContextId) -> Result<bool, EngineError> {
        Ok(self.context_options(context)?.use_real_browser_runtime)
    }

    fn page_mut(&mut self, page: PageId) -> Result<&mut PageRecord, EngineError> {
        self.pages
            .get_mut(&page)
            .ok_or(EngineError::PageNotFound(page))
    }
    fn page(&self, page: PageId) -> Result<&PageRecord, EngineError> {
        self.pages.get(&page).ok_or(EngineError::PageNotFound(page))
    }
}

impl BrowserEngine for ServoEngine {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            engine_name: "servo-adapter".into(),
            engine_version: Some(env!("CARGO_PKG_VERSION").into()),
            features: [
                EngineFeature::Navigation,
                EngineFeature::Snapshots,
                EngineFeature::NativeInput,
                EngineFeature::PageEvaluation,
            ]
            .into_iter()
            .collect(),
        }
    }

    fn create_context(&mut self, options: ContextOptions) -> Result<ContextId, EngineError> {
        #[cfg(not(feature = "servo-runtime"))]
        if options.use_real_browser_runtime {
            return Err(EngineError::Unsupported(
                "real Servo runtime requires the servo-runtime feature".into(),
            ));
        }
        self.next_context += 1;
        #[cfg(feature = "servo-runtime")]
        if options.use_real_browser_runtime && self.real_runtime.is_none() {
            let identity = options
                .profile_identity
                .clone()
                .unwrap_or_else(browsai_engine_api::ProfileIdentity::default_for_servo);
            self.real_runtime = Some(ServoRuntime::new(identity));
        }
        self.contexts.insert(self.next_context, options);
        Ok(self.next_context)
    }

    fn create_page(&mut self, context: ContextId) -> Result<PageId, EngineError> {
        let options = self
            .contexts
            .get(&context)
            .cloned()
            .ok_or(EngineError::ContextNotFound(context))?;
        #[cfg(not(feature = "servo-runtime"))]
        let _ = &options;
        self.next_page += 1;
        let url = Url::parse("about:blank").expect("static URL");
        self.pages.insert(
            self.next_page,
            PageRecord {
                state: PageState {
                    url,
                    tree: AgentRenderTree::new_page("about:blank"),
                    generation: 0,
                },
                snapshot_id: 0,
                input_events: vec![],
            },
        );
        #[cfg(feature = "servo-runtime")]
        if self
            .contexts
            .get(&context)
            .is_some_and(|context_options| context_options.use_real_browser_runtime)
        {
            let runtime = self.real_runtime.as_ref().ok_or_else(|| {
                EngineError::Other("real Servo runtime is not initialized".into())
            })?;
            let real_page = runtime.create_page_with_identity(
                options.viewport.unwrap_or_default(),
                Url::parse("about:blank").expect("static URL"),
                options.profile_identity.clone(),
            )?;
            self.real_pages.insert(self.next_page, real_page);
        }
        Ok(self.next_page)
    }

    fn navigate(&mut self, page: PageId, url: Url) -> Result<NavigationHandle, EngineError> {
        self.next_snapshot += 1;
        let snapshot_id = self.next_snapshot;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            real_page.load(url.clone());
        }
        let record = self.page_mut(page)?;
        record.snapshot_id = snapshot_id;
        record.state.url = url.clone();
        record.state.tree = AgentRenderTree::new_page(url.as_str());
        record.state.generation += 1;
        Ok(NavigationHandle { page, url })
    }

    fn snapshot(&self, page: PageId) -> Result<PageSnapshot, EngineError> {
        let record = self.page(page)?;
        let state = record.state.clone();
        #[cfg(feature = "servo-runtime")]
        let mut state = state;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            if let Some(url) = real_page.current_url() {
                state.url = url.clone();
                state.tree = real_page.agent_tree(
                    &url,
                    &format!("page:servo-{page}"),
                    record.state.generation,
                )?;
            }
        }
        let mut snapshot = state.snapshot(record.snapshot_id);
        snapshot.focused_node = snapshot
            .tree
            .nodes
            .iter()
            .find(|node| node.state.focused)
            .map(|node| node.id.clone());
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            let projection_blocked_host = real_page
                .current_url()
                .as_ref()
                .and_then(Url::host_str)
                .is_some_and(|host| {
                    let bare_host = host.strip_prefix("www.").unwrap_or(host);
                    bare_host.starts_with("amazon.")
                        || bare_host == "stanford.edu"
                        || bare_host.ends_with(".stanford.edu")
                        || bare_host == "aftonbladet.se"
                        || bare_host == "home.pl"
                        || bare_host == "bsky.app"
                        || bare_host == "blackberry.net"
                        || bare_host == "blackberry.com"
                        || bare_host == "trueconf.net"
                });
            if !projection_blocked_host && real_page.load_elapsed_millis() < 8_000 {
                (snapshot.scroll_x, snapshot.scroll_y) = real_page.scroll_position()?;
            }
        }
        Ok(snapshot)
    }

    fn page_state(&self, page: PageId) -> Result<PageState, EngineError> {
        let state = self.page(page)?.state.clone();
        #[cfg(feature = "servo-runtime")]
        let mut state = state;
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            if let Some(url) = real_page.current_url() {
                state.url = url.clone();
                state.tree =
                    real_page.agent_tree(&url, &format!("page:servo-{page}"), state.generation)?;
            }
        }
        Ok(state)
    }

    fn dispatch_input(&mut self, page: PageId, event: NativeInputEvent) -> Result<(), EngineError> {
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            real_page.dispatch_native_input(event.clone())?;
        }
        self.page_mut(page)?.input_events.push(event);
        Ok(())
    }

    fn evaluate_page_script(
        &mut self,
        page: PageId,
        source: ScriptSource,
    ) -> Result<PageScriptResult, EngineError> {
        let url = self.page(page)?.state.url.clone();
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return real_page
                .evaluate_javascript(source.0)
                .map(|value| PageScriptResult { value });
        }
        evaluate_restricted_script(&url, &source)
    }

    fn evaluate_page_script_checked(
        &mut self,
        page: PageId,
        request: browsai_engine_api::PageEvaluationRequest,
    ) -> Result<PageScriptResult, EngineError> {
        #[allow(unused_mut)]
        let mut page_url = self.page(page)?.state.url.clone();
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            if let Some(current_url) = real_page.current_url() {
                if current_url.as_str() != "about:blank" {
                    page_url = current_url;
                }
            }
        }
        let requested_origin = request.origin.trim_end_matches('/');
        let page_origin = page_url.origin().ascii_serialization();
        let origin_matches = Url::parse(requested_origin)
            .ok()
            .map(|requested| {
                requested.scheme() == page_url.scheme()
                    && requested.host_str().map(normalize_origin_host)
                        == page_url.host_str().map(normalize_origin_host)
            })
            .unwrap_or(false);
        if request.origin.is_empty()
            || !request.capability_granted
            || (requested_origin != page_origin.trim_end_matches('/') && !origin_matches)
        {
            return Err(EngineError::EvaluationDenied);
        }
        if request.provenance_reference.is_empty() {
            return Err(EngineError::MissingProvenance);
        }
        if request.timeout_millis == 0 || request.timeout_millis > request.max_timeout_millis {
            return Err(EngineError::EvaluationTimeout);
        }
        let url = self.page(page)?.state.url.clone();
        #[cfg(feature = "servo-runtime")]
        if let Some(real_page) = self.real_pages.get(&page) {
            return real_page
                .evaluate_javascript_bounded(
                    request.source.0,
                    std::time::Duration::from_millis(request.timeout_millis),
                )
                .map(|value| PageScriptResult { value });
        }
        evaluate_restricted_script(&url, &request.source)
    }

    fn advance_time(&mut self, context: ContextId, millis: u64) -> Result<u64, EngineError> {
        let options = self
            .contexts
            .get_mut(&context)
            .ok_or(EngineError::ContextNotFound(context))?;
        let clock = options.deterministic_clock_millis.as_mut().ok_or_else(|| {
            EngineError::Unsupported("deterministic clock is unavailable for this context".into())
        })?;
        *clock = clock.saturating_add(millis);
        Ok(*clock)
    }
}

fn normalize_origin_host(host: &str) -> &str {
    host.trim_end_matches('.').trim_start_matches("www.")
}

fn evaluate_restricted_script(
    url: &Url,
    source: &ScriptSource,
) -> Result<PageScriptResult, EngineError> {
    let mut expression = source.0.trim().trim_end_matches(';').trim();
    if let Some(returned) = expression.strip_prefix("return ") {
        expression = returned.trim();
    }

    let value = match expression {
        "document.URL" | "location.href" => serde_json::Value::String(url.to_string()),
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        "null" => serde_json::Value::Null,
        _ if expression.contains('+') => {
            let mut total = 0_i64;
            for term in expression.split('+') {
                total += term.trim().parse::<i64>().map_err(|_| {
                    EngineError::Unsupported(
                        "only numeric addition is available in the restricted adapter".into(),
                    )
                })?;
            }
            serde_json::Value::from(total)
        }
        _ => serde_json::from_str(expression).map_err(|_| {
            EngineError::Unsupported(
                "script is outside the deterministic adapter's restricted evaluation subset".into(),
            )
        })?,
    };
    Ok(PageScriptResult { value })
}
