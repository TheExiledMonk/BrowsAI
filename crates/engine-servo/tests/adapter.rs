use browsai_engine_api::{
    BrowserEngine, ContextOptions, EngineError, EngineFeature, PageEvaluationRequest, ScriptSource,
    VirtualViewport,
};
use browsai_engine_servo::ServoEngine;
use browsai_input::NativeInputEvent;
#[cfg(feature = "servo-runtime")]
use data_encoding::BASE64;
#[cfg(feature = "servo-runtime")]
use sha1::{Digest, Sha1};
#[cfg(feature = "servo-runtime")]
use std::io::{Read, Write};
#[cfg(feature = "servo-runtime")]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
#[cfg(feature = "servo-runtime")]
use std::time::Duration;
use url::Url;

#[cfg(feature = "servo-runtime")]
// The mutex prevents concurrent fixture servers when a single runtime test is
// selected. Servo's options are process-global, so each real-runtime fixture
// must be run in its own test process; the default workspace suite ignores them.
static REAL_RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn adapter_keeps_navigation_and_input_inside_engine_contract() {
    let mut engine = ServoEngine::new();
    let context = engine.create_context(ContextOptions::default()).unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/invoices").unwrap())
        .unwrap();
    engine
        .dispatch_input(page, NativeInputEvent::KeyDown { key: "Tab".into() })
        .unwrap();

    let snapshot = engine.snapshot(page).unwrap();
    assert_eq!(snapshot.url.as_str(), "https://example.test/invoices");
    assert_eq!(
        snapshot.tree.nodes[0].name.as_deref(),
        Some("https://example.test/invoices")
    );
    assert_eq!(engine.input_events(page).unwrap().len(), 1);
}

#[test]
fn adapter_reports_supported_features_including_restricted_page_execution() {
    let engine = ServoEngine::new();
    let capabilities = engine.capabilities();
    assert!(capabilities.features.contains(&EngineFeature::Navigation));
    assert!(capabilities
        .features
        .contains(&EngineFeature::PageEvaluation));
}

#[test]
fn headless_context_preserves_virtual_viewport_and_deterministic_options() {
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            headless: true,
            viewport: Some(VirtualViewport {
                width: 800,
                height: 600,
                device_scale_factor: 1.0,
            }),
            deterministic_clock_millis: Some(123),
            no_raster: true,
            ..Default::default()
        })
        .unwrap();
    let options = engine.context_options(context).unwrap();
    assert!(options.headless && options.no_raster);
    assert_eq!(options.deterministic_clock_millis, Some(123));
    assert_eq!(options.viewport.unwrap().width, 800);
}

#[cfg(not(feature = "servo-runtime"))]
#[test]
fn real_runtime_requests_fail_without_the_runtime_feature() {
    let mut engine = ServoEngine::new();
    assert!(matches!(
        engine.create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        }),
        Err(EngineError::Unsupported(_))
    ));
}

#[test]
fn capabilities_are_discoverable_per_context() {
    let mut engine = ServoEngine::new();
    let context = engine.create_context(ContextOptions::default()).unwrap();
    let capabilities = engine.context_capabilities(context).unwrap();
    assert!(capabilities.supports(EngineFeature::Navigation));
    assert!(capabilities.supports(EngineFeature::PageEvaluation));
}

#[test]
fn restricted_page_evaluation_requires_capability_timeout_and_provenance() {
    let mut engine = ServoEngine::new();
    let context = engine.create_context(ContextOptions::default()).unwrap();
    let page = engine.create_page(context).unwrap();
    let request = PageEvaluationRequest {
        source: ScriptSource("1 + 1".into()),
        origin: "https://example.test".into(),
        capability_granted: false,
        timeout_millis: 100,
        max_timeout_millis: 1_000,
        provenance_reference: "action-1".into(),
    };
    assert!(matches!(
        engine.evaluate_page_script_checked(page, request),
        Err(EngineError::EvaluationDenied)
    ));
}

