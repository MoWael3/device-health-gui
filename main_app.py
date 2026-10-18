#!/usr/bin/env python3
"""Entry point: python main_app.py (the app lives in src/device_health_gui/)."""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "src"))

from device_health_gui import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
