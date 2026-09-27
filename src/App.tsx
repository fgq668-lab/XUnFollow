import { ChangeEvent, FormEvent, useEffect, useMemo, useRef, useState } from "react";
import * as api from "./api";
import type { Bootstrap, Candidate, DecisionStatus } from "./types";

type Tab = "pending" | "unfollowed" | "keep" | "later" | "changed";

const labels: Record<Tab, string> = {
  pending: "全部待处理",
  unfollowed: "已取关",
  keep: "保留",
  later: "稍后",
  changed: "关系变化",
};

const today = () => new Intl.DateTimeFormat("en-CA", { timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone }).format(new Date());
const money = new Intl.NumberFormat("en-US", { minimumFractionDigits: 0, maximumFractionDigits: 2 });
const number = new Intl.NumberFormat("zh-CN");

function errorMessage(error: unknown, fallback: string) {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message) return error.message;
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string" && error.message) return error.message;
  return fallback;
}

function decisionOf(state: Bootstrap, candidate: Candidate): DecisionStatus {
  return state.decisions[candidate.stableXId]?.status ?? "pending";
}

function App() {
  const [state, setState] = useState<Bootstrap | null>(null);
  const [tab, setTab] = useState<Tab>("pending");
  const [query, setQuery] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [showSetup, setShowSetup] = useState(false);
  const importInput = useRef<HTMLInputElement>(null);

  const refresh = async () => {
    try {
      setState(await api.bootstrap());
    } catch (error) {
      setNotice(errorMessage(error, "无法读取本地数据"));
    }
  };

  useEffect(() => { void refresh(); }, []);

  const counts = useMemo(() => {
    const result: Record<Tab, number> = { pending: 0, unfollowed: 0, keep: 0, later: 0, changed: 0 };
    state?.candidates.forEach((candidate) => { result[decisionOf(state, candidate)] += 1; });
    return result;
  }, [state]);

  const completedToday = useMemo(
    () => Object.values(state?.decisions ?? {}).filter((item) => item.actionDay === today()).length,
    [state],
  );
  const queue = useMemo(
    () => (state?.candidates ?? []).filter((candidate) => decisionOf(state!, candidate) === "pending"),
    [state],
  );
  const current = completedToday < (state?.dailyGoal ?? 10) ? queue[0] : undefined;
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return (state?.candidates ?? []).filter((candidate) => {
      if (decisionOf(state!, candidate) !== tab) return false;
      return !normalized || `${candidate.name} ${candidate.username}`.toLocaleLowerCase().includes(normalized);
    });
  }, [state, tab, query]);

  const mutate = async (work: () => Promise<void>, success?: string) => {
    setBusy(true);
    try {
      await work();
      await refresh();
      if (success) setNotice(success);
    } catch (error) {
      setNotice(errorMessage(error, "本地保存失败"));
    } finally {
      setBusy(false);
    }
  };

  const importProgress = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setBusy(true);
    try {
      if (file.size > 20_000_000) throw new Error("导入文件不能超过 20 MB");
      const payload: unknown = JSON.parse(await file.text());
      if (!payload || typeof payload !== "object" || (payload as { format?: unknown }).format !== "xunfollow-progress-v1") {
        throw new Error("请选择由 XUnFollow 导出的进度 JSON 文件");
      }
      const decisions = (payload as { decisions?: unknown }).decisions;
      if (!decisions || typeof decisions !== "object" || Array.isArray(decisions)) throw new Error("导入文件没有有效的进度记录");
      const accepted = await api.importDecisions(decisions as Bootstrap["decisions"]);
      await refresh();
      setNotice(`已导入 ${accepted} 条与当前名单匹配的进度`);
    } catch (error) {
      setNotice(errorMessage(error, "导入失败"));
    } finally {
      setBusy(false);
    }
  };

  if (!state) return <main className="loading">正在打开本地数据库…</main>;

  const captured = state.summary?.capturedAt ? new Date(state.summary.capturedAt).toLocaleString("zh-CN") : "尚未同步";
  const history = state.history ?? [];

  return (
    <main className="shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">LOCAL-FIRST · 你的数据只在本机</p>
          <h1>XUnFollow</h1>
          <p className="subtle">
            {state.account ? `@${state.account.username} · 名单同步于 ${captured}` : "尚未连接 X 账号"}
          </p>
        </div>
        <div className="privacy-pill"><i />人工确认，不自动取关</div>
      </header>

      <section className="stats" aria-label="处理统计">
        <Stat value={number.format(counts.pending)} label="待处理" />
        <Stat value={number.format(counts.unfollowed)} label="已取关" />
        <Stat value={number.format(counts.keep)} label="保留关注" />
        <Stat value={number.format(state.candidates.length)} label="名单总数" />
      </section>

      {!state.account || showSetup ? (
        <Setup
          configured={state.apiKeyConfigured}
          pendingScan={state.pendingScan}
          onClose={() => setShowSetup(false)}
          onSaved={() => { setShowSetup(false); void refresh(); }}
          setNotice={setNotice}
        />
      ) : (
        <section className="task-panel">
          <div className="task-head">
            <div>
              <p className="eyebrow">今日任务</p>
              <h2>{completedToday} / {state.dailyGoal}</h2>
            </div>
            <div className="task-links">
              <button disabled={busy} onClick={() => void mutate(api.undoLast, "已撤销上一步")}>撤销上一步</button>
              <button disabled={busy} onClick={() => setShowSetup(true)}>同步设置</button>
            </div>
          </div>
          <div className="progress-track"><div style={{ width: `${Math.min(100, completedToday / state.dailyGoal * 100)}%` }} /></div>

          {current ? (
            <CandidateCard
              candidate={current}
              busy={busy}
              onOpen={() => void api.openExternalProfile(current.xUrl)}
              onDecision={(status) => void mutate(() => api.recordDecision(current.stableXId, status), status === "unfollowed" ? "已记录为取关" : "进度已保存")}
            />
          ) : (
            <div className="empty-state">
              {queue.length ? <>
                <p>当前 {state.dailyGoal} 个任务已经完成，可以马上继续下一批。</p>
                <button className="next-batch" disabled={busy} onClick={() => void mutate(api.continueBatch, `已生成下一批 ${state.batchSize} 人`)}>
                  继续下一批 {state.batchSize} 人
                </button>
              </> : <p>全部账号都已经处理完了。</p>}
            </div>
          )}
        </section>
      )}

      <section className="list-panel">
        <div className="list-tools">
          <div className="tabs" role="tablist">
            {(Object.keys(labels) as Tab[]).map((key) => <button key={key} className={tab === key ? "active" : ""} onClick={() => setTab(key)}>{labels[key]}</button>)}
          </div>
          <div className="list-actions"><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名字或 @用户名" aria-label="搜索" /><button disabled={busy} onClick={() => importInput.current?.click()}>导入进度</button><button onClick={() => downloadProgress(state)}>导出进度</button><input ref={importInput} hidden type="file" accept="application/json,.json" onChange={(event) => void importProgress(event)} /></div>
        </div>
        <p className="list-summary">共 {number.format(filtered.length)} 人，当前显示 {Math.min(filtered.length, 50)} 人</p>
        <div className="account-list">
          {filtered.slice(0, 50).map((candidate) => <AccountRow key={candidate.stableXId} candidate={candidate} onOpen={() => void api.openExternalProfile(candidate.xUrl)} />)}
        </div>
      </section>
      <section className="history-panel">
        <div><p className="eyebrow">处理历史</p><h2>最近记录</h2></div>
        {history.length ? <div className="history-list">{history.slice(0, 12).map((entry) => <article key={entry.stableXId}><div><strong>{entry.name || `@${entry.username}`}</strong><span>@{entry.username} · {entry.actionDay}</span></div><HistoryStatus status={entry.status} /></article>)}</div> : <p className="subtle">完成一次人工处理后，记录会只保存在这台电脑。</p>}
      </section>
      {notice && <div className="toast" onAnimationEnd={() => setNotice("")}>{notice}</div>}
    </main>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return <article><strong>{value}</strong><span>{label}</span></article>;
}

