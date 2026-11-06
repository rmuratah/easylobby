import { readFileSync, writeFileSync } from 'node:fs';
const file = new URL('../src/theme/tokens.css', import.meta.url);
const source = readFileSync(new URL('../src/theme/themeColors.ts', import.meta.url), 'utf8');
const colors = JSON.parse(source.replace(/^export const themeColors = /, '').replace(/ as const;\s*$/, ''));
function luminance(hex) {
  const [r,g,b] = hex.match(/[\da-f]{2}/gi).map(x => { const c=parseInt(x,16)/255; return c<=0.03928?c/12.92:((c+0.055)/1.055)**2.4; });
  return .2126*r+.7152*g+.0722*b;
}
function ratio(a,b) { const [lo,hi]=[luminance(a),luminance(b)].sort((a,b)=>a-b); return (hi+.05)/(lo+.05); }
function readable(fg, backgrounds, ink, minimum) {
  const rgb = hex => hex.match(/[\da-f]{2}/gi).map(x=>parseInt(x,16));
  const a=rgb(fg), b=rgb(ink);
  for(let i=0;i<=100;i++) {
    const mixed='#'+a.map((c,j)=>Math.round(c+(b[j]-c)*i/100).toString(16).padStart(2,'0')).join('');
    if(backgrounds.every(bg=>ratio(mixed,bg)>=minimum)) return mixed;
  }
  return ink;
}
let output='/* Gerado por node ui/scripts/generate-palette.mjs. Fonte: themeColors.ts. */\n';
for(const [name,p] of Object.entries(colors)) {
  const backgrounds=[p.background,p.card,p.secondary];
  const raw={};
  function flatten(value, prefix='palette') {
    for(const [key,item] of Object.entries(value)) {
      const path=prefix+'-'+key.replace(/[A-Z]/g,c=>'-'+c.toLowerCase());
      if(typeof item==='object') flatten(item,path); else raw[path]=item;
    }
  }
  flatten(p);
  const fg=(color,min=4.5)=>readable(color,backgrounds,p.foreground,min);
  const tokens={...raw,bg:p.background,surface:p.card,raised:p.secondary,ink:p.foreground,muted:fg(p.mutedForeground),line:p.border,control:fg(p.border,3),accent:fg(p.primary.default,4.5),'on-accent':p.primary.foreground,'accent-soft':p.primary.soft,'accent-ink':readable(p.primary.default,[p.primary.soft],p.foreground,4.5),danger:fg(p.destructive),warn:fg(p.accent.text),'status-online':fg(p.status.online,3),'status-game':fg(p.status.ingame,3),'status-away':fg(p.status.away,3),'status-offline':fg(p.status.offline,3),overlay:name==='light'?'rgb(23 32 43 / 0.32)':'rgb(0 0 0 / 0.55)'};
  if(ratio(tokens['on-accent'],tokens.accent)<4.5) tokens['on-accent']=readable(p.primary.foreground,[tokens.accent],name==='dark'?p.background:p.foreground,4.5);
  const theme=name==='light'?'claro':'escuro';
  output+=(name==='light'?':root,\n':'')+`[data-theme="${theme}"] {\n  color-scheme: ${name};\n`+Object.entries(tokens).map(([k,v])=>`  --el-${k}: ${v};`).join('\n')+'\n}\n\n';
}
const old=readFileSync(file,'utf8');
writeFileSync(file,output+old.slice(old.indexOf('@theme inline')));
