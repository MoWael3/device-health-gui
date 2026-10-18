#!/usr/bin/env python3
import os,time,sys,select,threading
from device_health_gui.opencode import analyze

ai_text=''; ai_busy=False; stats=None

last_cpu = None; last_net = None
def get_cpu():
    try:
        with open('/proc/stat') as f:
            p=list(map(int,f.readline().split()[1:8]))
        return p[0]+p[1]+p[2],p[3]
    except: return 0,0
def cpu_p():
    global last_cpu
    t,i=get_cpu()
    if last_cpu is None: last_cpu=(t,i); return 0.0
    lt,li=last_cpu; last_cpu=(t,i); d=t-lt; di=i-li
    return round(100.0*(d-di)/d,1) if d>0 else 0.0
def mem():
    try:
        m={}
        with open('/proc/meminfo') as f:
            for line in f:
                if ':' in line: k,v=line.split(':',1); m[k.strip()]=int(v.split()[0])*1024
        total=m['MemTotal']; avail=m.get('MemAvailable',m.get('MemFree',0)); used=total-avail
        return total//1048576,used//1048576,round(used/total*100,1)
    except: return 0,0,0
def disk():
    try:
        s=os.statvfs('/')
        total=s.f_blocks*s.f_frsize; free=s.f_bavail*s.f_frsize; used=total-free
        return total//1073741824,used//1073741824,round(used/total*100,1)
    except: return 0,0,0
def temp():
    try:
        tt=[]
        for root,dirs,files in os.walk('/sys/class/thermal'):
            if 'temp' in files:
                with open(os.path.join(root,'temp')) as f: v=int(f.read().strip())
                if v>0: tt.append(v/1000.0)
        return round(max(tt),1) if tt else 0.0
    except: return 0.0
def up():
    try:
        with open('/proc/uptime') as f: s=float(f.read().split()[0]); h=int(s//3600); m=int((s%3600)//60); se=int(s%60); return f"{h}:{m:02d}:{se:02d}"
    except: return "0:00:00"
def net():
    global last_net
    try:
        rx=tx=0
        with open('/proc/net/dev') as f:
            for line in f:
                if ':' in line: p=line.split(':',1)[1].split(); rx+=int(p[0]); tx+=int(p[8])
        if last_net is None: last_net=(rx,tx,time.time()); return 0,0
        lrx,ltx,lt=last_net; dt=time.time()-lt; last_net=(rx,tx,time.time())
        return (round((rx-lrx)/dt/1024,1), round((tx-ltx)/dt/1024,1)) if dt>0 else (0,0)
    except: return 0,0

def run_ai():
    global ai_text,ai_busy
    ai_busy=True; ai_text='Asking opencode...'
    def work(s):
        global ai_text,ai_busy
        ai_text=analyze(s); ai_busy=False
    threading.Thread(target=work,args=(stats,),daemon=True).start()

def draw():
    global stats
    tmem,umem,pmem=mem(); td,ud,pd=disk(); rxt,txt=net()
    c=cpu_p(); tp=temp(); u=up(); h,m,se=map(int,u.split(':'))
    stats={'cpu':c,'mem_total':tmem,'mem_used':umem,'mem_pct':pmem,'disk_total':td,'disk_used':ud,'disk_pct':pd,'temp':tp,'uptime':h*3600+m*60+se,'net_rx':rxt,'net_tx':txt}
    print("\033[H\033[J",end="")
    print("DEVICE HEALTH DASHBOARD")
    print("-----------------------")
    print(f"CPU        {c:5.1f}%")
    print(f"Memory     {umem:4d}/{tmem:4d} MB ({pmem:4.1f}%)")
    print(f"Disk (/)   {ud:4d}/{td:4d} GB ({pd:4.1f}%)")
    print(f"Temp       {tp:5.1f} °C")
    print(f"Uptime     {u}")
    print(f"Network    ↓ {rxt:5.1f} KB/s ↑ {txt:5.1f} KB/s")
    print("-----------------------")
    if ai_text: print(ai_text); print("-----------------------")
    print("Enter: analyze with opencode | Ctrl+C: exit")

def main():
    try:
        while True:
            draw()
            if select.select([sys.stdin],[],[],1)[0]:
                sys.stdin.readline()
                if not ai_busy: run_ai()
    except KeyboardInterrupt:
        pass
    except Exception as e:
        pass

if __name__=='__main__':
    main()
