import {useEffect,useRef,useState} from "react";
import * as api from "./api";
import type {BloggerReference,ReferenceSnapshot} from "./types";
import {beijingDisplay} from "./creatorTime";

export default function CreatorReferencePanel({selected,onSelection,disabled,onReadingChange}:{selected:string[];onSelection:(names:string[])=>void;disabled:boolean;onReadingChange:(reading:boolean)=>void}) {
  const [s,setS]=useState<ReferenceSnapshot>();const [notice,setNotice]=useState("");const [busy,setBusy]=useState(false);const initialized=useRef(false);
  const refresh=async()=>{setS(await api.referenceSnapshot());};
  useEffect(()=>{void refresh().catch(e=>setNotice(String(e)));const timer=window.setInterval(()=>void refresh().catch(e=>setNotice(String(e))),2000);return()=>window.clearInterval(timer);},[]);
  useEffect(()=>{onReadingChange(busy||Boolean(s?.reading));return()=>onReadingChange(false);},[busy,s?.reading,onReadingChange]);
  useEffect(()=>{if(s&&!initialized.current){initialized.current=true;onSelection(s.activeHandles.filter(name=>s.references.some(r=>r.username===name)));}},[s,onSelection]);
  const choose=async(names:string[])=>{await api.saveCreatorReferenceSelection(names);onSelection(names);};
  const blocked=disabled||busy||Boolean(s?.reading);
  return <section className="creator-form creator-reference-panel"><details open><summary>参考博主写法 · 学节奏，不搬运内容</summary><p className="wb-note">填写 X 主页或用户名，每次只读一页、最多20条，不自动翻页。过滤回复、转帖、非中文及明显粗口/露骨引流样本；基础过滤不能替代人工检查。只分析文本，不查看图片。选择参考后，生成时会把少量样本发送到官方 DeepSeek；不训练模型、不冒充博主。</p>
    <form className="reference-read-form" onSubmit={e=>{e.preventDefault();const d=new FormData(e.currentTarget);setBusy(true);setNotice("正在只读获取一页公开帖子，请稍候…");void api.readCreatorReference(String(d.get("username")),String(d.get("cap")),d.get("ack")==="on").then(async r=>{await refresh();await choose([...selected.filter(n=>n!==r.username),r.username].slice(-2));setNotice(`@${r.username} 的${r.posts.length}条合格样本已缓存并选为参考。再次生成会沿用缓存，不重复读取。`);}).catch(async e=>{setNotice(String(e));await refresh().catch(()=>{});}).finally(()=>setBusy(false));}}>
      <label>参考博主<input name="username" defaultValue="https://x.com/xupaopaogm" placeholder="@用户名 或 https://x.com/用户名" required /></label><label>本次读取上限（美元）<input name="cap" inputMode="decimal" defaultValue="0.01" required /></label><button disabled={blocked}>确认上限，读取并选为参考</button>
      {!!s?.lastRead?.uncertainMicros&&<label className="check-label wb-wide"><input type="checkbox" name="ack" required />上次可能计费约 ${(s.lastRead.uncertainMicros/1e6).toFixed(6)}，确认手动重新读取；不会自动重试</label>}
    </form>
    <p className="wb-note">单页按文档保守预留 $0.003；最终按全部返回条数记账，过滤掉的条目也可能计费。缓存离线可看。不是“成功账号认证”，点赞/浏览量不证明真实性，也不保证涨粉。</p>
    {(busy||s?.reading)&&<p className="reference-reading" role="status" aria-live="polite">正在读取公开样本…不修改 X，也不会生成或发布新帖。</p>}
    {s?.lastRead&&<p className="wb-note">最近读取 @{s.lastRead.username} · {s.lastRead.phase==="done"?"已结束":s.lastRead.phase==="reading"?"读取中":"已停止，计费不确定"} · 估算已确认 ${(s.lastRead.spentMicros/1e6).toFixed(6)}{s.lastRead.error&&` · ${s.lastRead.error}`}</p>}
    <div className="reference-cache">{s?.references.map(r=><ReferenceCard key={r.username} r={r} selected={selected.includes(r.username)} disabled={blocked} onToggle={on=>{if(on&&selected.length>=2){setNotice("最多同时参考2个博主，先取消一个。");return;}void choose(on?[...selected,r.username]:selected.filter(n=>n!==r.username)).catch(e=>setNotice(String(e)));}} onSaved={async()=>{await refresh();setNotice("参考规则已保存在本机，下次生成生效，不调用 API。");}} onError={setNotice} />)}</div>
    {!s?.references.length&&<p className="wb-note">尚未缓存参考。可以先读取一个博主，再查看样本、调整规则，最后重新生成。</p>}
    {notice&&<p className="creator-reference-notice" role="status">{notice}</p>}
  </details></section>;
}
function ReferenceCard({r,selected,disabled,onToggle,onSaved,onError}:{r:BloggerReference;selected:boolean;disabled:boolean;onToggle:(on:boolean)=>void;onSaved:()=>Promise<void>;onError:(message:string)=>void}) {
  const [saving,setSaving]=useState(false);
  return <div className="reference-card"><label className="check-label"><input type="checkbox" checked={selected} disabled={disabled} onChange={e=>onToggle(e.target.checked)} />生成时参考 @{r.username} 的写法</label><p className="wb-note">北京时间 {beijingDisplay(r.capturedAt)} · {r.posts.length}条合格样本 · 排除{r.filteredCount}条 · 中位{r.medianLength}字符（含链接） · {r.multilineCount}条有换行</p>
    <details><summary>看样本 / 调整参考规则</summary><form onSubmit={e=>{e.preventDefault();const guide=String(new FormData(e.currentTarget).get("guide"));setSaving(true);void api.saveCreatorReferenceGuide(r.username,guide).then(onSaved).catch(e=>onError(String(e))).finally(()=>setSaving(false));}}><label>借鉴什么，不借鉴什么<textarea name="guide" key={r.guide} defaultValue={r.guide} rows={3} maxLength={1500}/></label><button disabled={disabled||saving}>保存参考规则（不扣费）</button></form><div className="reference-posts">{r.posts.map(p=><article key={p.id}><p>{p.text}</p><span className="wb-note">{p.likes}赞 · {p.replies}回复 · {p.views}浏览 · </span><a href={p.url} onClick={e=>{e.preventDefault();void api.openExternalProfile(p.url).catch(e=>onError(String(e)));}}>由你打开原帖 ↗</a></article>)}</div></details>
  </div>;
}
