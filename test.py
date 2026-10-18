#!/usr/bin/env python3
import sys, os, time
from PyQt6.QtWidgets import QApplication, QLabel
app = QApplication(sys.argv)
w = QLabel('test')
w.show()
time.sleep(1)
print('ok')