#[test]
fn restricted_page_evaluation_executes_against_the_selected_page() {
    let mut engine = ServoEngine::new();
    let context = engine.create_context(ContextOptions::default()).unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/invoices").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("document.URL".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "action-1".into(),
            },
        )
        .unwrap();
    assert_eq!(
        result.value,
        serde_json::json!("https://example.test/invoices")
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this legacy fixture separately"]
fn selected_engine_runtime_executes_javascript() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            viewport: Some(VirtualViewport::default()),
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/compute").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("3 + 4".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    assert_eq!(result.value, serde_json::json!(7.0));
    let security_surface = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var out={secure:window.isSecureContext,origin:location.origin,idbIndex:typeof IDBIndex};function probe(name,fn){try{fn();out[name]='ok'}catch(e){out[name]={name:e&&e.name||null,message:e&&e.message||String(e)}}}probe('crypto',function(){var b=new Uint8Array(8);crypto.getRandomValues(b)});probe('localStorage',function(){localStorage.setItem('__browsai_probe','1');localStorage.removeItem('__browsai_probe')});probe('indexedDB',function(){indexedDB.open('__browsai_probe')});probe('documentDomain',function(){void document.domain});probe('serviceWorker',function(){void navigator.serviceWorker});return out})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "security-context-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(security_surface.value["secure"], serde_json::json!(true));
    assert_eq!(
        security_surface.value["origin"],
        serde_json::json!("https://example.test")
    );
    assert_eq!(security_surface.value["crypto"], serde_json::json!("ok"));
    assert_eq!(
        security_surface.value["idbIndex"],
        serde_json::json!("function")
    );
    assert_eq!(
        security_surface.value["localStorage"]["name"],
        serde_json::json!("SecurityError")
    );
    assert_eq!(
        security_surface.value["indexedDB"]["name"],
        serde_json::json!("SecurityError")
    );
    engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "document.body.innerHTML = '<input type=\\\"password\\\" value=\\\"hunter2\\\">'; null"
                        .into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    let snapshot = engine.snapshot(page).unwrap();
    assert!(snapshot.tree.nodes.len() > 1);
    assert!(snapshot.tree.nodes.iter().any(|node| {
        node.provenance
            .iter()
            .any(|source| source.reference == "dom:0")
    }));
    let second_snapshot = engine.snapshot(page).unwrap();
    assert_eq!(snapshot.tree.root, second_snapshot.tree.root);
    assert_eq!(snapshot.tree.nodes[1].id, second_snapshot.tree.nodes[1].id);
    assert!(snapshot.tree.nodes.iter().all(|node| {
        node.value != Some(browsai_agent_tree::AgentValue::Text("hunter2".into()))
    }));
    engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("document.body.innerHTML='<input name=\\\"api_token\\\" value=\\\"opaque-token\\\">'; null".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    let secret_snapshot = engine.snapshot(page).unwrap();
    assert!(secret_snapshot.tree.nodes.iter().all(|node| {
        node.value != Some(browsai_agent_tree::AgentValue::Text("opaque-token".into()))
    }));
    engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__clicks=0; document.body.innerHTML='<button id=\\\"go\\\">Go</button>'; document.getElementById('go').onclick=function(){window.__clicks++}; null".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    engine
        .dispatch_input(page, NativeInputEvent::PointerMove { x: 20.0, y: 20.0 })
        .unwrap();
    engine
        .dispatch_input(page, NativeInputEvent::PointerDown { button: 0 })
        .unwrap();
    engine
        .dispatch_input(page, NativeInputEvent::PointerUp { button: 0 })
        .unwrap();
    let clicks = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__clicks".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    assert_eq!(clicks.value, serde_json::json!(1.0));

    engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("document.body.innerHTML='<input id=\\\"name\\\">'; document.getElementById('name').focus(); null".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    engine
        .dispatch_input(
            page,
            NativeInputEvent::TextInput {
                text: "hello".into(),
            },
        )
        .unwrap();
    let input_value = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("document.getElementById('name').value".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    assert_eq!(input_value.value, serde_json::json!("hello"));
    let focused_snapshot = engine.snapshot(page).unwrap();
    assert!(focused_snapshot
        .tree
        .nodes
        .iter()
        .any(|node| node.state.focused));
    assert!(focused_snapshot.focused_node.is_some());

    engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "document.body.innerHTML='<div style=\"height:2000px\">tall</div>'; window.scrollTo(0,120); null"
                        .into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "runtime-test".into(),
            },
        )
        .unwrap();
    let scrolled_snapshot = engine.snapshot(page).unwrap();
    assert!(scrolled_snapshot.scroll_y >= 0.0);
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exposes_native_canvas_apis_and_worker_primitives() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(
            page,
            Url::parse("https://example.test/canvas-compat").unwrap(),
        )
        .unwrap();

    let window_apis = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var c=new OffscreenCanvas(8,4);var ctx=c.getContext('2d');var canvas=document.createElement('canvas');var canvas2d=canvas.getContext('2d');var gl2=canvas.getContext('webgl2',{failIfMajorPerformanceCaveat:true});var el=document.createElement('div');var animation=el.animate?el.animate([],{}):null;var sh=gl2.createShader(gl2.VERTEX_SHADER),pr=gl2.createProgram(),buf=gl2.createBuffer(),tex=gl2.createTexture(),fb=gl2.createFramebuffer();var objectState={before:[gl2.isShader(sh),gl2.isProgram(pr),gl2.isBuffer(buf),gl2.isTexture(tex),gl2.isFramebuffer(fb)],compile:false,link:false,deleted:false};gl2.shaderSource(sh,'void main(){}');gl2.compileShader(sh);objectState.compile=gl2.getShaderParameter(sh,gl2.COMPILE_STATUS);gl2.attachShader(pr,sh);gl2.linkProgram(pr);objectState.link=gl2.getProgramParameter(pr,gl2.LINK_STATUS);gl2.bindBuffer(gl2.ARRAY_BUFFER,buf);gl2.activeTexture(gl2.TEXTURE0+1);gl2.bindTexture(gl2.TEXTURE_2D,tex);gl2.bindFramebuffer(gl2.FRAMEBUFFER,fb);gl2.viewport(1,2,30,40);gl2.scissor(3,4,20,21);gl2.clearColor(.1,.2,.3,.4);gl2.enable(gl2.BLEND);gl2.blendFuncSeparate(gl2.SRC_ALPHA,gl2.ONE_MINUS_SRC_ALPHA,gl2.ONE,gl2.ZERO);gl2.depthFunc(gl2.LESS);gl2.stencilFunc(gl2.ALWAYS,2,255);gl2.stencilMask(127);var loc=gl2.getUniformLocation(pr,'u_value');gl2.uniform4f(loc,1,2,3,4);gl2.enableVertexAttribArray(2);gl2.vertexAttribPointer(2,3,gl2.FLOAT,false,12,4);var pixels=new Uint8Array(4);gl2.readPixels(0,0,1,1,gl2.RGBA,gl2.UNSIGNED_BYTE,pixels);objectState.state=[gl2.getParameter(gl2.ARRAY_BUFFER_BINDING)===buf,gl2.getParameter(gl2.ACTIVE_TEXTURE),gl2.getParameter(gl2.TEXTURE_BINDING_2D)===tex,gl2.getParameter(gl2.FRAMEBUFFER_BINDING)===fb,Array.from(gl2.getParameter(gl2.VIEWPORT)),Array.from(gl2.getParameter(gl2.SCISSOR_BOX)),Array.from(gl2.getParameter(gl2.COLOR_CLEAR_VALUE)),gl2.isEnabled(gl2.BLEND),gl2.getParameter(gl2.BLEND_SRC_RGB),gl2.getParameter(gl2.BLEND_DST_RGB),gl2.getParameter(gl2.DEPTH_FUNC),gl2.getParameter(gl2.STENCIL_REF),gl2.getParameter(gl2.STENCIL_WRITEMASK),gl2.getUniform(pr,loc),gl2.getVertexAttrib(2,gl2.VERTEX_ATTRIB_ARRAY_ENABLED),gl2.getVertexAttrib(2,gl2.VERTEX_ATTRIB_ARRAY_SIZE),gl2.getVertexAttrib(2,gl2.VERTEX_ATTRIB_ARRAY_BUFFER_BINDING)===buf,gl2.getVertexAttribOffset(2,gl2.VERTEX_ATTRIB_ARRAY_POINTER),gl2.__browsaiNonRendering,gl2.__browsaiPixelOutput,gl2.getError()];gl2.deleteShader(sh);gl2.deleteProgram(pr);gl2.deleteBuffer(buf);gl2.deleteTexture(tex);gl2.deleteFramebuffer(fb);objectState.deleted=[gl2.isShader(sh),gl2.isProgram(pr),gl2.isBuffer(buf),gl2.isTexture(tex),gl2.isFramebuffer(fb)];var brand={instanceof:!!gl2&&gl2 instanceof WebGL2RenderingContext,toString:!!gl2&&Object.prototype.toString.call(gl2),prototypeCall:false};try{brand.prototypeCall=!!gl2&&!!WebGL2RenderingContext.prototype.getParameter.call(gl2,gl2.VERSION)}catch(_){}return {constructor:typeof OffscreenCanvas,width:c.width,height:c.height,context:!!ctx,htmlCanvas2d:!!canvas2d,htmlCanvas2dFillRect:!!canvas2d&&typeof canvas2d.fillRect==='function',convertToBlob:typeof c.convertToBlob,transferToImageBitmap:typeof c.transferToImageBitmap,fontFace:typeof FontFace,fonts:!!document.fonts,language:navigator.language,intl:new Intl.DateTimeFormat().resolvedOptions().locale,webgl2:gl2&&{version:String(gl2.getParameter(gl2.VERSION)),maxTexture:gl2.getParameter(gl2.MAX_TEXTURE_SIZE),maxVertexUniforms:gl2.getParameter(gl2.MAX_VERTEX_UNIFORM_VECTORS),maxFragmentUniforms:gl2.getParameter(gl2.MAX_FRAGMENT_UNIFORM_VECTORS),maxVarying:gl2.getParameter(gl2.MAX_VARYING_VECTORS),shaderPrecision:gl2.getShaderPrecisionFormat(gl2.VERTEX_SHADER,gl2.HIGH_FLOAT).precision,extensions:gl2.getSupportedExtensions(),attrs:gl2.getContextAttributes()},objectState:objectState,brand:brand,animation:!!animation&&[typeof animation.play,typeof animation.pause,typeof animation.finished]};})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "offscreen-canvas-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        window_apis.value,
        serde_json::json!({
            "constructor": "function",
            "width": 8.0,
            "height": 4.0,
            "context": true,
            "htmlCanvas2d": true,
            "htmlCanvas2dFillRect": true,
            "convertToBlob": "function",
            "transferToImageBitmap": "function",
            "fontFace": "function",
            "fonts": true,
            "language": "en-US",
            "intl": "en-US",
            "webgl2": {
                "version": "WebGL 2.0 BrowsAI Fake",
                "maxTexture": 4096.0,
                "maxVertexUniforms": 256.0,
                "maxFragmentUniforms": 256.0,
                "maxVarying": 8.0,
                "shaderPrecision": 23.0,
                "extensions": [
                    "OES_element_index_uint", "OES_standard_derivatives", "OES_vertex_array_object",
                    "ANGLE_instanced_arrays", "WEBGL_lose_context", "WEBGL_debug_renderer_info",
                    "WEBGL_debug_shaders", "WEBGL_depth_texture", "OES_texture_float",
                    "OES_texture_half_float", "OES_texture_float_linear", "OES_texture_half_float_linear",
                    "EXT_texture_filter_anisotropic", "EXT_color_buffer_float", "EXT_color_buffer_half_float"
                ],
                "attrs": {
                    "alpha": true,
                    "antialias": false,
                    "depth": true,
                    "desynchronized": false,
                    "preserveDrawingBuffer": false,
                    "stencil": false,
                    "failIfMajorPerformanceCaveat": true,
                    "powerPreference": "default"
                }
            },
            "objectState": {
                "before": [true, true, true, true, true],
                "compile": true,
                "link": true,
                "state": [true, 33985.0, true, true, [1.0, 2.0, 30.0, 40.0], [3.0, 4.0, 20.0, 21.0], [0.10000000149011612, 0.20000000298023224, 0.30000001192092896, 0.4000000059604645], true, 770.0, 771.0, 513.0, 2.0, 127.0, [1.0, 2.0, 3.0, 4.0], true, 3.0, true, 4.0, true, "unavailable", 1282.0],
                "deleted": [false, false, false, false, false]
            },
            "brand": {
                "instanceof": true,
                "toString": "[object WebGL2RenderingContext]",
                "prototypeCall": true,
            },
            "animation": ["function", "function", "object"],
        })
    );

    let svg_constructor = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({svgScriptElement:typeof SVGScriptElement,svgElement:typeof SVGElement})"
                        .into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "svg-script-element-constructor-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        svg_constructor.value,
        serde_json::json!({"svgScriptElement": "function", "svgElement": "function"})
    );

    let uniform_contract = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var gl=document.createElement('canvas').getContext('webgl2');var shader=gl.createShader(gl.VERTEX_SHADER),program=gl.createProgram();gl.shaderSource(shader,'attribute vec3 a_position; uniform vec4 u_color; void main(){}');gl.compileShader(shader);gl.attachShader(program,shader);gl.linkProgram(program);var active=gl.getActiveUniform(program,0),attribute=gl.getActiveAttrib(program,0);return {count:gl.getProgramParameter(program,gl.ACTIVE_UNIFORMS),name:active&&active.name,size:active&&active.size,type:active&&active.type,attributeCount:gl.getProgramParameter(program,gl.ACTIVE_ATTRIBUTES),attributeName:attribute&&attribute.name,attributeSize:attribute&&attribute.size,attributeType:attribute&&attribute.type,attributeLocation:gl.getAttribLocation(program,'a_position')};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "webgl-active-uniform-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        uniform_contract.value,
        serde_json::json!({"count": 1.0, "name": "u_color", "size": 1.0, "type": 35666.0, "attributeCount": 1.0, "attributeName": "a_position", "attributeSize": 1.0, "attributeType": 35665.0, "attributeLocation": 0.0})
    );

    let identity = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("({uaChrome:/Chrome\\//.test(navigator.userAgent),vendor:navigator.vendor,platform:navigator.platform,webdriver:!!navigator.webdriver,chrome:!!window.chrome,userAgentData:!!navigator.userAgentData,languageConsistent:!!navigator.languages&&navigator.languages[0]===navigator.language,intlConsistent:new Intl.DateTimeFormat().resolvedOptions().locale===navigator.language,timezone:!!new Intl.DateTimeFormat().resolvedOptions().timeZone,viewport:innerWidth>0&&innerHeight>0,dpr:devicePixelRatio>0,screen:screen.width>0&&screen.height>0,fontFaceSet:!!document.fonts,fontAdd:!!document.fonts&&typeof document.fonts.add==='function',fontDelete:!!document.fonts&&typeof document.fonts.delete==='function',fontClear:!!document.fonts&&typeof document.fonts.clear==='function',fontCheck:!!document.fonts&&typeof document.fonts.check==='function',fontReady:!!document.fonts&&!!document.fonts.ready,cacheStorage:!!window.caches,cacheOpen:!!window.caches&&typeof window.caches.open==='function',cacheMatch:!!window.caches&&typeof window.caches.match==='function',cacheConstructor:typeof Cache==='function',indexedDbIndexCursor:typeof IDBIndex==='undefined'||typeof IDBIndex.prototype.openCursor==='function',indexedDbCursorContinue:typeof IDBCursor==='undefined'||typeof IDBCursor.prototype.continue==='function',indexedDbCursorAdvance:typeof IDBCursor==='undefined'||typeof IDBCursor.prototype.advance==='function',eventMethodWritable:typeof Event==='undefined'||!!Object.getOwnPropertyDescriptor(Event.prototype,'stopImmediatePropagation')&&Object.getOwnPropertyDescriptor(Event.prototype,'stopImmediatePropagation').writable===true,svgAnimationControls:typeof SVGSVGElement==='undefined'||typeof SVGSVGElement.prototype.pauseAnimations==='function'&&typeof SVGSVGElement.prototype.unpauseAnimations==='function',audioCompressor:typeof AudioContext==='undefined'||typeof AudioContext.prototype.createDynamicsCompressor==='function',keyframeEffect:typeof KeyframeEffect==='function',canvasRoundRect:typeof CanvasRenderingContext2D==='undefined'||typeof CanvasRenderingContext2D.prototype.roundRect==='function',visualViewport:!!window.visualViewport&&window.visualViewport.scale===1,notification:typeof Notification==='function'&&typeof Notification.requestPermission==='function',mediaDevices:!!navigator.mediaDevices,enumerateDevices:!!navigator.mediaDevices&&typeof navigator.mediaDevices.enumerateDevices==='function',windowCookieStore:!!window.cookieStore&&typeof window.cookieStore.get==='function',cookieStore:!!navigator.cookieStore,cookieGet:!!navigator.cookieStore&&typeof navigator.cookieStore.get==='function',cookieGetAll:!!navigator.cookieStore&&typeof navigator.cookieStore.getAll==='function',cookieSet:!!navigator.cookieStore&&typeof navigator.cookieStore.set==='function',cookieDelete:!!navigator.cookieStore&&typeof navigator.cookieStore.delete==='function'})".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "browser-identity-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        identity.value,
        serde_json::json!({
            "uaChrome": true,
            "vendor": "Google Inc.",
            "platform": "Linux x86_64",
            "webdriver": false,
            "chrome": true,
            "userAgentData": true,
            "languageConsistent": true,
            "intlConsistent": true,
            "timezone": true,
            "viewport": true,
            "dpr": true,
            "screen": true,
            "cacheStorage": true,
            "cacheOpen": true,
            "cacheMatch": true,
            "cacheConstructor": true,
            "consoleTable": true,
            "indexedDbIndexCursor": true,
            "indexedDbCursorContinue": true,
            "indexedDbCursorAdvance": true,
            "eventMethodWritable": true,
            "svgAnimationControls": true,
            "audioCompressor": true,
            "keyframeEffect": true,
            "canvasRoundRect": true,
            "visualViewport": true,
            "notification": true,
            "mediaDevices": true,
            "enumerateDevices": true,
            "windowCookieStore": true,
            "fontFaceSet": true,
            "fontAdd": true,
            "fontDelete": true,
            "fontClear": true,
            "fontCheck": true,
            "fontReady": true,
            "cookieStore": true,
            "cookieGet": true,
            "cookieGetAll": true,
            "cookieSet": true,
            "cookieDelete": true,
        })
    );

    let cache_write = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "window.caches.open('browsai-growing-cache').then(function(cache){var writes=[];for(var i=0;i<64;i++)writes.push(cache.put('/browsai-cache/'+i,new Response('value-'+i)));return Promise.all(writes).then(function(){return cache.match('/browsai-cache/63');}).then(function(response){return response&&response.text?response.text():'';}).then(function(value){window.__browsaiCacheProbe=value;});});({started:true})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "cache-storage-growing-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(cache_write.value, serde_json::json!({"started": true}));
    let cache_probe = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("({value:window.__browsaiCacheProbe||null,open:typeof caches.open,match:typeof caches.match})".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "cache-storage-growing-regression-readback".into(),
            },
        )
        .unwrap();
    assert_eq!(
        cache_probe.value,
        serde_json::json!({"value": "value-63", "open": "function", "match": "function"})
    );

    let locks = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "navigator.locks.request('browsai-test',function(lock){return {name:lock.name,mode:lock.mode};}).then(function(value){window.__browsaiLockResult=value;});({present:!!navigator.locks,request:typeof navigator.locks.request==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "web-locks-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        locks.value,
        serde_json::json!({"present": true, "request": true})
    );

    let cookie_store_alias = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({same:window.cookieStore===navigator.cookieStore,global:typeof cookieStore!=='undefined',get:typeof cookieStore.get==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "cookie-store-alias-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        cookie_store_alias.value,
        serde_json::json!({"same": true, "global": true, "get": true})
    );

    let restricted_capabilities = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({clipboard:!!navigator.clipboard,clipboardWrite:!!navigator.clipboard&&typeof navigator.clipboard.writeText==='function',geolocation:!!navigator.geolocation,geolocationQuery:!!navigator.geolocation&&typeof navigator.geolocation.getCurrentPosition==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "restricted-capability-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        restricted_capabilities.value,
        serde_json::json!({
            "clipboard": true,
            "clipboardWrite": true,
            "geolocation": true,
            "geolocationQuery": true,
        })
    );

    let chrome_runtime = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({chrome:!!window.chrome,runtime:!!window.chrome&&!!window.chrome.runtime,sendMessage:!!window.chrome&&!!window.chrome.runtime&&typeof window.chrome.runtime.sendMessage==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "chrome-runtime-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        chrome_runtime.value,
        serde_json::json!({"chrome": true, "runtime": true, "sendMessage": true})
    );

    let browser_identity_extensions = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({appVersionChrome:/Chrome\\//.test(navigator.appVersion),share:typeof navigator.share==='function',canShare:typeof navigator.canShare==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "browser-identity-extensions-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        browser_identity_extensions.value,
        serde_json::json!({"appVersionChrome": true, "share": true, "canShare": true})
    );

    let svg_document_api = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({object:typeof HTMLObjectElement==='undefined'||typeof HTMLObjectElement.prototype.getSVGDocument==='function',safe:typeof HTMLObjectElement==='undefined'||document.createElement('object').getSVGDocument()===null})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "svg-document-api-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        svg_document_api.value,
        serde_json::json!({"object": true, "safe": true})
    );

    let indexed_db_api = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({indexCount:typeof IDBIndex==='undefined'||typeof IDBIndex.prototype.count==='function',indexCursor:typeof IDBIndex==='undefined'||typeof IDBIndex.prototype.openCursor==='function',cursorContinue:typeof IDBCursor==='undefined'||typeof IDBCursor.prototype.continue==='function',cursorValueContinue:typeof IDBCursorWithValue==='undefined'||typeof IDBCursorWithValue.prototype.continue==='function',svgA:typeof SVGAElement==='undefined'||typeof SVGAElement==='function'})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "indexeddb-index-count-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        indexed_db_api.value,
        serde_json::json!({"indexCount": true, "indexCursor": true, "cursorContinue": true, "cursorValueContinue": true, "svgA": true})
    );

    let cookie_store_constructor = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({constructor:typeof CookieStore==='function',prototype:typeof CookieStore==='undefined'||typeof CookieStore.prototype.get==='function',navigator:!!navigator.cookieStore})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "cookie-store-constructor-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        cookie_store_constructor.value,
        serde_json::json!({"constructor": true, "prototype": true, "navigator": true})
    );

    let intersection_observer = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({observer:typeof IntersectionObserver,entry:typeof IntersectionObserverEntry})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "intersection-observer-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        intersection_observer.value,
        serde_json::json!({"observer": "function", "entry": "function"})
    );

    let performance_detail_measure = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var entry=performance.measure('browsai-detail-only',{detail:{source:'regression'}});return {ok:!!entry,name:entry.name,entryType:entry.entryType};}catch(error){return {ok:false,error:String(error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "performance-measure-detail-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        performance_detail_measure.value,
        serde_json::json!({
            "ok": true,
            "name": "browsai-detail-only",
            "entryType": "measure",
        })
    );

    let resource_timing_fetch_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var entries=performance.getEntriesByType('navigation'),entry=entries&&entries[0];return {supported:!!entry||typeof PerformanceResourceTiming==='function',value:entry?entry.fetchStart:null};}catch(error){return {supported:false,error:String(error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "performance-resource-fetch-start-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        resource_timing_fetch_start.value.get("supported"),
        Some(&serde_json::Value::Bool(true))
    );

    let legacy_performance_timing = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var timing=performance.timing;return {supported:!!timing,fetchStart:typeof timing.fetchStart==='number',domainLookupStart:typeof timing.domainLookupStart==='number',connectStart:typeof timing.connectStart==='number',responseStart:typeof timing.responseStart==='number'};}catch(error){return {supported:false,error:String(error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "legacy-performance-timing-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        legacy_performance_timing.value,
        serde_json::json!({
            "supported": true,
            "fetchStart": true,
            "domainLookupStart": true,
            "connectStart": true,
            "responseStart": true,
        })
    );

    let console_proxy_diagnostic = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var value=new Proxy({answer:42},{ownKeys:function(){throw new Error('diagnostic enumeration failed');}});console.log(value);return {ok:true};}catch(error){return {ok:false,error:String(error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "console-reentrant-printer-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        console_proxy_diagnostic.value,
        serde_json::json!({"ok": true})
    );

    let feature_policy = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({present:!!document.featurePolicy,allows:document.featurePolicy&&document.featurePolicy.allowsFeature('camera'),features:document.featurePolicy&&document.featurePolicy.features()})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "feature-policy-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        feature_policy.value,
        serde_json::json!({"present": true, "allows": false, "features": []})
    );

    let font_lifecycle = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var face=new FontFace('BrowsAI-Test','local(Arial)');var set=document.fonts;var returned=set.add(face)===set;var present=set.has(face);var removed=set.delete(face);return {returned:returned,present:present,removed:removed,afterDelete:set.has(face)};})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "document-fonts-lifecycle-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        font_lifecycle.value,
        serde_json::json!({
            "returned": true,
            "present": true,
            "removed": true,
            "afterDelete": false,
        })
    );

    let svg_regression = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('width','320');svg.setAttribute('height','180');var width=svg.width,height=svg.height,matrix=svg.createSVGMatrix(),translated=matrix.translate(4,5);return {width:!!width,widthBase:!!width&&width.baseVal.value,height:!!height,heightBase:!!height&&height.baseVal.value,matrix:!!matrix,matrixIdentity:matrix.a===1&&matrix.d===1&&matrix.e===0&&matrix.f===0,translated:translated.e===4&&translated.f===5};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "svg-animated-length-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        svg_regression.value,
        serde_json::json!({"width": true, "widthBase": 320.0, "height": true, "heightBase": 180.0, "matrix": true, "matrixIdentity": true, "translated": true})
    );

    let animation_regression = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var animation=new Animation();animation.play();animation.pause();animation.finish();return {constructor:typeof Animation,play:typeof animation.play,pause:typeof animation.pause,finished:!!animation.finished,playState:animation.playState};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "animation-constructor-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        animation_regression.value,
        serde_json::json!({"constructor": "function", "play": "function", "pause": "function", "finished": true, "playState": "finished"})
    );

    let locale_regression = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var intl=new Intl.DateTimeFormat().resolvedOptions();var canonical=Intl.getCanonicalLocales(navigator.language);return {language:navigator.language,languages:navigator.languages,canonical:canonical[0],intl:intl.locale,timezone:intl.timeZone||''};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "locale-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        locale_regression.value["language"],
        serde_json::json!("en-US")
    );
    assert_eq!(
        locale_regression.value["languages"],
        serde_json::json!(["en-US"])
    );
    assert_eq!(
        locale_regression.value["canonical"],
        serde_json::json!("en-US")
    );
    assert_eq!(locale_regression.value["intl"], serde_json::json!("en-US"));
    assert_eq!(
        locale_regression.value["timezone"],
        serde_json::json!("UTC")
    );

    let window_realm = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("({OffscreenCanvas:typeof OffscreenCanvas==='function',fetch:typeof fetch==='function',WebSocket:typeof WebSocket==='function',crypto:typeof crypto==='object',TextEncoder:typeof TextEncoder==='function',URL:typeof URL==='function'})".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "window-api-realm-matrix".into(),
            },
        )
        .unwrap();
    assert_eq!(
        window_realm.value,
        serde_json::json!({
            "OffscreenCanvas": true,
            "fetch": true,
            "WebSocket": true,
            "crypto": true,
            "TextEncoder": true,
            "URL": true,
        })
    );

    let auxiliary_realms = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("({sharedWorker:typeof SharedWorker==='function',serviceWorker:!!navigator.serviceWorker,serviceWorkerRegister:!!navigator.serviceWorker&&typeof navigator.serviceWorker.register==='function',serviceWorkerStartMessages:!!navigator.serviceWorker&&typeof navigator.serviceWorker.startMessages==='function',permissions:!!navigator.permissions,permissionsQuery:!!navigator.permissions&&typeof navigator.permissions.query==='function',serviceWorkerMode:window.__browsaiServiceWorkerMode||'unavailable'})".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "auxiliary-worker-realm-assessment".into(),
            },
        )
        .unwrap();
    assert_eq!(
        auxiliary_realms.value,
        serde_json::json!({
            "sharedWorker": true,
            "serviceWorker": true,
            "serviceWorkerRegister": true,
            "serviceWorkerStartMessages": true,
            "permissions": true,
            "permissionsQuery": true,
            "serviceWorkerMode": "native",
        })
    );

    let shared_worker_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__sharedWorkerMatrix='pending';(function(){var blob=new Blob([\"self.addEventListener('connect',function(event){var port=event.ports[0];try{var c=new OffscreenCanvas(2,2);port.postMessage(JSON.stringify({OffscreenCanvas:typeof OffscreenCanvas==='function'&&c.width===2&&c.height===2,fetch:typeof fetch==='function',WebSocket:typeof WebSocket==='function',crypto:typeof crypto==='object',TextEncoder:typeof TextEncoder==='function',URL:typeof URL==='function'}));setTimeout(function(){self.close()},500)}catch(e){port.postMessage('error:'+e);setTimeout(function(){self.close()},500)}});\"],{type:'text/javascript'});var worker=new SharedWorker(URL.createObjectURL(blob));window.__sharedWorkerProbe=worker;worker.onerror=function(){window.__sharedWorkerMatrix='shared-worker-error';};worker.port.onmessage=function(event){window.__sharedWorkerMatrix=event.data;worker.port.close();};worker.port.start();})();null".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "shared-worker-api-realm-matrix".into(),
            },
        )
        .unwrap();
    assert_eq!(shared_worker_start.value, serde_json::Value::Null);
    let mut shared_worker_matrix = serde_json::Value::Null;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        shared_worker_matrix = engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("window.__sharedWorkerMatrix".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "shared-worker-api-realm-matrix-result".into(),
                },
            )
            .unwrap()
            .value;
        if shared_worker_matrix != serde_json::json!("pending") {
            break;
        }
    }
    if shared_worker_matrix == serde_json::json!("pending") {
        engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("if(window.__sharedWorkerProbe){try{window.__sharedWorkerProbe.port.close()}catch(_){}}window.__sharedWorkerMatrix='shared-worker-timeout';null".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "shared-worker-api-realm-matrix-cleanup".into(),
                },
            )
            .unwrap();
        shared_worker_matrix = serde_json::json!("shared-worker-timeout");
    }
    let shared_worker_matrix = match shared_worker_matrix.as_str() {
        Some(value) if value.starts_with('{') => serde_json::from_str::<serde_json::Value>(value)
            .unwrap_or_else(|_| serde_json::json!({"error": value})),
        Some(value) => serde_json::json!(value),
        None => serde_json::json!({"error": shared_worker_matrix}),
    };
    assert_eq!(
        shared_worker_matrix,
        serde_json::json!({
            "OffscreenCanvas": true,
            "fetch": true,
            "WebSocket": true,
            "crypto": true,
            "TextEncoder": true,
            "URL": true,
        })
    );

    let texture_framebuffer_probe = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var c=document.createElement('canvas'),g=c.getContext('webgl2');var t=g.createTexture(),f=g.createFramebuffer(),r=g.createRenderbuffer();g.bindTexture(g.TEXTURE_2D,t);g.texImage2D(g.TEXTURE_2D,0,g.RGBA,1,1,0,g.RGBA,g.UNSIGNED_BYTE,null);g.bindFramebuffer(g.FRAMEBUFFER,f);g.bindRenderbuffer(g.RENDERBUFFER,r);g.renderbufferStorage(g.RENDERBUFFER,g.DEPTH_COMPONENT16,1,1);g.framebufferRenderbuffer(g.FRAMEBUFFER,g.DEPTH_ATTACHMENT,g.RENDERBUFFER,r);return {texture:!!t&&g.isTexture(t),framebuffer:!!f&&g.isFramebuffer(f),renderbuffer:!!r&&g.isRenderbuffer(r),status:g.checkFramebufferStatus(g.FRAMEBUFFER)};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "webgl-texture-framebuffer-probe".into(),
            },
        )
        .unwrap();
    assert_eq!(
        texture_framebuffer_probe.value,
        serde_json::json!({
            "texture": true,
            "framebuffer": true,
            "renderbuffer": true,
            "status": 36053.0,
        })
    );

    let webgl_state_probe = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var c=document.createElement('canvas'),g=c.getContext('webgl2');g.viewport(2,3,40,50);g.scissor(4,5,20,21);g.depthMask(false);g.colorMask(true,false,true,false);g.blendEquationSeparate(g.FUNC_ADD,g.FUNC_SUBTRACT);g.polygonOffset(1.5,2.5);g.pixelStorei(g.UNPACK_ALIGNMENT,1);return {constants:typeof g.VIEWPORT==='number'&&typeof g.SCISSOR_BOX==='number',methods:typeof g.depthMask==='function'&&typeof g.colorMask==='function'&&typeof g.blendEquationSeparate==='function'&&typeof g.polygonOffset==='function'&&typeof g.pixelStorei==='function',viewport:Array.from(g.getParameter(g.VIEWPORT)),scissor:Array.from(g.getParameter(g.SCISSOR_BOX)),depth:g.getParameter(g.DEPTH_WRITEMASK),color:Array.from(g.getParameter(g.COLOR_WRITEMASK)),unpack:g.getParameter(g.UNPACK_ALIGNMENT)};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "webgl-state-contract-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        webgl_state_probe.value,
        serde_json::json!({
            "constants": true,
            "methods": true,
            "viewport": [2.0, 3.0, 40.0, 50.0],
            "scissor": [4.0, 5.0, 20.0, 21.0],
            "depth": false,
            "color": [true, false, true, false],
            "unpack": 1.0,
        })
    );

    let webgl_descriptor = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var d=Object.getOwnPropertyDescriptor(WebGL2RenderingContext.prototype,'getParameter');return d&&{configurable:!!d.configurable,enumerable:!!d.enumerable,writable:!!d.writable};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "webgl-native-descriptor-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        webgl_descriptor.value,
        serde_json::json!({
            "configurable": true,
            "enumerable": false,
            "writable": true,
        })
    );

    let indexed_db_bulk_methods = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("({indexGetAll:typeof IDBIndex==='undefined'||typeof IDBIndex.prototype.getAll==='function',indexGetAllKeys:typeof IDBIndex==='undefined'||typeof IDBIndex.prototype.getAllKeys==='function'})".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "indexeddb-index-bulk-methods-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        indexed_db_bulk_methods.value,
        serde_json::json!({
            "indexGetAll": true,
            "indexGetAllKeys": true,
        })
    );

    let worker_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "window.__offscreenWorkerResult='pending';(function(){var blob=new Blob([\"try{var c=new OffscreenCanvas(2,2);self.postMessage(typeof OffscreenCanvas+'|'+!!c.getContext('2d')+'|'+c.width+'|'+c.height)}catch(e){self.postMessage('error:'+e)}\"],{type:'text/javascript'});var worker=new Worker(URL.createObjectURL(blob));worker.onmessage=function(event){window.__offscreenWorkerResult=event.data;worker.terminate();};worker.onerror=function(event){window.__offscreenWorkerResult='worker-error';worker.terminate();};})();null".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "offscreen-canvas-worker-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(worker_start.value, serde_json::Value::Null);

    let mut worker_result = serde_json::Value::Null;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        worker_result = engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("window.__offscreenWorkerResult".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "offscreen-canvas-worker-result-regression".into(),
                },
            )
            .unwrap()
            .value;
        if worker_result == serde_json::json!("function|true|2|2") {
            break;
        }
    }
    assert_eq!(worker_result, serde_json::json!("function|true|2|2"));

    // Replay the corpus-002 failure shape: a third-party blob worker creates
    // an OffscreenCanvas during initialization. The old runtime raised
    // "OffscreenCanvas is not defined" at this point; the replay requires the
    // same worker-side construction and 2D probe to complete instead.
    let failure_replay_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "window.__offscreenFailureReplay='pending';(function(){var source=\"try{var canvas=new OffscreenCanvas(2,2);var context=canvas.getContext('2d');self.postMessage(JSON.stringify({constructor:typeof OffscreenCanvas,context:!!context,width:canvas.width,height:canvas.height,failureSignature:false}))}catch(error){self.postMessage(JSON.stringify({failureSignature:String(error).indexOf('OffscreenCanvas')>=0,error:String(error)}))}\";var worker=new Worker(URL.createObjectURL(new Blob([source],{type:'text/javascript'})));worker.onmessage=function(event){window.__offscreenFailureReplay=event.data;worker.terminate();};worker.onerror=function(){window.__offscreenFailureReplay='worker-error';worker.terminate();};})();null".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "corpus-002-offscreen-failure-replay".into(),
            },
        )
        .unwrap();
    assert_eq!(failure_replay_start.value, serde_json::Value::Null);
    let mut failure_replay = serde_json::Value::Null;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        failure_replay = engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("window.__offscreenFailureReplay".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "corpus-002-offscreen-failure-replay-result".into(),
                },
            )
            .unwrap()
            .value;
        if failure_replay != serde_json::json!("pending") {
            break;
        }
    }
    assert_eq!(
        failure_replay,
        serde_json::json!(
            r#"{"constructor":"function","context":true,"width":2,"height":2,"failureSignature":false}"#
        )
    );

    let worker_matrix_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__workerRealmMatrix='pending';(function(){var blob=new Blob([\"try{var c=new OffscreenCanvas(2,2);self.postMessage(JSON.stringify({OffscreenCanvas:typeof OffscreenCanvas==='function'&&c.width===2&&c.height===2,fetch:typeof fetch==='function',WebSocket:typeof WebSocket==='function',crypto:typeof crypto==='object',TextEncoder:typeof TextEncoder==='function',URL:typeof URL==='function'}))}catch(e){self.postMessage('error:'+e)}\"],{type:'text/javascript'});var worker=new Worker(URL.createObjectURL(blob));worker.onmessage=function(event){window.__workerRealmMatrix=event.data;worker.terminate();};worker.onerror=function(){window.__workerRealmMatrix='worker-error';worker.terminate();};})();null".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "dedicated-worker-api-realm-matrix".into(),
            },
        )
        .unwrap();
    assert_eq!(worker_matrix_start.value, serde_json::Value::Null);
    let mut worker_matrix = serde_json::Value::Null;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        worker_matrix = engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("window.__workerRealmMatrix".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "dedicated-worker-api-realm-matrix-result".into(),
                },
            )
            .unwrap()
            .value;
        if worker_matrix != serde_json::json!("pending") {
            break;
        }
    }
    let worker_matrix = worker_matrix
        .as_str()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({"error": worker_matrix}));
    assert_eq!(
        worker_matrix,
        serde_json::json!({
            "OffscreenCanvas": true,
            "fetch": true,
            "WebSocket": true,
            "crypto": true,
            "TextEncoder": true,
            "URL": true,
        })
    );

    let module_worker_start = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "window.__moduleWorkerResult='pending';(function(){var blob=new Blob([\"try{var c=new OffscreenCanvas(2,2);self.postMessage(JSON.stringify({OffscreenCanvas:typeof OffscreenCanvas==='function'&&!!c.getContext('2d')&&c.width===2&&c.height===2,fetch:typeof fetch==='function',WebSocket:typeof WebSocket==='function',crypto:typeof crypto==='object',TextEncoder:typeof TextEncoder==='function',URL:typeof URL==='function'}))}catch(e){self.postMessage('error:'+e)}\"],{type:'text/javascript'});var worker=new Worker(URL.createObjectURL(blob),{type:'module'});worker.onmessage=function(event){window.__moduleWorkerResult=event.data;worker.terminate();};worker.onerror=function(){window.__moduleWorkerResult='worker-error';worker.terminate();};})();null".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "offscreen-canvas-module-worker-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(module_worker_start.value, serde_json::Value::Null);
    let mut module_worker_result = serde_json::Value::Null;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        module_worker_result = engine
            .evaluate_page_script_checked(
                page,
                PageEvaluationRequest {
                    source: ScriptSource("window.__moduleWorkerResult".into()),
                    origin: "https://example.test".into(),
                    capability_granted: true,
                    timeout_millis: 100,
                    max_timeout_millis: 1_000,
                    provenance_reference: "offscreen-canvas-module-worker-result-regression".into(),
                },
            )
            .unwrap()
            .value;
        if module_worker_result != serde_json::json!("pending") {
            break;
        }
    }
    let module_worker_result = module_worker_result
        .as_str()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({"error": module_worker_result}));
    assert_eq!(
        module_worker_result,
        serde_json::json!({
            "OffscreenCanvas": true,
            "fetch": true,
            "WebSocket": true,
            "crypto": true,
            "TextEncoder": true,
            "URL": true,
        })
    );

    let document_language = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({documentLang:document.documentElement?document.documentElement.lang:'',navigatorLang:navigator.language})".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "document-language-default-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        document_language,
        serde_json::json!({"documentLang": "", "navigatorLang": "en-US"})
    );

    let websocket_contract = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var ws=new WebSocket('ws://127.0.0.1:1/socket',['chat']);var duplicate=false;try{new WebSocket('ws://127.0.0.1:1/socket',['chat','CHAT'])}catch(e){duplicate=e.name==='SyntaxError'}var result={url:ws.url,protocol:ws.protocol,readyState:ws.readyState,connecting:WebSocket.CONNECTING,open:WebSocket.OPEN,closing:WebSocket.CLOSING,closed:WebSocket.CLOSED,binaryType:ws.binaryType,duplicateProtocolRejected:duplicate};try{ws.close()}catch(_){}return result;})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "websocket-contract-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        websocket_contract,
        serde_json::json!({
            "url": "ws://127.0.0.1:1/socket",
            "protocol": "",
            "readyState": 0.0,
            "connecting": 0.0,
            "open": 1.0,
            "closing": 2.0,
            "closed": 3.0,
            "binaryType": "blob",
            "duplicateProtocolRejected": true,
        })
    );
    let webgl2 = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("(function(){var gl=document.createElement('canvas').getContext('webgl2');gl.drawBuffers([gl.COLOR_ATTACHMENT0]);gl.readBuffer(gl.COLOR_ATTACHMENT0);gl.clearBufferfv(gl.COLOR,0,[0,0,0,1]);gl.texStorage2D(gl.TEXTURE_2D,1,gl.RGBA8,1,1);return {drawBuffers:typeof gl.drawBuffers==='function',readBuffer:typeof gl.readBuffer==='function',clearBuffer:typeof gl.clearBufferfv==='function',texStorage:typeof gl.texStorage2D==='function',maxDrawBuffers:gl.getParameter(gl.MAX_DRAW_BUFFERS)};})()".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "webgl2-core-compatibility".into(),
            },
        )
        .unwrap();
    assert_eq!(
        webgl2.value,
        serde_json::json!({
            "drawBuffers": true,
            "readBuffer": true,
            "clearBuffer": true,
            "texStorage": true,
            "maxDrawBuffers": 4.0,
        })
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exposes_svg_uri_and_vertex_array_compatibility() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/compat").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){window.__browsaiForceFakeWebGL=true;var svg=document.createElementNS('http://www.w3.org/2000/svg','svg'),a=document.createElementNS('http://www.w3.org/2000/svg','a');a.setAttribute('href','/target');svg.appendChild(a);var box=document.createElement('div');document.body.appendChild(box);var ctm=a.getScreenCTM(),inverse=ctm&&ctm.inverse?ctm.inverse():null,css=getComputedStyle(box),speech=typeof webkitSpeechRecognition==='function'?new webkitSpeechRecognition():null,canvas=document.createElement('canvas'),gl=canvas.getContext('webgl'),ext=gl&&gl.getExtension('OES_vertex_array_object'),vao=ext&&ext.createVertexArrayOES(),angle=gl&&gl.getExtension('ANGLE_instanced_arrays'),derivatives=gl&&gl.getExtension('OES_standard_derivatives'),anisotropy=gl&&gl.getExtension('EXT_texture_filter_anisotropic'),depth=gl&&gl.getExtension('WEBGL_depth_texture');if(ext)ext.bindVertexArrayOES(vao);if(angle)angle.vertexAttribDivisorANGLE(0,1);return {href:!!a.href&&a.href.baseVal==='/target',screenCtm:!!ctm&&ctm.a===1,screenCtmInverse:!!inverse&&inverse.a===1,grid:typeof css.gridTemplateColumns==='string',speech:!!speech&&typeof speech.addEventListener==='function'&&typeof speech.removeEventListener==='function'&&typeof speech.dispatchEvent==='function',create:!!ext&&typeof ext.createVertexArrayOES==='function',bind:!!ext&&typeof ext.bindVertexArrayOES==='function',is:!!ext&&ext.isVertexArrayOES(vao),binding:!!vao,instanced:!!angle&&typeof angle.drawArraysInstancedANGLE==='function'&&typeof angle.drawElementsInstancedANGLE==='function'&&typeof angle.vertexAttribDivisorANGLE==='function',divisorValue:gl&&gl.getVertexAttrib(0,gl.VERTEX_ATTRIB_ARRAY_DIVISOR),extensionConstants:!!derivatives&&derivatives.FRAGMENT_SHADER_DERIVATIVE_HINT_OES===0x8B8B&&!!anisotropy&&anisotropy.MAX_TEXTURE_MAX_ANISOTROPY_EXT===0x84FF&&gl.getParameter(anisotropy.MAX_TEXTURE_MAX_ANISOTROPY_EXT)===16&&!!depth&&depth.UNSIGNED_INT_24_8_WEBGL===0x84FA};})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "svg-webgl-compatibility".into(),
            },
        )
        .unwrap();
    assert_eq!(
        result.value,
        serde_json::json!({
            "href": true,
            "screenCtm": true,
            "screenCtmInverse": true,
            "grid": true,
            "speech": true,
            "create": true,
            "bind": true,
            "is": true,
            "binding": true,
            "instanced": true,
            "divisorValue": 1.0,
            "extensionConstants": true,
        })
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_supports_simple_has_selectors() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/has").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var outer=document.createElement('div'),inner=document.createElement('span');outer.className='outer';inner.className='inner';outer.appendChild(inner);document.body.appendChild(outer);return {count:document.querySelectorAll('.outer:has(.inner)').length,first:document.querySelector('.outer:has(.inner)')===outer,autofillCount:document.querySelectorAll('input:-webkit-autofill').length,autofillFirst:document.querySelector('input:-webkit-autofill')===null};})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "css-has-selector-compatibility".into(),
            },
        )
        .unwrap();
    assert_eq!(
        result.value,
        serde_json::json!({
            "count": 1.0,
            "first": true,
            "autofillCount": 0.0,
            "autofillFirst": true,
        })
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exposes_inserted_css_rules() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/cssom").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){var style=document.createElement('style');document.head.appendChild(style);var sheet=style.sheet,index=sheet.insertRule('.probe { color: red; }',sheet.cssRules.length),rules=sheet.cssRules,again=sheet.cssRules;return {index:index,length:rules.length,item:!!rules.item(index),indexed:!!rules[index],againIndexed:!!again[index],same:rules===again,selector:rules.item(index)&&rules.item(index).selectorText};})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "cssom-insert-rule-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        result.value,
        serde_json::json!({
            "index": 0.0,
            "length": 1.0,
            "item": true,
            "indexed": true,
            "againIndexed": true,
            "same": true,
            "selector": ".probe",
        })
    );
    let shadow_result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var host=document.createElement('div'),root=host.attachShadow({mode:'open'});document.body.appendChild(host);root.innerHTML='<style>:host(:hover){cursor:pointer}</style>';var style=root.querySelector('style'),sheet=style&&style.sheet,index=sheet.insertRule(':host(:hover:not([notoggle])){}',sheet.cssRules.length),rules=sheet.cssRules,again=sheet.cssRules;return {shadow:true,sheet:!!sheet,index:index,length:rules.length,item:!!rules.item(index),indexed:!!rules[index],againIndexed:!!again[index],same:rules===again};}catch(error){return {shadow:true,error:String(error&&error.name||error),message:String(error&&error.message||error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "shadow-cssom-insert-rule-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        shadow_result.value,
        serde_json::json!({
            "shadow": true,
            "sheet": true,
            "index": 1.0,
            "length": 2.0,
            "item": true,
            "indexed": true,
            "againIndexed": true,
            "same": true,
        })
    );
    let custom_element_result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var C=class extends HTMLElement{constructor(){super();this.attachShadow({mode:'open'});this.shadowRoot.innerHTML='<style>:host(:hover){cursor:pointer}</style>';}connectedCallback(){var style=this.shadowRoot.querySelector('style'),sheet=style.sheet,rule=':host(:hover:not([notoggle])){}',index=sheet.cssRules.length;for(var i=0;i<sheet.cssRules.length;i++)if(sheet.cssRules[i].selectorText===rule.slice(0,-2)){index=i;break;}if(index===sheet.cssRules.length)sheet.insertRule(rule,index);var inserted=sheet.cssRules[index];inserted.style.setProperty('cursor','pointer');this.dataset.result=inserted&&inserted.style.getPropertyValue('cursor')||'missing';}};customElements.define('cssom-probe-element',C);var host=document.createElement('cssom-probe-element');document.body.appendChild(host);return {result:host.dataset.result,ruleCount:host.shadowRoot.querySelector('style').sheet.cssRules.length};}catch(error){return {error:String(error&&error.name||error),message:String(error&&error.message||error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "shadow-cssom-custom-element-regression".into(),
            },
        )
        .unwrap();
    assert_eq!(
        custom_element_result.value,
        serde_json::json!({
            "result": "pointer",
            "ruleCount": 2.0,
        })
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exposes_offline_audio_rendering() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/audio").unwrap())
        .unwrap();
    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){try{var Audio=window.OfflineAudioContext||window.webkitOfflineAudioContext;if(typeof Audio!=='function')return {available:false};var context=new Audio(1,5000,44100),osc=context.createOscillator(),compressor=context.createDynamicsCompressor();osc.type='triangle';osc.frequency.value=10000;compressor.threshold.value=-50;compressor.knee.value=40;compressor.ratio.value=12;compressor.attack.value=0;compressor.release.value=.25;osc.connect(compressor);compressor.connect(context.destination);osc.start(0);window.__offlineAudioProbe={state:'pending'};context.startRendering().then(function(buffer){window.__offlineAudioProbe={channels:buffer.numberOfChannels,length:buffer.length,sampleRate:buffer.sampleRate};},function(error){window.__offlineAudioProbe={error:String(error&&error.name||error),message:String(error&&error.message||error)};});return {available:true,startRendering:typeof context.startRendering==='function',state:context.state};}catch(error){return {available:true,error:String(error&&error.name||error),message:String(error&&error.message||error)};}})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 2_000,
                max_timeout_millis: 5_000,
                provenance_reference: "offline-audio-rendering-regression".into(),
            },
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let rendered = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__offlineAudioProbe".into()),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "offline-audio-rendering-result".into(),
            },
        )
        .unwrap();
    assert_eq!(
        result.value,
        serde_json::json!({
            "available": true,
            "startRendering": true,
            "state": "running",
        })
    );
    assert_eq!(
        rendered.value,
        serde_json::json!({
            "channels": 1.0,
            "length": 5000.0,
            "sampleRate": 44100.0,
        })
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exposes_service_worker_api_matrix() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    std::env::set_var("BROWSAI_DIAGNOSTIC_STATUS", "1");

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..size]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let (content_type, body) = if path.ends_with("/sw.js") {
                (
                    "application/javascript",
                    "self.addEventListener('message',event=>{try{var c=new OffscreenCanvas(320,180);var r={realm:'ServiceWorker',OffscreenCanvas:typeof OffscreenCanvas==='function'&&c.width===320&&c.height===180,context2d:!!c.getContext('2d'),fetch:typeof fetch==='function',WebSocket:typeof WebSocket==='function',crypto:typeof crypto==='object',TextEncoder:typeof TextEncoder==='function',URL:typeof URL==='function'};event.ports[0].postMessage(r)}catch(e){event.ports[0].postMessage('error:'+e)}});",
                )
            } else {
                (
                    "text/html",
                    "<!doctype html><output id=state>pending</output><script>navigator.serviceWorker.register('/sw.js').then(r=>{var w=r.active||r.waiting||r.installing;var c=new MessageChannel;c.port1.onmessage=e=>state.textContent=JSON.stringify(e.data);w.postMessage({type:'probe'},[c.port2])}).catch(e=>state.textContent='error:'+e)</script>",
                )
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    engine
        .navigate(
            page,
            Url::parse(&format!("{origin}/register.html")).unwrap(),
        )
        .unwrap();
    engine.pump_runtime(page, 2_000).unwrap();

    let result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("document.querySelector('#state').textContent".into()),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "service-worker-api-realm-matrix".into(),
            },
        )
        .unwrap()
        .value;
    let matrix = result
        .as_str()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({"error": result}));
    assert_eq!(
        matrix,
        serde_json::json!({
            "realm": "ServiceWorker",
            "OffscreenCanvas": true,
            "context2d": true,
            "fetch": true,
            "WebSocket": true,
            "crypto": true,
            "TextEncoder": true,
            "URL": true,
        })
    );
    let document_mime_type = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({mimeType:document.contentType,statusCode:document.__browsaiStatusCode})"
                        .into(),
                ),
                origin,
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "navigation-mime-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        document_mime_type["mimeType"],
        serde_json::json!("text/html")
    );
    assert_eq!(document_mime_type["statusCode"].as_f64(), Some(200.0));
    drop(engine);
    let _ = server.join();
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_records_redirect_fixture_without_hanging() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    std::env::set_var("BROWSAI_DIAGNOSTIC_STATUS", "1");

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..size]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let (status, headers, body) = match path {
                "/redirect" => ("302 Found", "Location: /final\r\n", String::new()),
                "/module.html" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><script type=module>import {value} from '/module.js';window.__staticModuleResult=value;import('/module.js').then(function(m){window.__dynamicModuleResult=m.value}).catch(function(e){window.__dynamicModuleResult='error:'+e})</script>".into(),
                ),
                "/module.js" => (
                    "200 OK",
                    "Content-Type: text/javascript\r\n",
                    "export const value='module-loaded'; export default value;".into(),
                ),
                "/module-tla.html" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><script type=module>import {value} from '/module-tla.js';window.__tlaResult=value</script>".into(),
                ),
                "/module-tla.js" => (
                    "200 OK",
                    "Content-Type: text/javascript\r\n",
                    "const value=await Promise.resolve('tla-loaded'); export {value};".into(),
                ),
                "/module-importmap.html" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><script type=importmap>{\"imports\":{\"fixture-module\":\"/module.js\"}}</script><script type=module>import {value} from 'fixture-module';window.__importMapResult=value</script>".into(),
                ),
                "/module-error.html" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><script type=module>import('/missing-module.js').then(function(){window.__moduleError='unexpected-success'}).catch(function(e){window.__moduleError=String(e)})</script>".into(),
                ),
                "/missing-module.js" => (
                    "404 Not Found",
                    "Content-Type: text/javascript\r\n",
                    "missing module".into(),
                ),
                "/forbidden" => (
                    "403 Forbidden",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><title>Forbidden</title><p>forbidden</p>".into(),
                ),
                "/download" => (
                    "200 OK",
                    "Content-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=fixture.bin\r\n",
                    "binary-fixture".into(),
                ),
                "/frame-only" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><iframe src='/embedded'></iframe>".into(),
                ),
                "/embedded" => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><p id='embedded-value'>frame-only-content</p>".into(),
                ),
                "/missing" => (
                    "404 Not Found",
                    "Content-Type: text/plain\r\n",
                    "missing".into(),
                ),
                _ => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><title>final</title><p>final</p>".into(),
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            if path == "/missing" {
                break;
            }
        }
    });

    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    engine
        .navigate(page, Url::parse(&format!("{origin}/redirect")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 2_000).unwrap();

    let request_history = engine.runtime_navigation_request_history(page).unwrap();
    assert!(request_history.iter().any(|url| url.path() == "/redirect"));
    let committed_history = engine.runtime_navigation_history(page).unwrap();
    assert!(committed_history.iter().any(|url| url.path() == "/final"));
    assert_eq!(engine.runtime_current_url(page).unwrap().path(), "/final");

    engine
        .navigate(page, Url::parse(&format!("{origin}/module.html")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let module_results = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({staticResult:window.__staticModuleResult||null,dynamicResult:window.__dynamicModuleResult||null})".into(),
                ),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "module-loading-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        module_results,
        serde_json::json!({
            "staticResult": "module-loaded",
            "dynamicResult": "module-loaded",
        })
    );

    engine
        .navigate(
            page,
            Url::parse(&format!("{origin}/module-tla.html")).unwrap(),
        )
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let tla_result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__tlaResult||null".into()),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "module-top-level-await-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(tla_result, serde_json::json!("tla-loaded"));

    engine
        .navigate(
            page,
            Url::parse(&format!("{origin}/module-importmap.html")).unwrap(),
        )
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let import_map_result = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__importMapResult||null".into()),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "module-import-map-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(import_map_result, serde_json::json!("module-loaded"));

    engine
        .navigate(
            page,
            Url::parse(&format!("{origin}/module-error.html")).unwrap(),
        )
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let module_error = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__moduleError||null".into()),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "module-error-propagation-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert!(module_error.as_str().is_some_and(|value| !value.is_empty()));

    engine
        .navigate(page, Url::parse(&format!("{origin}/forbidden")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let forbidden_metadata = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({mimeType:document.contentType,statusCode:document.__browsaiStatusCode})"
                        .into(),
                ),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "navigation-http-403-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        forbidden_metadata["mimeType"],
        serde_json::json!("text/html")
    );
    assert_eq!(forbidden_metadata["statusCode"].as_f64(), Some(403.0));

    engine
        .navigate(page, Url::parse(&format!("{origin}/download")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let download_metadata = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({mimeType:document.contentType,statusCode:document.__browsaiStatusCode})"
                        .into(),
                ),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "navigation-download-mime-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        download_metadata["mimeType"],
        serde_json::json!("application/octet-stream")
    );
    assert_eq!(download_metadata["statusCode"].as_f64(), Some(200.0));

    engine
        .navigate(page, Url::parse(&format!("{origin}/frame-only")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let frame_metadata = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({main:document.body.textContent.trim(),frame:(document.querySelector('iframe')&&document.querySelector('iframe').contentDocument&&document.querySelector('iframe').contentDocument.body.textContent.trim())||null})".into(),
                ),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "navigation-frame-only-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(
        frame_metadata,
        serde_json::json!({"main": "", "frame": "frame-only-content"})
    );

    engine
        .navigate(page, Url::parse(&format!("{origin}/missing")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 1_000).unwrap();
    let error_metadata = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "({mimeType:document.contentType,statusCode:document.__browsaiStatusCode})"
                        .into(),
                ),
                origin: origin.clone(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "navigation-http-error-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(error_metadata["mimeType"], serde_json::json!("text/plain"));
    assert_eq!(error_metadata["statusCode"].as_f64(), Some(404.0));

    drop(engine);
    let _ = server.join();
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_bounds_redirect_loop_without_hanging() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    std::env::set_var("BROWSAI_DIAGNOSTIC_STATUS", "1");

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let loop_b_location = format!("Location: http://127.0.0.1:{port}/loop-b\r\n");
    let loop_a_location = format!("Location: http://127.0.0.1:{port}/loop-a\r\n");
    let request_count = Arc::new(AtomicUsize::new(0));
    let server_request_count = Arc::clone(&request_count);
    let seen_paths = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_seen_paths = Arc::clone(&seen_paths);
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..size]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            server_seen_paths.lock().unwrap().push(path.to_string());
            let count = server_request_count.fetch_add(1, Ordering::Relaxed) + 1;
            let (status, headers, body) = match path {
                "/loop-a" => ("302 Found", loop_b_location.as_str(), String::new()),
                "/loop-b" => ("302 Found", loop_a_location.as_str(), String::new()),
                _ => (
                    "200 OK",
                    "Content-Type: text/html\r\n",
                    "<!doctype html><p>unexpected</p>".into(),
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            if count >= 12 {
                break;
            }
        }
    });

    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    engine
        .navigate(page, Url::parse(&format!("{origin}/loop-a")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 2_000).unwrap();

    let history = engine.runtime_navigation_request_history(page).unwrap();
    assert!(
        history.len() >= 2,
        "redirect loop was not requested: history={history:?}, server_requests={}",
        request_count.load(Ordering::Relaxed)
    );
    assert!(
        history.len() <= 256,
        "redirect loop exceeded bounded history"
    );
    assert!(history.iter().any(|url| url.path() == "/loop-a"));
    assert!(
        seen_paths
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/loop-b"),
        "redirect target was not requested by the server: {history:?}"
    );
    assert!(request_count.load(Ordering::Relaxed) <= 12);

    drop(engine);
    let _ = server.join();
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_exercises_websocket_handshake_frames_and_close() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let size = stream.read(&mut chunk).unwrap_or(0);
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..size]);
                if request.len() > 16 * 1024 {
                    break;
                }
            }
            let request_text = String::from_utf8_lossy(&request);
            let path = request_text
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            if path == "/ws.html" {
                let body = format!(
                    "<!doctype html><script>window.__wsState={{open:false,message:null,binary:null,protocol:null,closeCode:null,error:false}};var ws=new WebSocket('ws://127.0.0.1:{port}/socket',['chat','unused']);ws.binaryType='arraybuffer';ws.onopen=function(){{__wsState.open=true;__wsState.protocol=ws.protocol}};ws.onmessage=function(e){{if(typeof e.data==='string')__wsState.message=e.data;else __wsState.binary=Array.from(new Uint8Array(e.data))}};ws.onerror=function(){{__wsState.error=true}};ws.onclose=function(e){{__wsState.closeCode=e.code}}</script>"
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                continue;
            }

            let key = request_text
                .lines()
                .find_map(|line| line.strip_prefix("Sec-WebSocket-Key:"))
                .map(str::trim)
                .unwrap_or("");
            let mut digest = Sha1::new();
            digest.update(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11"));
            let accept = BASE64.encode(&digest.finalize());
            let response = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\nSec-WebSocket-Protocol: chat\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).unwrap();
            let message = b"hello";
            stream.write_all(&[0x81, message.len() as u8]).unwrap();
            stream.write_all(message).unwrap();
            stream.write_all(&[0x82, 0x03, 0x01, 0x02, 0x03]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
            stream.write_all(&[0x88, 0x02, 0x03, 0xE8]).unwrap();
            break;
        }
    });

    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    engine
        .navigate(page, Url::parse(&format!("{origin}/ws.html")).unwrap())
        .unwrap();
    engine.pump_runtime(page, 2_000).unwrap();
    let state = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource("window.__wsState".into()),
                origin,
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "websocket-handshake-frame-close-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(state["open"], serde_json::json!(true));
    assert_eq!(state["protocol"], serde_json::json!("chat"));
    assert_eq!(state["message"], serde_json::json!("hello"));
    assert_eq!(state["binary"], serde_json::json!([1.0, 2.0, 3.0]));
    assert_eq!(state["closeCode"].as_f64(), Some(1000.0));
    assert_eq!(state["error"], serde_json::json!(false));

    drop(engine);
    let _ = server.join();
}

