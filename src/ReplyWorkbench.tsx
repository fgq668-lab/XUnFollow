import { FormEvent, useEffect, useMemo, useState } from "react";
import * as api from "./api";
import type { ReplyConfig, ReplyPost, WorkbenchSnapshot } from "./types";
import { replyQueue } from "./replyQueue";

const money=(v:string)=>`$${Number(v).toFixed(4)}`;
const statusLabel={pending:"待回复",copied:"已复制",replied:"已回复",skipped:"已跳过"};
const errorText=(e:unknown)=>typeof e==="string"?e:e instanceof Error?e.message:"操作失败，请重试";
export default function ReplyWorkbench({onSettings}:{onSettings:()=>void}) {
  const [snapshot,setSnapshot]=useState<WorkbenchSnapshot>();
  const [notice,setNotice]=useState("");
  const [busy,setBusy]=useState(false);
  const [scope,setScope]=useState<ReplyConfig["scope"]>("mutual");
  const [filter,setFilter]=useState("pending");
  const [query,setQuery]=useState("");
  const [focus,setFocus]=useState("");
  const [selection,setSelection]=useState<Set<string>>(new Set());
  const [edits,setEdits]=useState<Record<string,string>>({});
  const [undo,setUndo]=useState<{id:string;status:ReplyPost["status"]}>();
  const [goal,setGoal]=useState(200);
  const [cap,setCap]=useState("");
  const refresh=async()=>{const s=await api.workbenchSnapshot();setSnapshot(s);return s;};
  useEffect(()=>{void refresh().then(s=>setGoal(s.preferences.dailyTarget)).catch(e=>setNotice(errorText(e)));},[]);
  useEffect(()=>{const timer=window.setInterval(()=>void refresh().catch(e=>setNotice(errorText(e))),2000);return()=>window.clearInterval(timer);},[]);
  useEffect(()=>{if(snapshot)setGoal(snapshot.preferences.dailyTarget);},[snapshot?.preferences.dailyTarget]);
  const action=async(work:()=>Promise<unknown>,success:string)=>{
    setBusy(true);try{await work();await refresh();setNotice(success);}catch(e){setNotice(errorText(e));await refresh().catch(()=>{});}finally{setBusy(false);}
  };
  const start=async(e:FormEvent<HTMLFormElement>)=>{
    e.preventDefault();const data=new FormData(e.currentTarget);const f=(key:string)=>String(data.get(key)??"");
    const config:ReplyConfig={scope:f("scope") as ReplyConfig["scope"],keywords:f("keywords").split(/[,，\n]/).map(s=>s.trim()).filter(Boolean),targetCount:Number(f("batchCount")),language:f("language") as ReplyConfig["language"],lookbackHours:Number(f("lookbackHours")),sortMode:f("sortMode") as ReplyConfig["sortMode"],xCapUsd:f("xCap"),aiCapUsd:f("aiCap"),ownUsername:snapshot?.preferences.ownUsername??""};
    await action(()=>api.startReplyRun(config),"后台已开始，候选和草稿会逐步出现；你可以同时处理已有草稿");setFilter("pending");
  };
  const posts=useMemo(()=>replyQueue(snapshot?.posts??[],filter,query),[snapshot,filter,query]);
  const current=posts.find(p=>p.postId===focus)??posts[0];
  const currentIndex=current?posts.indexOf(current):-1;
  const persistEdit=async(post:ReplyPost)=>{const draft=edits[post.postId];if(draft!==undefined&&draft!==post.draft){await api.updateReplyPost(post.postId,draft);setEdits(old=>{const next={...old};delete next[post.postId];return next;});}};
  const mark=(post:ReplyPost,status:ReplyPost["status"])=>void action(async()=>{await persistEdit(post);await api.updateReplyPost(post.postId,undefined,status);setUndo({id:post.postId,status:post.status});setSelection(old=>{const next=new Set(old);next.delete(post.postId);return next;});},status==="replied"?"已记录回复，进度已保存":"已保存状态");
  const openOne=(post:ReplyPost)=>void action(async()=>{await persistEdit(post);await api.openReplyPosts([post.postId]);},(edits[post.postId]??post.draft)?"草稿已复制，X 页面已打开；粘贴并发布后回来标记已回复":"X 页面已打开；这条帖子暂时没有草稿");
  const bulkOpen=()=>void action(async()=>{const ids=posts.filter(p=>selection.has(p.postId)&&["pending","copied"].includes(p.status)).map(p=>p.postId);if(!ids.length||ids.length>5)throw new Error("请选择1–5条待回复帖子");for(const id of ids)await persistEdit(posts.find(p=>p.postId===id)!);await api.openReplyPosts(ids);setSelection(new Set());},"已打开所选页面；多开时不会覆盖剪贴板，请逐条复制对应草稿");
  const toggle=(id:string)=>setSelection(old=>{const next=new Set(old);if(next.has(id))next.delete(id);else if(next.size<5)next.add(id);else setNotice("一次最多选择5条，避免打开太多页面");return next;});
  if(!snapshot) return <section className="workbench-empty">正在打开回复工作台…{notice}</section>;
  const s=snapshot.preferences;const run=snapshot.run;const ready=snapshot.posts.filter(p=>p.draft&&["pending","copied"].includes(p.status)).length;
  const dailyProgress=Math.min(100,snapshot.todayRepliedCount/Math.max(1,s.dailyTarget)*100);
  const jobProgress=run?.phase==="done"?100:run?.phase==="generating"?50+50*run.draftedCount/Math.max(1,run.selectedCount):Math.min(45,(run?.pagesDone??0)/Math.max(1,run?.maxPages??1)*45);
  const stage=run?.phase==="searching"?(snapshot.networkProgress??`搜索中 · ${run.pagesDone}/${run.maxPages}页`):run?.phase==="generating"?`生成中 · ${run.draftedCount}/${run.selectedCount}条`:run?.phase==="paused"?"本轮已暂停":run?.phase==="done"?"本轮准备完成":"尚未开始";
  const configured=snapshot.twitterKeyConfigured&&snapshot.deepseekKeyConfigured&&snapshot.persona.identity&&s.ownUsername;
  return <div className="reply-desk">
    <section className="desk-overview"><div className="desk-heading"><div><p className="eyebrow">X 回复工作台</p><h2>认真接话，慢慢建立关系</h2></div><button onClick={onSettings}>设置 ↗</button></div>
      <div className="desk-metrics"><div><strong>{snapshot.todayRepliedCount}<small> / {s.dailyTarget}</small></strong><span>今天已回复</span></div><div><strong>{snapshot.totalRepliedCount}</strong><span>累计已回复</span></div><div><strong>{snapshot.pendingCount}</strong><span>待回复</span></div><div><strong>{ready}</strong><span>草稿已就绪</span></div>
        <form className="desk-goal" onSubmit={e=>{e.preventDefault();const target=Number(new FormData(e.currentTarget).get("goal"));void action(()=>api.saveReplyGoal(target),"今天的目标已保存");}}><label>今日目标<input name="goal" type="number" min={1} max={5000} value={goal} onInput={e=>setGoal(Number(e.currentTarget.value))} required /></label><button disabled={busy}>保存</button></form>
      </div><div className="progress-track" role="progressbar" aria-label="今日回复目标进度" aria-valuenow={snapshot.todayRepliedCount} aria-valuemin={0} aria-valuemax={s.dailyTarget}><div style={{width:`${dailyProgress}%`}} /></div>
      <p className="wb-note">{snapshot.todayRepliedCount>=s.dailyTarget?"今天的目标已完成！你仍可以继续，也可以休息一下。":"在 X 发布后，请点击「我已发布回复」：跨天、跨批次不再推荐这条帖子。打开或复制不算已回复，应用不会自动读取你的发布行为。"}</p>
    </section>
    <section className="desk-discovery"><div className="desk-heading"><h3>准备下一批回复</h3><span className="wb-note">模型：{s.model}</span></div>
      {!configured&&<p className="wb-error">先到设置中保存 X 用户名、API Key 和人设。<button onClick={onSettings}>去设置</button></p>}
      <form onSubmit={e=>void start(e)} key={`${s.defaultXCapUsd}-${s.defaultAiCapUsd}`}>
        <div className="desk-search-line"><label>内容来源<select name="scope" value={scope} onChange={e=>setScope(e.target.value as ReplyConfig["scope"])}><option value="mutual">互关人的动态 · 默认</option><option value="keywords">全站领域关键词</option></select></label><label className="desk-keywords">关键词{scope==="mutual"?"（可留空）":"（1–8个）"}<input name="keywords" placeholder={scope==="mutual"?"留空看互关动态；也可填写 AI, 创业":"AI 工具, 独立开发"} required={scope==="keywords"} /></label><label>本批条数<input name="batchCount" type="number" defaultValue={20} min={1} max={200} required /></label><button className="open-x" disabled={busy||snapshot.running||!configured}>{snapshot.running?"后台处理中…":"查找并批量生成"}</button></div>
        {scope==="mutual"&&<div className="desk-cache wb-note">{snapshot.mutualCapturedAt?`本地互关名单：${snapshot.mutualCount}人 · ${new Date(snapshot.mutualCapturedAt).toLocaleDateString("zh-CN")}采集`:"首次会读取关注者和正在关注名单，以稳定 X ID 验证互关；下次复用本地名单。"}<button type="button" disabled={busy||snapshot.running} onClick={()=>void action(api.clearMutualCache,"已清除互关缓存，下次查找会重新采集（需要 API 费用）")}>下次刷新名单</button></div>}
        <p className="wb-note">默认寻找中文帖子，待回复队列中文优先；旧英文草稿保留。可在筛选中切换语言，中文不足不会自动混入英文。</p>
        <details className="desk-advanced"><summary>筛选与费用上限</summary><div className="wb-grid"><label>搜索语言<select name="language" defaultValue="zh"><option value="zh">中文 · 默认</option><option value="all">不限（队列中文优先）</option><option value="en">英文</option></select></label><label>时间范围<select name="lookbackHours" defaultValue={72}><option value={12}>近12小时</option><option value={24}>近24小时</option><option value={72}>近3天</option><option value={168}>近7天</option></select></label><label>排序<select name="sortMode" defaultValue="recommended"><option value="recommended">综合推荐</option><option value="latest">最新优先</option><option value="hot">热度优先</option></select></label><label>本轮搜索上限（含名单采集，美元）<input name="xCap" defaultValue={s.defaultXCapUsd} inputMode="decimal" required /></label><label>本轮 AI 上限（美元）<input name="aiCap" defaultValue={s.defaultAiCapUsd} inputMode="decimal" required /></label></div><p className="wb-note">最多搜索30页，互关账号分组轮换；不会把非互关帖子混入结果。数量是期望值，合格结果不足不会硬凑。到达费用上限前暂停或停止新的请求，不会自动提额。</p></details>
      </form>
      {run&&<div className="desk-job" aria-live="polite"><div className="desk-heading"><strong>{stage}</strong><div className="wb-actions">{snapshot.running?<button onClick={()=>void action(api.stopReplyRun,"当前请求完成后暂停")}>暂停</button>:run.phase!=="done"?<button disabled={busy} onClick={()=>void action(api.resumeReplyRun,"已继续后台任务")}>{run.needsExplicitRetry||Number(run.aiUncertainUsd)+Number(run.xUncertainUsd)>0?"确认可能重复计费，继续":"继续任务"}</button>:null}</div></div><div className="progress-track" role="progressbar" aria-label="本轮准备进度" aria-valuenow={Math.round(jobProgress)} aria-valuemin={0} aria-valuemax={100}><div style={{width:`${jobProgress}%`}} /></div><p className="wb-note">候选{run.candidateCount} · 已选{run.selectedCount} · 草稿{run.draftedCount} · 本轮已回复{run.repliedCount} ｜ 搜索 {money(run.xSpentUsd)} / {money(run.xCapUsd)}，AI保守估算 {money(run.aiSpentUsd)} / {money(run.aiCapUsd)}{Number(run.aiUncertainUsd)+Number(run.xUncertainUsd)>0?` ｜ 不确定费用 ${money(String(Number(run.aiUncertainUsd)+Number(run.xUncertainUsd)))}（计入上限）`:""}</p>
        {run.error&&<p className="wb-error">{run.error}</p>}
        {!snapshot.running&&run.error?.includes("上限")&&<div className="wb-cap-raise"><label>提高本轮上限（美元）<input inputMode="decimal" value={cap} onInput={e=>setCap(e.currentTarget.value)} /></label><button disabled={busy||!cap} onClick={()=>void action(()=>run.phase==="paused"?api.increaseReplyXCap(cap):api.increaseReplyAiCap(cap),"上限已提高，继续准备")}>{run.phase==="paused"?"提高搜索上限并继续":"提高 AI 上限并继续"}</button></div>}
      </div>}
    </section>
    <section className="desk-queue"><div className="desk-toolbar"><div className="desk-filters">{[["pending","待回复"],["ready","有草稿"],["replied","已回复"],["skipped","已跳过"],["all","全部"]].map(([key,label])=><button key={key} className={filter===key?"active":""} onClick={()=>{setFilter(key);setSelection(new Set());}}>{label}</button>)}</div><input aria-label="搜索本地帖子" value={query} onChange={e=>setQuery(e.target.value)} placeholder="搜索作者或内容" /><button disabled={busy||!undo} onClick={()=>void action(async()=>{if(undo)await api.updateReplyPost(undo.id,undefined,undo.status);setUndo(undefined);},"已撤销上一步状态")}>撤销标记</button></div>
      <div className="desk-bulk"><span className="wb-note">{posts.length}条展示 · 全局去重保留历史{snapshot.posts.length>=1000?" · 最多显示最近1000条":""}</span><div><button disabled={busy||!posts.some(p=>["pending","copied"].includes(p.status))} onClick={()=>setSelection(new Set(posts.filter(p=>["pending","copied"].includes(p.status)).slice(0,s.openBatchSize).map(p=>p.postId)))}>选前{s.openBatchSize}条</button><button disabled={busy||selection.size===0} onClick={bulkOpen}>打开所选 {selection.size||""} ↗</button></div></div>
      {current?<div className="desk-columns"><div className="desk-list" aria-label="帖子队列">{posts.map(p=><div className={`desk-list-row ${p.postId===current.postId?"active":""}`} key={p.postId}><input type="checkbox" aria-label={`选择 @${p.username} 的帖子 ${p.postId}`} checked={selection.has(p.postId)} disabled={!["pending","copied"].includes(p.status)} onChange={()=>toggle(p.postId)} /><button onClick={()=>setFocus(p.postId)}><div><strong>@{p.username}</strong><span>{statusLabel[p.status]}</span></div><p>{p.postText}</p><small>{p.generationError?"生成失败":p.draft?"草稿就绪":"等待草稿"}{p.openedAt?" · 已打开":""}</small></button></div>)}</div>
        <article className="desk-editor"><div className="desk-heading"><strong>@{current.username}</strong><span className="wb-note">{currentIndex+1} / {posts.length} · {statusLabel[current.status]}</span></div><p className="wb-note">{new Date(current.createdAt).toLocaleString("zh-CN")}</p><div className="desk-original">{current.postText}</div><p className="wb-reason">{current.reason}</p><label>回复草稿<textarea rows={5} value={edits[current.postId]??current.draft} onInput={e=>{const value=e.currentTarget.value;setEdits(old=>({...old,[current.postId]:value}));}} placeholder="草稿会逐步出现，也可以手动写。" maxLength={5000} /></label>
          {current.generationError&&<p className="wb-error">生成失败：{current.generationError}</p>}
          <div className="desk-draft-tools"><span className="wb-note">{(edits[current.postId]??current.draft).length}字 · {edits[current.postId]!==undefined&&edits[current.postId]!==current.draft?"有未保存修改":"已保存在本机"}</span><button disabled={busy||edits[current.postId]===undefined} onClick={()=>void action(()=>persistEdit(current),"草稿已保存")}>保存修改</button><button title="按当前设置重新生成，计入本轮 AI 上限；成功前保留原草稿" disabled={busy||snapshot.running||run?.phase==="searching"||["replied","skipped"].includes(current.status)} onClick={()=>void action(async()=>{await persistEdit(current);await api.regenerateReplyPost(current.postId);},"已加入本轮重新生成队列，使用当前提示词与模型")}>重新生成</button></div>
          <div className="desk-primary"><button className="open-x" disabled={busy||!["pending","copied"].includes(current.status)} onClick={()=>openOne(current)}>打开 X{(edits[current.postId]??current.draft)?"并复制草稿":""} ↗</button><button disabled={busy||!["pending","copied"].includes(current.status)||!(edits[current.postId]??current.draft)} onClick={()=>void action(async()=>{await persistEdit(current);await api.copyReplyDraft(current.postId);},"草稿已复制")}>只复制</button></div>
          <div className="desk-complete"><button className="next-batch" disabled={busy||["replied","skipped"].includes(current.status)} onClick={()=>mark(current,"replied")}>我已发布回复，下一条 ✓</button><button disabled={busy||["replied","skipped"].includes(current.status)} onClick={()=>mark(current,"skipped")}>跳过</button>{["replied","skipped"].includes(current.status)&&<button disabled={busy} onClick={()=>mark(current,"pending")}>恢复待回复（允许再次处理）</button>}</div>
          <div className="desk-pagination"><button disabled={currentIndex<=0} onClick={()=>setFocus(posts[currentIndex-1].postId)}>上一条</button><span className="wb-note">打开和复制不等于已发布；请亲自在 X 粘贴并检查。</span><button disabled={currentIndex>=posts.length-1} onClick={()=>setFocus(posts[currentIndex+1].postId)}>下一条</button></div>
        </article></div>:<div className="workbench-empty">{snapshot.running?"后台正在查找和准备，帖子会逐步出现。":filter==="pending"?"待回复队列已清空。可以准备下一批，或切换查看已回复历史。":"当前筛选下没有帖子。"}</div>}
    </section>
    {notice&&<div className="toast" role="status" key={notice} onAnimationEnd={()=>setNotice("")}>{notice}</div>}
  </div>;
}
