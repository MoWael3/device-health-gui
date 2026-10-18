#!/usr/bin/env python3
import os, time, threading, tkinter as tk
from device_health_gui.opencode import analyze
from tkinter import ttk

class Dash(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("Device Health Dashboard")
        self.geometry("620x380")
        self.configure(bg='#0f1026')
        self.last_cpu=self.get_cpu(); self.last_net=None
        self.style=ttk.Style(self)
        self.style.theme_use('clam')
        self.style.configure('TFrame', background='#1a1b2e')
        self.style.configure('TLabel', background='#1a1b2e', foreground='#e6e6e6')
        self.style.configure('Head.TLabel', background='#0f1026', foreground='#e6e6e6', font=('monospace',14,'bold'))
        self.style.configure('Key.TLabel', background='#1a1b2e', foreground='#9ca3af', font=('monospace',9))
        self.style.configure('Val.TLabel', background='#1a1b2e', foreground='#e6e6e6', font=('monospace',18))
        main=ttk.Frame(self,padding=15); main.pack(fill=tk.BOTH,expand=True)
        ttk.Label(main,text="Device Health Dashboard",style='Head.TLabel').pack(anchor='w',pady=(0,12))
        grid=ttk.Frame(main); grid.pack(fill=tk.BOTH,expand=True)
        self.cpu=self.mk(grid,'CPU','0.0%'); self.mem=self.mk(grid,'Memory','0/0 MB (0%)'); self.disk=self.mk(grid,'Disk (/)','0/0 GB (0%)'); self.temp=self.mk(grid,'Temperature','0.0 °C'); self.up=self.mk(grid,'Uptime','0:00:00'); self.net=self.mk(grid,'Network','↓ 0.0 KB/s ↑ 0.0 KB/s')
        for i,w in enumerate([self.cpu,self.mem,self.disk,self.temp,self.up,self.net]):
            w.grid(row=i//2,column=i%2,sticky='nsew',padx=6,pady=6)
        grid.columnconfigure(0,weight=1); grid.columnconfigure(1,weight=1)
        for r in range(3): grid.rowconfigure(r,weight=1)
        self.stats=None; self.ai_result=None
        self.btn=ttk.Button(main,text="Analyze with opencode",command=self.run_ai); self.btn.pack(anchor='w',pady=(8,0))
        self.ai=ttk.Label(main,text="",wraplength=580,justify='left'); self.ai.pack(anchor='w',pady=(8,0))
        self.tick()
    def mk(self,parent,k,v):
        f=ttk.Frame(parent,padding=10)
        ttk.Label(f,text=k,style='Key.TLabel').pack(anchor='w')
        ttk.Label(f,text=v,style='Val.TLabel').pack(anchor='w',pady=(4,0))
        return f
    def get_cpu(self):
        try:
            with open('/proc/stat') as fh: p=list(map(int,fh.readline().split()[1:8]))
            return p[0]+p[1]+p[2],p[3]
        except: return 0,0
    def cpu_p(self):
        t,i=self.get_cpu(); lt,li=self.last_cpu; self.last_cpu=(t,i)
        d=t-lt; di=i-li; return round(100.0*(d-di)/d,1) if d>0 else 0.0
    def read_mem(self):
        try:
            m={}
            with open('/proc/meminfo') as f:
                for line in f:
                    if ':' in line: kk,vv=line.split(':',1); m[kk.strip()]=int(vv.split()[0])*1024
            total=m['MemTotal']; avail=m.get('MemAvailable',m.get('MemFree',0)); used=total-avail
            return total//1048576,used//1048576,round(used/total*100,1)
        except: return 0,0,0
    def read_disk(self):
        try:
            s=os.statvfs('/')
            total=s.f_blocks*s.f_frsize; free=s.f_bavail*s.f_frsize; used=total-free
            return total//1073741824,used//1073741824,round(used/total*100,1)
        except: return 0,0,0
    def read_temp(self):
        try:
            tt=[]
            for root,dirs,files in os.walk('/sys/class/thermal'):
                if 'temp' in files:
                    with open(os.path.join(root,'temp')) as f: v=int(f.read().strip())
                    if v>0: tt.append(v/1000.0)
            return round(max(tt),1) if tt else 0.0
        except: return 0.0
    def read_up(self):
        try:
            with open('/proc/uptime') as f: s=float(f.read().split()[0]); h=int(s//3600); m=int((s%3600)//60); se=int(s%60); return f"{h}:{m:02d}:{se:02d}"
        except: return "0:00:00"
    def read_net(self):
        try:
            rx=tx=0
            with open('/proc/net/dev') as f:
                for line in f:
                    if ':' in line: p=line.split(':',1)[1].split(); rx+=int(p[0]); tx+=int(p[8])
            if self.last_net is None: self.last_net=(rx,tx,time.time()); return 0,0
            lrx,ltx,lt=self.last_net; dt=time.time()-lt; self.last_net=(rx,tx,time.time())
            return (round((rx-lrx)/dt/1024,1), round((tx-ltx)/dt/1024,1)) if dt>0 else (0,0)
        except: return 0,0
    def run_ai(self):
        if not self.stats: return
        self.btn.state(['disabled']); self.ai.config(text="Asking opencode..."); self.ai_result=None
        threading.Thread(target=lambda s=self.stats: setattr(self,'ai_result',analyze(s)),daemon=True).start()
        self.after(300,self.poll_ai)
    def poll_ai(self):
        if self.ai_result is None: self.after(300,self.poll_ai); return
        self.ai.config(text=self.ai_result); self.btn.state(['!disabled'])
    def tick(self):
        for child in self.winfo_children(): pass
        # update labels
        cpuv=self.cpu_p()
        self.cpu.winfo_children()[1].config(text=f"{cpuv:.1f}%")
        tmem,umem,pmem=self.read_mem(); self.mem.winfo_children()[1].config(text=f"{umem}/{tmem} MB ({pmem:.1f}%)")
        td,ud,pd=self.read_disk(); self.disk.winfo_children()[1].config(text=f"{ud}/{td} GB ({pd:.1f}%)")
        self.temp.winfo_children()[1].config(text=f"{self.read_temp():.1f} °C")
        up=self.read_up(); self.up.winfo_children()[1].config(text=up)
        rxt,txt=self.read_net(); self.net.winfo_children()[1].config(text=f"↓ {rxt:.1f} KB/s ↑ {txt:.1f} KB/s")
        tp=self.read_temp(); h,m,se=map(int,up.split(':'))
        self.stats={'cpu':cpuv,'mem_total':tmem,'mem_used':umem,'mem_pct':pmem,'disk_total':td,'disk_used':ud,'disk_pct':pd,'temp':tp,'uptime':h*3600+m*60+se,'net_rx':rxt,'net_tx':txt}
        self.after(1000,self.tick)

if __name__=='__main__':
    app=Dash(); app.mainloop()