#[cfg(feature = "servo-runtime")]
#[test]
#[ignore = "Servo's process-global options allow one real runtime per test process; run this fixture separately"]
fn selected_engine_runtime_reports_truncated_large_agent_tree_projection() {
    let _runtime_test_lock = REAL_RUNTIME_TEST_LOCK.lock().unwrap();
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            use_real_browser_runtime: true,
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    engine
        .navigate(page, Url::parse("https://example.test/large-dom").unwrap())
        .unwrap();
    let count = engine
        .evaluate_page_script_checked(
            page,
            PageEvaluationRequest {
                source: ScriptSource(
                    "(function(){document.body.innerHTML='';for(var i=0;i<6001;i++){var e=document.createElement('div');e.textContent='node';document.body.appendChild(e);}return document.body.childElementCount;})()".into(),
                ),
                origin: "https://example.test".into(),
                capability_granted: true,
                timeout_millis: 100,
                max_timeout_millis: 1_000,
                provenance_reference: "large-dom-projection-regression".into(),
            },
        )
        .unwrap()
        .value;
    assert_eq!(count, serde_json::json!(6001.0));
    let snapshot = engine.snapshot(page).unwrap();
    assert!(snapshot.tree.truncated);
    assert!(snapshot.tree.nodes.len() <= 5_002);
}

