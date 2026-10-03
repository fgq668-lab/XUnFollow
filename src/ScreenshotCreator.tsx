import { FormEvent, useRef, useState } from "react";
import * as api from "./api";
import type { WorkbenchPreferences } from "./types";
export default function ScreenshotCreator({preferences,disabled,act}:{preferences:WorkbenchPreferences;disabled:boolean;act:(f:()=>Promise<unknown>,text:string)=>Promise<void>}) {
  const [image,setImage]=useState("");const [notice,setNotice]=useState("");const [loading,setLoading]=useState(false);const input=useRef<HTMLInputElement>(null);
  const load=async(file:File)=>{setLoading(true);try{
    if(!["image/png","image/jpeg","image/webp"].includes(file.type)||file.size>10*1024*1024)throw new Error("请选择10MB以内的PNG、JPEG或WebP截图");
    const src=await new Promise<string>((ok,no)=>{const r=new FileReader();r.onload=()=>ok(String(r.result));r.onerror=()=>no(new Error("截图读取失败"));r.readAsDataURL(file);});
    const img=new Image();img.src=src;await img.decode();
    if(img.naturalWidth>8192||img.naturalHeight>8192||img.naturalWidth*img.naturalHeight>20_000_000)throw new Error("截图过大，请先裁剪到需要的内容");
    const scale=Math.min(1,1600/Math.max(img.naturalWidth,img.naturalHeight));const canvas=document.createElement("canvas");canvas.width=Math.max(1,Math.round(img.naturalWidth*scale));canvas.height=Math.max(1,Math.round(img.naturalHeight*scale));const ctx=canvas.getContext("2d");if(!ctx)throw new Error("截图预处理失败");ctx.fillStyle="#ffffff";ctx.fillRect(0,0,canvas.width,canvas.height);ctx.drawImage(img,0,0,canvas.width,canvas.height);const normalized=canvas.toDataURL("image/jpeg",0.92);
    if(normalized.length>2_796_200)throw new Error("截图处理后仍超过2MB，请再裁剪一下");setImage(normalized);setNotice("截图已在本机预览，未发送。请检查并遮挡隐私信息后生成。");
  }catch(e){setNotice(String(e));}finally{setLoading(false);}};
  const submit=(e:FormEvent<HTMLFormElement>)=>{e.preventDefault();const d=new FormData(e.currentTarget);void act(async()=>{await api.startScreenshotRun(image,String(d.get("notes")??""),String(d.get("origin")??""),Number(d.get("count")),String(d.get("cap")));setImage("");},"截图二次创作已开始，草稿将在下方出现，仍需人工审核。");};
  return <details className="creator-form screenshot-creator"><summary>截图二次创作 · 粘贴截图，按人设重新写</summary><p className="wb-note">只用官方 deepseek-flash 图片理解能力，沿用你的人设与创作提示词。不会访问或操作 X 页面，不自动读取剪贴板。请先遮挡私信、账号密钥等敏感内容。</p>
    <div className="screenshot-drop" tabIndex={0} role="group" aria-label="截图粘贴区" onPaste={e=>{const f=Array.from(e.clipboardData.files).find(f=>f.type.startsWith("image/"));if(f){e.preventDefault();void load(f);}else setNotice("剪贴板没有图片，可点击选择截图；Mac 用⌘V，Windows 用Ctrl+V。");}} onDragOver={e=>e.preventDefault()} onDrop={e=>{e.preventDefault();const f=e.dataTransfer.files[0];if(f)void load(f);}}>
      {image?<img src={image} alt="待二次创作截图预览" />:<p>{loading?"正在处理截图…":"点击此区域后按 ⌘V / Ctrl+V 粘贴截图，也可以拖入图片"}</p>}
      <textarea rows={1} aria-label="在这里粘贴截图" placeholder="点这里，然后 ⌘V / Ctrl+V 粘贴图片" value="" onChange={()=>{}} onPaste={e=>{if(!Array.from(e.clipboardData.files).some(f=>f.type.startsWith("image/"))){e.preventDefault();setNotice("剪贴板没有图片，请复制截图图片或选择文件。");}}} />
      <button type="button" disabled={loading} onClick={()=>input.current?.click()}>选择截图文件</button>{image&&<button type="button" onClick={()=>{setImage("");setNotice("已移除本机预览。");}}>移除截图</button>}<input ref={input} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={e=>{const f=e.currentTarget.files?.[0];e.currentTarget.value="";if(f)void load(f);}} />
    </div><p className="wb-note" role="status">{notice}</p><form onSubmit={submit}><div className="wb-grid"><label className="wb-wide">补充想法 / 希望怎么改写<textarea name="notes" rows={3} maxLength={4000} placeholder="例如：结合小工具开发，换个角度写，少点感叹，不照抄截图" /></label><label className="wb-wide">原帖链接（可选，不知道可留空）<input name="origin" type="url" placeholder="https://x.com/用户名/status/帖子ID" /></label><label>生成篇数<input name="count" type="number" min={1} max={3} defaultValue={1} required /></label><label>本次 AI 上限（美元）<input name="cap" inputMode="decimal" defaultValue={preferences.defaultAiCapUsd} required /></label></div>
      <p className="wb-note">截图会在你点击生成后发送给 DeepSeek 官方。图片本机暂存以支持失败后手动继续，完成或结束本轮后清理缓存记录；不会导出原图。来源只使用你填写的真实链接，不让模型编造。识别可能有误，二创不等于获得原作或图片授权。</p><button className="open-x" disabled={!image||disabled||loading} type="submit">确认上限，分析截图并二次创作</button></form></details>;
}