function CandidateCard({ candidate, busy, onOpen, onDecision }: {
  candidate: Candidate; busy: boolean; onOpen: () => void; onDecision: (status: Exclude<DecisionStatus, "pending">) => void;
}) {
  return <>
    <div className="candidate-card">
      <Avatar candidate={candidate} large />
      <div className="identity"><h3>{candidate.name || `@${candidate.username}`}</h3><p>@{candidate.username}</p><div className="metrics"><span><b>{number.format(candidate.followersCount)}</b> 关注者</span><span><b>{number.format(candidate.followingCount)}</b> 正在关注</span></div></div>
      <button className="open-x" onClick={onOpen}>在 X 中查看 ↗</button>
    </div>
    <div className="actions">
      <button className="unfollow" disabled={busy} onClick={() => onDecision("unfollowed")}>已取关，下一位</button>
      <button disabled={busy} onClick={() => onDecision("keep")}>保留关注</button>
      <button disabled={busy} onClick={() => onDecision("later")}>稍后处理</button>
      <button disabled={busy} onClick={() => onDecision("changed")}>关系已变化</button>
    </div>
  </>;
}

function AccountRow({ candidate, onOpen }: { candidate: Candidate; onOpen: () => void }) {
  return <article className="account-row"><Avatar candidate={candidate} /><div className="who"><strong>{candidate.name || `@${candidate.username}`}</strong><span>@{candidate.username}</span></div><button onClick={onOpen}>打开 ↗</button></article>;
}

