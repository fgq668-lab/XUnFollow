export function beijingInput(iso?:string):string {
  if(!iso) return "";
  const d=new Date(iso); if(!Number.isFinite(d.getTime())) return "";
  return new Date(d.getTime()+8*3600_000).toISOString().slice(0,16);
}
export function fromBeijingInput(value:string):string|undefined {
  if(!value) return undefined;
  if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value)) throw new Error("北京时间格式无效");
  const d=new Date(`${value}:00+08:00`); if(!Number.isFinite(d.getTime())) throw new Error("北京时间格式无效");
  return d.toISOString();
}
export const beijingDisplay=(value?:string)=>value?new Intl.DateTimeFormat("zh-CN",{timeZone:"Asia/Shanghai",month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hourCycle:"h23"}).format(new Date(value)):"待排期";
