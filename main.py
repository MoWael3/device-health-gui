#!/usr/bin/env python3
import os
import time
import webview
import threading
from device_health_gui.opencode import analyze

last_net = None

def get_cpu():
    try:
        with open('/proc/stat') as f:
            parts = list(map(int, f.readline().split()[1:8]))
        return parts[0]+parts[1]+parts[2], parts[3]
    except Exception:
        return 0,0

def cpu_percent():
    global last_cpu
    t,i = get_cpu()
    lt,li = last_cpu
    last_cpu = (t,i)
    d=t-lt; di=i-li
    return round(100.0*(d-di)/d,1) if d>0 else 0.0
last_cpu = get_cpu()

def mem():
    try:
        m={}
        with open('/proc/meminfo') as f:
            for line in f:
                if ':' in line:
                    k,v = line.split(':',1)
                    m[k.strip()] = int(v.split()[0])*1024
        total=m['MemTotal']; avail=m.get('MemAvailable',m.get('MemFree',0))
        used=total-avail
        return total//1048576, used//1048576, round(used/total*100,1)
    except Exception:
        return 0,0,0

def disk():
    try:
        s=os.statvfs('/')
        total=s.f_blocks*s.f_frsize; free=s.f_bavail*s.f_frsize
        used=total-free
        return total//1073741824, used//1073741824, round(used/total*100,1)
    except Exception:
        return 0,0,0

def temp():
    try:
        temps=[]
        for root,dirs,files in os.walk('/sys/class/thermal'):
            if 'temp' in files:
                with open(os.path.join(root,'temp')) as f:
                    v=int(f.read().strip())
                    if v>0: temps.append(v/1000.0)
        return round(max(temps),1) if temps else 0.0
    except Exception:
        return 0.0

def uptime():
    try:
        with open('/proc/uptime') as f:
            return int(float(f.read().split()[0]))
    except Exception:
        return 0

def net():
    global last_net
    try:
        rx=tx=0
        with open('/proc/net/dev') as f:
            for line in f:
                if ':' in line:
                    parts=line.split(':',1)[1].split()
                    rx+=int(parts[0]); tx+=int(parts[8])
        if last_net is None:
            last_net=(rx,tx,time.time())
            return 0,0
        lrx,ltx,lt = last_net
        dt=time.time()-lt
        last_net=(rx,tx,time.time())
        if dt<=0: return 0,0
        return round((rx-lrx)/dt/1024,1), round((tx-ltx)/dt/1024,1)
    except Exception:
        return 0,0

class Api:
    last = None

    def analyze(self):
        return analyze(self.last) if self.last else 'No stats yet.'

    def get_stats(self):
        tmem,umem,pmem = mem()
        td,ud,pd = disk()
        rxt,txt = net()
        up = uptime()
        self.last = {
            'cpu': cpu_percent(),
            'mem_total': tmem,'mem_used': umem,'mem_pct': pmem,
            'disk_total': td,'disk_used': ud,'disk_pct': pd,
            'temp': temp(),
            'uptime': up,
            'net_rx': rxt,'net_tx': txt
        }
        return self.last

def main():
    api = Api()
    webview.create_window('Device Health Dashboard', html=HTML, js_api=api, width=800, height=500, resizable=True)
    webview.start()

HTML = '''<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
*{margin:0;padding:0;box-sizing:border-box;font-family:monospace}
body{background:#0f1026;color:#e6e6e6;padding:20px}
h1{font-size:18px;margin-bottom:16px}
.grid{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.card{background:#1a1b2e;border:1px solid #2a2b3e;border-radius:8px;padding:12px}
.label{color:#9ca3af;font-size:12px}
.value{font-size:24px;margin-top:4px}
.bar{height:6px;background:#2a2b3e;border-radius:3px;margin-top:8px;overflow:hidden}
.fill{height:100%;background:linear-gradient(90deg,#22d3ee,#22c55e)}
.row{display:flex;gap:12px;margin-top:12px}
button{margin-top:12px;background:#22d3ee;color:#0f1026;border:0;border-radius:6px;padding:8px 14px;cursor:pointer}
button:disabled{opacity:.5}
#ai{white-space:pre-wrap;margin-top:12px;font-size:13px}
</style>
</head>
<body>
<h1>Device Health Dashboard</h1>
<div class="grid">
  <div class="card"><div class="label">CPU</div><div class="value" id="cpu">0.0%</div><div class="bar"><div class="fill" id="cpu-fill" style="width:0%"></div></div></div>
  <div class="card"><div class="label">Memory</div><div class="value" id="mem">0/0 MB (0%)</div><div class="bar"><div class="fill" id="mem-fill" style="width:0%"></div></div></div>
  <div class="card"><div class="label">Disk (/)</div><div class="value" id="disk">0/0 GB (0%)</div><div class="bar"><div class="fill" id="disk-fill" style="width:0%"></div></div></div>
  <div class="card"><div class="label">Temperature</div><div class="value" id="temp">0.0 °C</div></div>
  <div class="card"><div class="label">Uptime</div><div class="value" id="uptime">0s</div></div>
  <div class="card"><div class="label">Network</div><div class="value" id="net">↓ 0.0 KB/s ↑ 0.0 KB/s</div></div>
</div>
<button id="ai-btn" onclick="runAI()">Analyze with opencode</button>
<div id="ai"></div>
<script>
function fmtU(s){const h=Math.floor(s/3600),m=Math.floor((s%3600)/60),sec=s%60;return h+":"+String(m).padStart(2,'0')+":"+String(sec).padStart(2,'0');}
async function runAI(){
  const b=document.getElementById('ai-btn'),o=document.getElementById('ai');
  b.disabled=true;o.textContent='Asking opencode...';
  try{o.textContent=await pywebview.api.analyze();}catch(e){o.textContent='Error: '+e;}
  b.disabled=false;
}
async function tick(){
  try{
    const d=await pywebview.api.get_stats();
    document.getElementById('cpu').textContent=d.cpu.toFixed(1)+"%";
    document.getElementById('cpu-fill').style.width=d.cpu+"%";
    document.getElementById('mem').textContent=d.mem_used+"/"+d.mem_total+" MB ("+d.mem_pct.toFixed(1)+"%)";
    document.getElementById('mem-fill').style.width=d.mem_pct+"%";
    document.getElementById('disk').textContent=d.disk_used+"/"+d.disk_total+" GB ("+d.disk_pct.toFixed(1)+"%)";
    document.getElementById('disk-fill').style.width=d.disk_pct+"%";
    document.getElementById('temp').textContent=d.temp.toFixed(1)+" °C";
    document.getElementById('uptime').textContent=fmtU(d.uptime);
    document.getElementById('net').textContent="↓ "+d.net_rx.toFixed(1)+" KB/s ↑ "+d.net_tx.toFixed(1)+" KB/s";
  }catch(e){}
  setTimeout(tick,1000);
}
tick();
</script>
</body>
</html>'''
if __name__=='__main__':
    main()
