/** Browser-only fixture for the real chat UI. Never included in production entries. */
import React from "react";
import { legacySettingsFixture } from "../../src/testing/settingsFixtures";
import { createRoot } from "react-dom/client";
import { nativeHistoryFixture, registeredHistoryFixture, catalogPageFixture } from "../../src/testing/historyCatalogFixtures";
import { normalizeThemeId } from "../../src/settings/theme";
import "../../src/theme.css";
const win = window as any;
const parameters = new URLSearchParams(location.search);
const composerPreview = parameters.has("composer");
const models = composerPreview ? [
 {configuredProviderId:"fixture",sidecarId:"fixture",modelId:"deepseek-flash",modelName:"DeepSeek-V4.1-Flash"},
 {configuredProviderId:"fixture",sidecarId:"fixture",modelId:"deepseek-v4-pro",modelName:"DeepSeek-V4-Pro"},
] : [{configuredProviderId:"fixture",sidecarId:"fixture",modelId:"fixture",modelName:"预览模型"}];
if (parameters.get("models") === "many") {
 for (let index = 0; index < 30; index++) models.push({configuredProviderId:"fixture",sidecarId:"fixture",modelId:`qa-${index}`,modelName:`QA-long-model-name-for-narrow-window-${index}-Flash`});
}
const settings = {...legacySettingsFixture(),updateRepo:"",autostart:false,settingsLarge:false,chatLarge:false,userAvatar:null as string | null,language:"zh-CN",theme:normalizeThemeId(parameters.get("theme") ?? "lavender"),personaId:"xiaozhu",userName:"",providerId:"fixture",modelId:models[0].modelId,memoryAutoExtract:true,memoryAiUse:true,worklogAutoArchive:true,scheduledTasks:[],providers:[],mouseFollow:false};
let selection: any = {mode:"inherit"};
const listeners = new Map<number, {event:string;handler:number}>();
let callback = 0;
const session = "ses_preview";
const messages: any[] = [];
const eventStreams = new Set<ReadableStreamDefaultController<Uint8Array>>();
win.__TAURI_INTERNALS__ = {
 metadata: { currentWindow: { label: "chat" }, currentWebview: { label: "chat" } },
 transformCallback: (fn: Function) => { const id = ++callback; win[`_${id}`] = fn; return id; },
 unregisterCallback: () => {}, convertFileSrc: (path: string) => path,
 invoke: async (command: string, args: any = {}) => {
  switch(command) {
   case "get_settings": return settings;
   case "set_settings": {
    Object.assign(settings, args.settings);
    localStorage.setItem("yume-avatar-qa-settings", JSON.stringify(settings));
    for (const listener of listeners.values()) if (listener.event === "deskmate://settings-changed") win[`_${listener.handler}`]?.({event:listener.event,payload:{...settings}});
    return;
   }
   case "installed_packs": return parameters.has("historyAvatars") ? [
    {packId:"xiaoxiongchong",version:"1",personaIds:["xiaoxiongchong"],thumbnailPath:"/qa-bear.png"},
    {packId:"baobao",version:"1",personaIds:["baobao"],thumbnailPath:"/qa-cat.png"}
   ] : [];
   case "plugin:path|resolve_directory": return "/qa";
   case "plugin:path|join": return args.paths.join("/");
   case "get_pet_visibility_error": return null;
   case "app_version": return "0.4.18";
   case "load_persona": return {persona:"你是小著。",skills:null,placeholders:null};
   case "chat_model_catalog": return {models,defaultModel:models[0]};
   case "chat_model_resolve": return models.find(model => model.modelId === (args.selection?.mode === "override" ? args.selection.modelId : models[0].modelId)) ?? models[0];
   case "history_model_selection_get": return selection;
   case "history_model_selection_set": selection = args.selection; return;
   case "history_recent_workspaces": return composerPreview ? ["/private/tmp/yume-composer-qa/Output","/private/tmp/yume-composer-qa/very-long-workspace-name-for-narrow-window"] : [];
   case "history_validate_workspace": return args.directory;
   case "tool_permission_pending": return [];
   case "history_register_native_session": return registeredHistoryFixture(args);
   case "history_catalog_list": return catalogPageFixture(parameters.has("historyAvatars") ? [nativeHistoryFixture(session,".","混合角色头像验收")] : []);
   case "history_catalog_access": return nativeHistoryFixture(session,".");
   case "history_catalog_load": return {entry:nativeHistoryFixture(session,"."),messages:parameters.has("historyAvatars") ? [
    {role:"user",text:"历史提问",time:1,messageId:"u_old"},
    ...["xiaoxiongchong","baobao","xiaozhu-sandaime",undefined,"removed-persona"].map((personaId,index)=>({role:"assistant",text:`历史回复 ${index+1}`,time:index+2,messageId:`a_${index}`,personaId}))
   ] : []};
   case "sidecar_base_url": return location.origin;
   case "memory_context": return {memories:[],promptBlock:""};
   case "memory_automation_status": return {pending:0,failed:0};
   case "agent_run_read": return {active:null,recent:[]};
   case "plugin:event|listen": { const id = ++callback; listeners.set(id, args); return id; }
   case "plugin:event|unlisten": listeners.delete(args.eventId); return;
   default:return undefined;
  }
 }
};
win.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
const originalFetch=window.fetch.bind(window);
window.fetch=async(input: any,init?:RequestInit)=>{
 const url=new URL(String(input),location.href);
 if(url.pathname==="/event")return new Response(new ReadableStream<Uint8Array>({start(controller){eventStreams.add(controller);init?.signal?.addEventListener("abort",()=>{eventStreams.delete(controller);controller.close();},{once:true});}}),{headers:{"Content-Type":"text/event-stream"}});
 if(url.pathname==="/session") return Response.json(init?.method==="POST"?{id:session,title:"预览",directory:"."}:[]);
 if(url.pathname.endsWith("/prompt_async")){
  const body=JSON.parse(String(init?.body));win.__lastPrompt=body;const user=body.parts.find((p:any)=>p.type==="text")?.text??"";
  messages.push({info:{id:body.messageID,sessionID:session,role:"user",system:body.system,time:{created:Date.now()}},parts:[{type:"text",text:user}]});
  messages.push({info:{id:`msg_reply_${messages.length}`,sessionID:session,role:"assistant",parentID:body.messageID,finish:"stop",time:{created:Date.now(),completed:Date.now()}},parts:[{type:"text",text:parameters.has("avatars") && user.includes("排版") ? "这是长消息排版检查。\n\n```ts\nconst sample = \"long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_long_code_value_\";\n```\n\n| 项目 | 结果 |\n| --- | --- |\n| 圆角头像 | 正常 |\n| 长代码 | 可横向滚动 |" : user.includes("模型")?"这是界面预览。模型信息会由当前会话配置决定。":"好，我们继续。以后会按你的偏好简短回答，工作进展也会在后台整理。"}]});
  setTimeout(()=>{
   const reply=messages[messages.length-1];
   for(const event of [
    {type:"message.updated",properties:{info:reply.info}},
    {type:"message.part.updated",properties:{part:{...reply.parts[0],id:`part_${reply.info.id}`,messageID:reply.info.id,sessionID:session}}},
    {type:"session.idle",properties:{sessionID:session}}
   ])for(const events of eventStreams)events.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(event)}\n\n`));
  },120);
  return new Response(null,{status:204});
 }
 if(url.pathname.endsWith("/message"))return Response.json(messages.map(m=>({...m,parts:m.parts.map((p:any,index:number)=>({...p,id:`part_${m.info.id}_${index}`,messageID:m.info.id,sessionID:session}))})));
 if(url.pathname.endsWith("/abort"))return new Response(null,{status:204});
 if(url.pathname==="/session/status")return Response.json({});
 return originalFetch(input,init);
};
const {default:ChatApp}=await import("../../src/chat/ChatApp");
if (parameters.has("avatars")) {
 const saved = localStorage.getItem("yume-avatar-qa-settings");
 if (saved) Object.assign(settings, JSON.parse(saved));
 window.addEventListener("storage", event => {
  if (event.key !== "yume-avatar-qa-settings" || !event.newValue) return;
  Object.assign(settings, JSON.parse(event.newValue));
  for (const listener of listeners.values()) if (listener.event === "deskmate://settings-changed") win[`_${listener.handler}`]?.({event:listener.event,payload:{...settings}});
 });
}
if (parameters.has("settings")) {
 const { default: SettingsApp } = await import("../../src/settings/SettingsApp");
 createRoot(document.getElementById("root")!).render(<SettingsApp/>);
} else createRoot(document.getElementById("root")!).render(<ChatApp/>);
if (composerPreview) {
 const control = document.createElement("select");
 control.setAttribute("aria-label", "预览主题");
 control.style.cssText = "position:fixed;top:52px;right:12px;z-index:10000;font:11px system-ui";
 for (const theme of ["dark", "mint", "peach", "lavender"]) control.add(new Option(theme, theme));
 control.value = settings.theme;
 control.addEventListener("change", () => {
  settings.theme = normalizeThemeId(control.value);
  for (const listener of listeners.values()) if (listener.event === "deskmate://settings-changed") win[`_${listener.handler}`]?.({event:listener.event,payload:{...settings}});
 });
 document.body.append(control);
}
