#!/usr/bin/env python3
import sys, os, time
from PyQt6.QtWidgets import QApplication, QMainWindow, QWidget, QGridLayout, QLabel, QFrame, QVBoxLayout
from PyQt6.QtCore import Qt, QTimer

class Dash(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Device Health Dashboard")
        self.resize(780,480)
        self.last_cpu=self.get_cpu(); self.last_net=None
        c=QWidget(); self.setCentralWidget(c); l=QGridLayout(c)
        self.cpu=self.lbl("0.0%"); self.m=self.lbl("0/0 MB (0%)"); self.d=self.lbl("0/0 GB (0%)"); self.t=self.lbl("0.0 °C"); self.u=self.lbl("0:00:00"); self.n=self.lbl("↓ 0.0 KB/s ↑ 0.0 KB/s")
        for i,(k,v) in enumerate([("CPU",self.cpu),("Memory",self.m),("Disk (/)",self.d),("Temperature",self.t),("Uptime",self.u),("Network",self.n)]):
            l.addWidget(self.card(k,v), i//2, i%2)
        self.timer=QTimer(self); self.timer.timeout.connect(self.update); self.timer.start(1000); self.update()
    def lbl(self,text):
        x=QLabel(text); x.setStyleSheet("font-size:24px;color:#e6e6e6;"); return x
    def card(self,k,v):
        f=QFrame(); f.setStyleSheet("background:#1a1b2e;border:1px solid #2a2b3e;border-radius:8px;")
        vb=QVBoxLayout(f); lab=QLabel(k); lab.setStyleSheet("color:#9ca3af;font-size:12px;"); vb.addWidget(lab); vb.addWidget(v); return f
    def get_cpu(self):
        try:
            with open('/proc/stat') as fh: p=list(map(int,fh.readline().split()[1:8])); return p[0]+p[1]+p[2],p[3]
        except: return 0,0
    def cpu_p(self):
        t,i=self.get_cpu(); lt,li=self.last_cpu; self.last_cpu=(t,i); d=t-lt; di=i-li
        return round(100.0*(d-di)/d,1) if d>0 else 0.0
    def mem(self):
        try:
            m={}
            with open('/proc/meminfo') as f:
                for line in f:
                    if ':' in line: kk,vv=line.split(':',1); m[kk.strip()]=int(vv.split()[0])*1024
            total=m['MemTotal']; avail=m.get('MemAvailable',m.get('MemFree',0)); used=total-avail
            return total//1048576,used//1048576,round(used/total*100,1)
        except: return 0,0,0
    def disk(self):
        try:
            s=os.statvfs('/'); total=s.f_blocks*s.f_frsize; free=s.f_bavail*s.f_frsize; used=total-free
            return total//1073741824,used//1073741824,round(used/total*100,1)
        except: return 0,0,0
    def temp(self):
        try:
            tt=[]
            for root,dirs,files in os.walk('/sys/class/thermal'):
                if 'temp' in files:
                    with open(os.path.join(root,'temp')) as f: v=int(f.read().strip())
                    if v>0: tt.append(v/1000.0)
            return round(max(tt),1) if tt else 0.0
        except: return 0.0
    def up(self):
        try:
            with open('/proc/uptime') as f: s=float(f.read().split()[0]); h=int(s//3600); m=int((s%3600)//60); se=int(s%60); return f"{h}:{m:02d}:{se:02d}"
        except: return "0:00:00"
    def net(self):
        try:
            rx=tx=0
            with open('/proc/net/dev') as f:
                for line in f:
                    if ':' in line: p=line.split(':',1)[1].split(); rx+=int(p[0]); tx+=int(p[8])
            if self.last_net is None: self.last_net=(rx,tx,time.time()); return 0,0
            lrx,ltx,lt=self.last_net; dt=time.time()-lt; self.last_net=(rx,tx,time.time())
            return (round((rx-lrx)/dt/1024,1),round((tx-ltx)/dt/1024,1)) if dt>0 else (0,0)
        except: return 0,0
    def update(self):
        self.cpu.setText(f"{self.cpu_p():.1f}%")
        tmem,umem,pmem=self.mem(); self.m.setText(f"{umem}/{tmem} MB ({pmem:.1f}%)")
        td,ud,pd=self.disk(); self.d.setText(f"{ud}/{td} GB ({pd:.1f}%)")
        self.t.setText(f"{self.temp():.1f} °C"); self.u.setText(self.up())
        rxt,txt=self.net(); self.n.setText(f"↓ {rxt:.1f} KB/s ↑ {txt:.1f} KB/s")

if __name__=='__main__':
    app=QApplication(sys.argv)
    w=Dash(); w.show(); sys.exit(app.exec())
