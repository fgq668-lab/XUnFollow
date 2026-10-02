import { FormEvent, useEffect, useRef, useState } from "react";
import * as api from "./api";
import type { ReplyConfig, ReplyPersona, ReplyPost, WorkbenchSnapshot } from "./types";

const defaults: ReplyConfig = { keywords: [], targetCount: 20, language: "zh", lookbackHours: 24, sortMode: "recommended", xCapUsd: "0.05", aiCapUsd: "0.10", ownUsername: "" };
const formatMoney = (value: string) => `$${Number(value).toFixed(6)}`;
const message = (error: unknown) => typeof error === "string" ? error : error instanceof Error ? error.message : "操作失败，请重试";

export default function ReplyWorkbench({ ownUsername }: { ownUsername?: string }) {
  const [snapshot, setSnapshot] = useState<WorkbenchSnapshot>();
  const [persona, setPersona] = useState<ReplyPersona>({ identity: "", topics: "", voice: "", language: "zh", avoid: "" });
  const [config, setConfig] = useState<ReplyConfig>({ ...defaults, ownUsername: ownUsername ?? "" });
  const [keywordText, setKeywordText] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [topic, setTopic] = useState("");
  const [article, setArticle] = useState("");
  const deepseekKey = useRef<HTMLInputElement>(null);
  const twitterKey = useRef<HTMLInputElement>(null);
  const [keysChanged, setKeysChanged] = useState(false);
  const [raisedCap, setRaisedCap] = useState("");
  const personaTouched = useRef(false);

  const refresh = async () => {
    try {
      const next = await api.workbenchSnapshot();
      setSnapshot(next);
      if (next.article && !article) { setTopic(next.article.topic); setArticle(next.article.body); }
    } catch (error) { setNotice(message(error)); }
  };
  useEffect(() => { void api.workbenchSnapshot().then((next) => { setSnapshot(next); if (!personaTouched.current) setPersona(next.persona); if (next.article) { setTopic(next.article.topic); setArticle(next.article.body); } }).catch((error) => setNotice(message(error))); }, []);
  useEffect(() => { if (!snapshot?.run || !["searching","generating"].includes(snapshot.run.phase)) return; const timer = window.setInterval(() => void refresh(), 1500); return () => window.clearInterval(timer); }, [snapshot?.run?.phase]);

  const saveSettings = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    // Native autofill/accessibility input can update an input's visible value
    // without updating React's onChange state. Read the submitted DOM values.
    const values = new FormData(event.currentTarget);
    const field = (name: string) => String(values.get(name) ?? "");
    const submittedPersona: ReplyPersona = {
      identity: field("identity"), topics: field("topics"), voice: field("voice"),
      language: field("language") as ReplyPersona["language"], avoid: field("avoid"),
    };
    const x = field("twitterKey").trim();
    const ds = field("deepseekKey").trim();
    personaTouched.current = true;
    setPersona(submittedPersona);
    setBusy(true);
    try {
      await api.saveReplyPersona(submittedPersona);
      if (x) await api.saveApiKey(x);
      if (ds) await api.saveDeepseekKey(ds);
      if (twitterKey.current) twitterKey.current.value = "";
      if (deepseekKey.current) deepseekKey.current.value = "";
      setKeysChanged(false);
      await refresh(); setNotice("人设和 API 设置已保存在本机");
    } catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  };

  const start = async (event: FormEvent) => {
    event.preventDefault(); setBusy(true);
    try {
      const keywords = keywordText.split(/[,，\n]/).map((s) => s.trim()).filter(Boolean);
      await api.startReplyRun({ ...config, keywords });
      await refresh(); setNotice("已开始搜索；合格帖子和草稿会逐步显示");
    } catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  };

  const action = async (work: () => Promise<unknown>, success: string) => {
    setBusy(true);
    try { await work(); await refresh(); setNotice(success); }
    catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  };

  const run = snapshot?.run;
  const progress = run ? run.phase === "searching" ? Math.min(45, run.pagesDone / Math.max(1, run.maxPages) * 45) : run.phase === "generating" ? 50 + run.draftedCount / Math.max(1, run.selectedCount) * 50 : run.phase === "done" ? 100 : 0 : 0;
  const stageText = run?.phase === "searching" ? `正在搜索 · ${run.pagesDone}/${run.maxPages} 页 · ${run.candidateCount} 个候选` : run?.phase === "generating" ? `正在生成草稿 · ${run.draftedCount}/${run.selectedCount}` : run?.phase === "paused" ? "任务已暂停" : run?.phase === "done" ? "本轮已完成" : "尚未开始";

  return <div className="workbench">
    <section className="workbench-intro">
      <div><p className="eyebrow">X 回复工作台</p><h2>找到值得回复的帖子，批量准备草稿</h2><p>你自己复制、检查并在 X 上发布。XUnFollow 不会自动回复，也不需要 X 密码或 Cookie。</p></div>
      <div className="workbench-day"><strong>{snapshot?.todayRepliedCount ?? 0}</strong><span>今天已标记回复</span></div>
    </section>

    <details className="workbench-settings" open={!snapshot?.persona.identity || !snapshot?.deepseekKeyConfigured}>
      <summary>人设与 API 设置 {snapshot?.persona.identity && snapshot.deepseekKeyConfigured ? "✓" : "· 首次使用先填写"}</summary>
      <form onSubmit={(event) => void saveSettings(event)}>
        <div className="wb-grid">
          <label>我的身份<input name="identity" value={persona.identity} onInput={(e) => { const value = e.currentTarget.value; personaTouched.current = true; setPersona((current) => ({ ...current, identity: value })); }} placeholder="例如：独立开发者，分享真实产品实践" required maxLength={1000} /></label>
          <label>擅长领域<input name="topics" value={persona.topics} onInput={(e) => { const value = e.currentTarget.value; personaTouched.current = true; setPersona((current) => ({ ...current, topics: value })); }} placeholder="AI 工具、编程、创业" maxLength={1000} /></label>
          <label>表达风格<input name="voice" value={persona.voice} onInput={(e) => { const value = e.currentTarget.value; personaTouched.current = true; setPersona((current) => ({ ...current, voice: value })); }} placeholder="真诚、具体、不夸张" maxLength={1000} /></label>
          <label>输出语言<select name="language" value={persona.language} onChange={(e) => { const value = e.currentTarget.value as ReplyPersona["language"]; personaTouched.current = true; setPersona((current) => ({ ...current, language: value })); }}><option value="zh">中文</option><option value="en">英文</option><option value="auto">跟随原帖</option></select></label>
          <label className="wb-wide">避免的说法<input name="avoid" value={persona.avoid} onInput={(e) => { const value = e.currentTarget.value; personaTouched.current = true; setPersona((current) => ({ ...current, avoid: value })); }} placeholder="例如：不要硬广，不要过度赞美" maxLength={1000} /></label>
          <label>TwitterAPI.io Key<input name="twitterKey" ref={twitterKey} className="secret-input" type="password" autoComplete="off" spellCheck={false} onInput={() => setKeysChanged(true)} placeholder={snapshot?.twitterKeyConfigured ? "已保存；留空沿用" : "粘贴官方 API Key"} /></label>
          <label>DeepSeek 官方 Key<input name="deepseekKey" ref={deepseekKey} className="secret-input" type="password" autoComplete="off" spellCheck={false} onInput={() => setKeysChanged(true)} placeholder={snapshot?.deepseekKeyConfigured ? "已保存；留空沿用" : "粘贴 DeepSeek 官方 API Key"} /></label>
        </div>
        <p className="wb-note">Key 只保存在本机数据库，不在页面回显。生成草稿时，选中帖子的内容和你填写的人设会发送给 DeepSeek 官方 API。</p>
        <button className="open-x" type="submit" disabled={busy}>{keysChanged ? "保存 Key 和人设" : "保存人设与设置"}</button>
      </form>
    </details>

    <section className="workbench-search">
      <div><p className="eyebrow">今日搜索</p><h2>想准备多少条回复？</h2></div>
      <form onSubmit={(event) => void start(event)}>
        <div className="wb-grid">
          <label className="wb-wide">领域关键词（用逗号分隔，可填 1–8 个）<input value={keywordText} onChange={(e) => setKeywordText(e.target.value)} placeholder="例如 AI 工具, 独立开发, 产品增长" required /></label>
          <label>目标数量<input type="number" min={1} max={200} value={config.targetCount} onChange={(e) => setConfig({ ...config, targetCount: Number(e.target.value) })} required /></label>
          <label>搜索语言<select value={config.language} onChange={(e) => setConfig({ ...config, language: e.target.value as ReplyConfig["language"] })}><option value="zh">中文</option><option value="en">英文</option><option value="all">不限</option></select></label>
          <label>时间范围<select value={config.lookbackHours} onChange={(e) => setConfig({ ...config, lookbackHours: Number(e.target.value) })}><option value={12}>近 12 小时</option><option value={24}>近 24 小时</option><option value={72}>近 3 天</option><option value={168}>近 7 天</option></select></label>
          <label>排序<select value={config.sortMode} onChange={(e) => setConfig({ ...config, sortMode: e.target.value as ReplyConfig["sortMode"] })}><option value="recommended">综合推荐</option><option value="hot">热度优先</option><option value="latest">最新优先</option></select></label>
          <label>X 用户名<input value={config.ownUsername} onChange={(e) => setConfig({ ...config, ownUsername: e.target.value.replace(/^@/, "") })} placeholder="用于排除自己的帖子" required /></label>
          <label>搜索费用上限（美元）<input inputMode="decimal" value={config.xCapUsd} onChange={(e) => setConfig({ ...config, xCapUsd: e.target.value })} required /></label>
          <label>AI 费用上限（美元）<input inputMode="decimal" value={config.aiCapUsd} onChange={(e) => setConfig({ ...config, aiCapUsd: e.target.value })} required /></label>
        </div>
        <p className="wb-note">搜索最多 {Math.min(30, Math.max(1, Math.ceil(config.targetCount * 3 / 20)))} 页；按每页最多 20 条、当前公开单价估算，搜索不超过约 {formatMoney(String(Math.min(30, Math.max(1, Math.ceil(config.targetCount * 3 / 20))) * 0.003))}。目标数量不是保证找到或能回复的数量。</p>
        <div className="wb-actions"><button className="next-batch" type="submit" disabled={busy || snapshot?.running || !snapshot?.deepseekKeyConfigured || !snapshot?.twitterKeyConfigured || !snapshot?.persona.identity}>搜索并批量生成</button></div>
      </form>
    </section>

    {run && <section className="workbench-progress" aria-live="polite">
      <div className="wb-progress-head"><div><p className="eyebrow">本轮进度</p><h2>{stageText}</h2></div><div className="wb-actions">{snapshot?.running ? <button onClick={() => void action(api.stopReplyRun,"正在暂停；当前请求完成后停止")}>暂停</button> : run.phase !== "done" ? <button className="open-x" onClick={() => void action(api.resumeReplyRun,"已继续后台任务")}>{run.needsExplicitRetry || run.error ? "确认可能重复计费，继续" : "继续未完成任务"}</button> : null}</div></div>
      <div className="progress-track"><div style={{ width: `${progress}%` }} /></div>
      <div className="wb-stages"><span className="active">① 搜索 {run.pagesDone}/{run.maxPages} 页</span><span className={run.phase !== "searching" ? "active" : ""}>② 筛选 {run.selectedCount} 条</span><span className={run.phase === "generating" || run.phase === "done" ? "active" : ""}>③ 草稿 {run.draftedCount}/{run.selectedCount}</span></div>
      <p>搜索预估最多 {formatMoney(run.xEstimatedUsd)}，已用 {formatMoney(run.xSpentUsd)} / 上限 {formatMoney(run.xCapUsd)}；AI 已选帖子保守预留约 {formatMoney(run.aiEstimatedUsd)}，已用 {formatMoney(run.aiSpentUsd)}，处理中预留 {formatMoney(run.aiReservedUsd)} / 上限 {formatMoney(run.aiCapUsd)}。{Number(run.xUncertainUsd) + Number(run.aiUncertainUsd) > 0 ? `另有不确定费用：搜索 ${formatMoney(run.xUncertainUsd)}，AI ${formatMoney(run.aiUncertainUsd)}（已计入上限）。` : ""}</p>
      {run.error && <p className="wb-error">{run.error}</p>}
      {!snapshot?.running && run.phase === "done" && run.error?.includes("上限") && <div className="wb-cap-raise"><label>如果想继续生成，可提高本轮 AI 上限<input inputMode="decimal" value={raisedCap} onChange={(e) => setRaisedCap(e.target.value)} placeholder={`高于 ${run.aiCapUsd} 美元`} /></label><button disabled={busy || !raisedCap} onClick={() => void action(() => api.increaseReplyAiCap(raisedCap),"已提高本轮 AI 上限并继续生成")}>提高上限并继续</button></div>}
      {run.phase === "done" && run.selectedCount < run.targetCount && <p>合格帖子只有 {run.selectedCount} 条；不会拿低质量结果凑满目标。</p>}
    </section>}

    {snapshot?.posts.length ? <section className="workbench-posts"><div><p className="eyebrow">候选与草稿</p><h2>{snapshot.posts.length} 条帖子</h2></div><div className="wb-post-list">{snapshot.posts.map((post) => <PostCard key={post.postId} post={post} disabled={busy} canRegenerate={run?.phase !== "searching"} onRefresh={refresh} onNotice={setNotice} />)}</div></section> : run && <section className="workbench-empty">{snapshot.running ? "正在寻找合格帖子，请稍候…" : "目前没有可显示的帖子。"}</section>}

    <section className="workbench-article"><div><p className="eyebrow">X 文章草稿</p><h2>用同一人设写一篇文章</h2></div><label>主题<input value={topic} onChange={(e) => setTopic(e.target.value)} placeholder="例如：独立开发者如何挑选 AI 工具" maxLength={300} /></label><p className="wb-note">本次文章的 DeepSeek 请求上限沿用上方 AI 费用上限 {formatMoney(config.aiCapUsd)}；请求前会按最大输出量预留。</p><div className="wb-actions"><button className="open-x" disabled={busy || !snapshot?.deepseekKeyConfigured || !snapshot?.persona.identity} onClick={() => void action(async () => { setArticle(await api.createArticleDraft(topic,config.aiCapUsd)); },"文章草稿已生成")}>生成文章草稿</button></div>{article && <><textarea value={article} onChange={(e) => setArticle(e.target.value)} rows={12} />{snapshot?.article && <p className="wb-note">上次生成确认费用 {formatMoney(snapshot.article.costUsd)} / 单次上限 {formatMoney(snapshot.article.capUsd)}</p>}<div className="wb-actions"><button onClick={() => void action(() => api.saveArticleDraft(topic,article),"文章草稿已保存在本机")}>保存修改</button><button onClick={() => void navigator.clipboard.writeText(article).then(() => setNotice("文章已复制"))}>复制文章</button></div></>}</section>
    {notice && <div className="toast" onAnimationEnd={() => setNotice("")}>{notice}</div>}
  </div>;
}