#[cfg(feature = "servo-runtime")]
#[test]
fn network_idle_interceptor_script_covers_fetch_and_xhr() {
    let script = browsai_engine_servo::network_idle_install_script();
    // Idempotency guard
    assert!(
        script.contains("__browsaiNetworkIdleInstalled"),
        "missing install guard"
    );
    assert!(script.contains("__browsaiInFlight"), "missing counter");
    // Fetch hook
    assert!(
        script.contains("origFetch") && script.contains("window.fetch"),
        "missing fetch interceptor"
    );
    // XHR hooks
    assert!(script.contains("XMLHttpRequest"), "missing XHR interceptor");
    assert!(
        script.contains("loadend") && script.contains("error") && script.contains("abort"),
        "missing XHR completion listeners"
    );
    // Counter balance: every increment must have a matching decrement
    // path. Count `__browsaiInFlight++` occurrences (increments) and
    // ensure the same number of decrements via `dec()`.
    let inc_count = script.matches("__browsaiInFlight++").count();
    let dec_calls = script.matches("dec();").count();
    let dec_defs = script.matches("function dec").count();
    assert!(inc_count > 0, "no increments");
    assert!(
        dec_calls >= inc_count,
        "unbalanced counter: {inc_count} inc vs {dec_calls} dec calls"
    );
    assert!(dec_defs >= 1, "missing dec() definition");
}

#[cfg(feature = "servo-runtime")]
#[test]
fn network_idle_interceptor_balanced_parens() {
    // Parens sanity check — make sure nobody breaks the script with a
    // stray bracket. A real JS parser would be better; this catches
    // the common case.
    let script = browsai_engine_servo::network_idle_install_script();
    let opens = script.chars().filter(|c| *c == "(").count();
    let closes = script.chars().filter(|c| *c == ")").count();
    assert_eq!(
        opens, closes,
        "unbalanced parens: {opens} open vs {closes} close"
    );
}