function HistoryStatus({ status }: { status: Exclude<DecisionStatus, "pending"> }) {
  const statusLabels: Record<Exclude<DecisionStatus, "pending">, string> = { unfollowed: "已取关", keep: "保留", later: "稍后", changed: "关系变化" };
  return <span className={`history-status ${status}`}>{statusLabels[status]}</span>;
}

function Avatar({ candidate, large = false }: { candidate: Candidate; large?: boolean }) {
  if (candidate.profileImageUrl) return <img className={large ? "avatar large" : "avatar"} src={candidate.profileImageUrl} alt="" />;
  return <div className={large ? "avatar fallback large" : "avatar fallback"}>{candidate.name.slice(0, 1).toLocaleUpperCase()}</div>;
}

function downloadProgress(state: Bootstrap) {
  const exportData = {
    format: "xunfollow-progress-v1",
    exportedAt: new Date().toISOString(),
    account: state.account,
    summary: state.summary,
    decisions: state.decisions,
  };
  const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `xunfollow-progress-${new Date().toISOString().slice(0, 10)}.json`;
  link.click();
  URL.revokeObjectURL(url);
}

function Setup({ configured, pendingScan, onClose, onSaved, setNotice }: { configured: boolean; pendingScan?: Bootstrap["pendingScan"]; onClose: () => void; onSaved: () => void; setNotice: (message: string) => void }) {
  // Keep the secret field uncontrolled. WebKit can autofill a password field
  // without firing React's onChange, so React state may be empty even while a
  // visible value exists in the native Tauri window.
  const apiKeyInput = useRef<HTMLInputElement>(null);
  const [handle, setHandle] = useState(pendingScan?.handle ?? "");
  const [cap, setCap] = useState(pendingScan?.hardCapUsd ?? "0.50");
  const [estimate, setEstimate] = useState<{ text: string; handle: string; cap: string }>();
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const apiKeyToSave = apiKeyInput.current?.value.trim() ?? "";
    setBusy(true);
    try {
      if (apiKeyToSave) await api.saveApiKey(apiKeyToSave);
      const cost = await api.estimateCost(handle, cap);
      setEstimate({ handle, cap, text: `预计费用约 $${money.format(Number(cost.estimatedUsd))}；硬上限 $${cost.hardCapUsd}。${cost.assumption}` });
    } catch (error) { setNotice(errorMessage(error, "无法验证设置")); }
    finally { setBusy(false); }
  };
  const scan = async () => {
    if (!estimate || estimate.handle !== handle || estimate.cap !== cap) {
      setNotice("账号或费用上限已变化，请先重新估算费用。");
      return;
    }
    setBusy(true);
    try { await api.startScan(handle, cap); onSaved(); }
    catch (error) { setNotice(errorMessage(error, "同步没有开始")); }
    finally { setBusy(false); }
  };
  const resume = async () => {
    if (!pendingScan) return;
    setBusy(true);
    try { await api.resumeScanOnce(handle, cap); onSaved(); }
    catch (error) { setNotice(errorMessage(error, "恢复同步失败")); }
    finally { setBusy(false); }
  };
  return <section className="setup-panel">
    <div><p className="eyebrow">首次设置</p><h2>用自己的 API Key 建立本地名单</h2><p>Key 仅保存在本机安全存储；XUnFollow 不要求 X 密码、Cookie 或登录。</p></div>
    <form onSubmit={(event) => void submit(event)}>
      <label>TwitterAPI.io API Key<input ref={apiKeyInput} name="apiKey" type="password" autoComplete="off" placeholder={configured ? "已保存；留空即可复用" : "粘贴你自己的 API Key"} required={!configured} /></label>
      <label>X 用户名<input value={handle} onChange={(event) => { setHandle(event.target.value.replace(/^@/, "")); setEstimate(undefined); }} placeholder="例如 guoqingfeng6" required /></label>
      <label>本次费用硬上限（美元）<input inputMode="decimal" value={cap} onChange={(event) => { setCap(event.target.value); setEstimate(undefined); }} required /></label>
      <div className="setup-actions"><button className="outline" type="button" onClick={onClose}>取消</button><button className="open-x" disabled={busy} type="submit">保存并估算费用</button></div>
    </form>
    {estimate && <div className="estimate"><p>{estimate.text}</p><button className="next-batch" disabled={busy} onClick={() => void scan()}>确认上限，开始只读同步</button></div>}
    {pendingScan && <div className="estimate recovery"><p>{pendingScan.needsExplicitRetry ? "检测到上次请求结果或计费不确定。恢复时会把该页费用按保守值计入预算，并且仅允许这一次由你手动批准的重试。" : "检测到未完成的本地同步检查点。使用同一账号与费用上限即可从上次已保存的位置继续。"}</p><button className="next-batch" disabled={busy} onClick={() => void resume()}>{pendingScan.needsExplicitRetry ? "我了解费用风险，单次重试并继续" : "继续未完成的同步"}</button></div>}
  </section>;
}

export default App;