function PostCard({ post, disabled, canRegenerate, onRefresh, onNotice }: { post: ReplyPost; disabled: boolean; canRegenerate: boolean; onRefresh: () => Promise<void>; onNotice: (message: string) => void }) {
  const [draft, setDraft] = useState(post.draft);
  const [editing, setEditing] = useState(false);
  useEffect(() => { if (!editing) setDraft(post.draft); }, [post.draft, editing]);
  const save = async () => { try { await api.updateReplyPost(post.postId,draft); setEditing(false); await onRefresh(); onNotice("草稿已保存"); } catch (error) { onNotice(message(error)); } };
  const mark = async (status: ReplyPost["status"]) => { try { await api.updateReplyPost(post.postId,undefined,status); await onRefresh(); onNotice("进度已保存"); } catch (error) { onNotice(message(error)); } };
  return <article className={`wb-post ${post.status}`}>
    <div className="wb-post-head"><strong>@{post.username}</strong><span>{new Date(post.createdAt).toLocaleString("zh-CN")}</span><span className="wb-status">{post.status === "replied" ? "已回复" : post.status === "skipped" ? "已跳过" : post.status === "copied" ? "已复制" : "待处理"}</span></div>
    <p className="wb-post-text">{post.postText}</p><p className="wb-reason">推荐理由：{post.reason}</p>
    <div className="wb-draft"><label>回复草稿</label>{post.generationError ? <p className="wb-error">生成失败：{post.generationError}</p> : null}<textarea rows={3} value={draft} onFocus={() => setEditing(true)} onChange={(e) => setDraft(e.target.value)} placeholder="正在生成；也可以手动写…" maxLength={5000} /></div>
    <div className="wb-actions"><button disabled={disabled || !editing} onClick={() => void save()}>保存编辑</button><button disabled={disabled || !canRegenerate} onClick={() => void api.regenerateReplyPost(post.postId).then(onRefresh).then(() => onNotice("已加入重新生成队列")).catch((error) => onNotice(message(error)))}>重新生成</button><button disabled={!draft} onClick={() => void navigator.clipboard.writeText(draft).then(() => mark("copied"))}>复制</button><button onClick={() => void api.openExternalProfile(post.postUrl)}>打开 X ↗</button><button disabled={disabled} onClick={() => void mark("replied")}>已回复</button><button disabled={disabled} onClick={() => void mark("skipped")}>跳过</button></div>
  </article>;
}
