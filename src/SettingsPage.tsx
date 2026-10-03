import { FormEvent, useEffect, useState } from "react";
import * as api from "./api";
import type { ReplyPersona, WorkbenchSnapshot, WorkbenchPreferences } from "./types";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";

export default function SettingsPage({ onSaved }: { onSaved: () => void }) {
  const [snapshot,setSnapshot]=useState<WorkbenchSnapshot>();
  const [notice,setNotice]=useState("");
  const [busy,setBusy]=useState(false);
  const [prompt,setPrompt]=useState("");
  const [creatorPrompt,setCreatorPrompt]=useState("");
  const [defaultCreatorPrompt,setDefaultCreatorPrompt]=useState("");
  useEffect(()=>{void api.workbenchSnapshot().then(s=>{setSnapshot(s);setPrompt(s.preferences.replyPrompt);setCreatorPrompt(s.preferences.creatorPrompt);}).catch(e=>setNotice(String(e)));void api.creatorSnapshot().then(s=>setDefaultCreatorPrompt(s.defaultPrompt)).catch(e=>setNotice(String(e)));},[]);
  const save=async(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault(); const form=event.currentTarget; const data=new FormData(form); const f=(key:string)=>String(data.get(key)??"");
    const persona:ReplyPersona={identity:f("identity"),topics:f("topics"),voice:f("voice"),language:f("language") as ReplyPersona["language"],avoid:f("avoid")};
    const preferences:WorkbenchPreferences={replyPrompt:f("replyPrompt"),catchphrases:f("catchphrases"),model:f("model") as WorkbenchPreferences["model"],dailyTarget:Number(f("dailyTarget")),ownUsername:f("ownUsername").trim().replace(/^@/,""),defaultXCapUsd:f("xCap"),defaultAiCapUsd:f("aiCap"),openBatchSize:Number(f("openBatchSize")),creatorPrompt:f("creatorPrompt"),creatorVoiceSamples:f("creatorVoiceSamples"),creatorDailyTarget:Number(f("creatorDailyTarget")),creatorDayStart:f("creatorDayStart"),creatorDayEnd:f("creatorDayEnd"),remindersEnabled:data.has("remindersEnabled"),reminderSound:data.has("reminderSound")};
    setBusy(true);
    try {
      if(preferences.remindersEnabled&&api.isNative&&!await isPermissionGranted()&&await requestPermission()!=="granted") throw new Error("请先在系统设置中允许 XUnFollow 通知，或关闭发布提醒后保存");
      await api.saveWorkbenchSettings(preferences,persona,f("twitterKey").trim(),f("deepseekKey").trim());
      (form.elements.namedItem("twitterKey") as HTMLInputElement).value=""; (form.elements.namedItem("deepseekKey") as HTMLInputElement).value="";
      setSnapshot(await api.workbenchSnapshot()); onSaved(); setNotice("设置已保存在本机；新的提示词与模型用于下一批或重新生成");
    } catch(e) {setNotice(String(e));} finally {setBusy(false);}
  };
  if(!snapshot) return <section className="settings-page">正在读取本地设置…{notice}</section>;
  const p=snapshot.persona; const s=snapshot.preferences;
  return <section className="settings-page"><div className="desk-heading"><div><p className="eyebrow">偏好与连接</p><h2>设置</h2><p className="wb-note">一次设置，工作台保持清爽。密钥不回显；留空沿用已保存的值。</p></div></div>
    <form onSubmit={e=>void save(e)}>
      <fieldset><legend>账号与 API</legend><div className="wb-grid">
        <label>X 用户名<input name="ownUsername" defaultValue={s.ownUsername} required maxLength={16} placeholder="你的 X 用户名，不需要密码" /></label>
        <label>DeepSeek 官方模型<select name="model" defaultValue={s.model}><option value="deepseek-flash">deepseek-flash · 默认 / 轻快</option><option value="deepseek-v4-pro">deepseek-v4-pro · 费用较高</option></select></label>
        <label>TwitterAPI.io Key<input type="password" name="twitterKey" autoComplete="off" spellCheck={false} placeholder={snapshot.twitterKeyConfigured?"已保存；留空沿用":"粘贴 TwitterAPI.io Key"} /></label>
        <label>DeepSeek 官方 Key<input type="password" name="deepseekKey" autoComplete="off" spellCheck={false} placeholder={snapshot.deepseekKeyConfigured?"已保存；留空沿用":"粘贴 DeepSeek 官方 Key"} /></label>
      </div><p className="wb-note">只连接 api.deepseek.com，不支持第三方代理。密钥保存在本机 SQLite，未额外加密，请保护应用数据目录。生成时帖子、人设与提示词会发送到官方 API。</p></fieldset>
      <fieldset><legend>你的表达与人设</legend><div className="wb-grid">
        <label>我的身份<input name="identity" defaultValue={p.identity} placeholder="独立开发者，分享产品实践" maxLength={1000} required /></label>
        <label>擅长领域<input name="topics" defaultValue={p.topics} placeholder="AI、编程、创业" maxLength={1000} /></label>
        <label>表达风格<input name="voice" defaultValue={p.voice} placeholder="口语、真诚、简短" maxLength={1000} /></label>
        <label>输出语言<select name="language" defaultValue={p.language||"zh"}><option value="zh">中文</option><option value="en">英文</option><option value="auto">跟随原帖</option></select></label>
        <label className="wb-wide">避免的说法<input name="avoid" defaultValue={p.avoid} maxLength={1000} placeholder="不要硬广、不要尬夸" /></label>
        <label className="wb-wide">口头禅 / 语气词<input name="catchphrases" defaultValue={s.catchphrases} maxLength={200} placeholder="比如：哈、呗、慢慢来；留空也可以" /></label>
        <label className="wb-wide">回复提示词<textarea name="replyPrompt" rows={7} value={prompt} onInput={e=>setPrompt(e.currentTarget.value)} required maxLength={6000} /></label>
      </div><div className="settings-prompt-footer"><span className="wb-note">默认一句话、约30字，轻轻鼓励，不分析、不表态、不尬夸。语气词最多自然用一个；生成后请检查。</span><button type="button" onClick={()=>setPrompt(snapshot.defaultReplyPrompt)}>恢复默认提示词</button></div></fieldset>
      <fieldset><legend>日常工作习惯</legend><div className="wb-grid">
        <label>每日回复目标<input name="dailyTarget" type="number" defaultValue={s.dailyTarget} min={1} max={5000} required /></label>
        <label>一次打开的页面数<select name="openBatchSize" defaultValue={s.openBatchSize}>{[1,2,3,4,5].map(n=><option value={n} key={n}>{n} 个页面</option>)}</select></label>
        <label>默认搜索费用上限（美元）<input name="xCap" inputMode="decimal" defaultValue={s.defaultXCapUsd} required /></label>
        <label>默认 AI 费用上限（美元）<input name="aiCap" inputMode="decimal" defaultValue={s.defaultAiCapUsd} required /></label>
      </div><p className="wb-note">每日目标不代表自动发送数量。一批准备最多200条；首次互关名单采集也计入本轮搜索上限。费用显示采用峰时、非缓存价格的保守估算，实际以 Provider 账单为准。</p></fieldset>
      <fieldset><legend>创作与北京时间发布提醒</legend><div className="wb-grid">
        <label className="wb-wide">创作提示词<textarea name="creatorPrompt" rows={6} value={creatorPrompt} onInput={e=>setCreatorPrompt(e.currentTarget.value)} maxLength={6000} required /></label>
        <label className="wb-wide">我平时怎么说话（仅创作使用，选填）<textarea name="creatorVoiceSamples" rows={5} defaultValue={s.creatorVoiceSamples} maxLength={4000} placeholder="贴3–5条你自己写过、觉得自然的帖子，段间空一行。不是让模型抄内容，只参考节奏和用词。请不要放密钥、私信或别人的隐私。" /></label>
        <p className="wb-note wb-wide">少一点AI味，真实语气样稿通常比反复写“不要像AI”更有用。生成时样稿会发送到 DeepSeek 官方；只用于当前提示词参考，不在本地训练模型。已有草稿不会自动重写，自定义创作提示词也不会被升级覆盖。</p>
        <label>每日发布目标<input name="creatorDailyTarget" type="number" min={1} max={200} defaultValue={s.creatorDailyTarget} required /></label>
        <label>时区<input value="北京时间 · Asia/Shanghai (UTC+8)" readOnly /></label>
        <label>时段开始<input name="creatorDayStart" type="time" defaultValue={s.creatorDayStart} required /></label>
        <label>时段结束<input name="creatorDayEnd" type="time" defaultValue={s.creatorDayEnd} required /></label>
        <label className="check-label"><input name="remindersEnabled" type="checkbox" defaultChecked={s.remindersEnabled} />开启本机发布提醒</label>
        <label className="check-label"><input name="reminderSound" type="checkbox" defaultChecked={s.reminderSound} />提醒带声音（取决于系统通知设置）</label>
      </div><div className="settings-prompt-footer"><button type="button" disabled={!defaultCreatorPrompt} onClick={()=>setCreatorPrompt(defaultCreatorPrompt)}>恢复默认创作提示词</button><button type="button" onClick={()=>void api.testCreatorNotification().then(()=>setNotice("已提交本机测试通知；如未出现，请检查系统通知设置/勿扰模式")).catch(e=>setNotice(String(e)))}>测试系统通知</button></div>
      <p className="wb-note">仅审核通过的内容会提醒。关闭窗口会隐藏到托盘，退出应用/关机后提醒停止；重开时逾期任务合并提示，不自动补发。可导出日历交给系统提醒。修改目标/时段不自动覆盖旧排期，请在创作工作台手动重新安排。系统可能禁用通知，请用测试按钮确认。</p></fieldset>
      <div className="settings-save"><span role="status">{notice}</span><button className="open-x" disabled={busy} type="submit">{busy?"保存中…":"保存设置"}</button></div>
    </form>
  </section>;
}
