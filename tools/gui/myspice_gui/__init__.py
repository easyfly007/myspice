"""
MySpice GUI - Graphical user interface for MySpice circuit simulator.

This package provides a PySide6-based GUI for:
- Editing SPICE netlists
- Running simulations (OP, DC, TRAN, AC)
- Viewing waveforms and results
"""

__version__ = "0.1.0"
__author__ = "MySpice Team"

from .client import MySpiceClient
from .main_window import MainWindow

__all__ = ["MySpiceClient", "MainWindow", "__version__"]
