"""
Bode plot widget for displaying AC analysis results.

Provides dual plots for:
- Magnitude (dB) vs Frequency
- Phase (degrees) vs Frequency

Both with logarithmic frequency axis.
"""

from typing import Optional, Dict, List, Tuple
from dataclasses import dataclass

import numpy as np
from PySide6.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QPushButton,
    QSplitter,
    QFileDialog,
)
from PySide6.QtCore import Qt, Signal

import pyqtgraph as pg


# Default colors
DEFAULT_COLORS = [
    "#1f77b4",  # Blue
    "#ff7f0e",  # Orange
    "#2ca02c",  # Green
    "#d62728",  # Red
    "#9467bd",  # Purple
]


@dataclass
class AcSignalData:
    """Data for an AC signal."""
    name: str
    frequencies: np.ndarray
    magnitude_db: np.ndarray
    phase_deg: np.ndarray
    color: str
    visible: bool = True
    mag_plot: Optional[pg.PlotDataItem] = None
    phase_plot: Optional[pg.PlotDataItem] = None


class BodePlot(QWidget):
    """
    Bode plot widget for AC analysis.

    Features:
    - Magnitude plot (dB vs log frequency)
    - Phase plot (degrees vs log frequency)
    - Multi-signal support
    - Zoom and pan
    - Linked X-axes
    - Export to image

    Signals:
        signal_added: Emitted when a signal is added (name)
        signal_removed: Emitted when a signal is removed (name)
    """

    signal_added = Signal(str)
    signal_removed = Signal(str)

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)

        self._signals: Dict[str, AcSignalData] = {}
        self._color_index = 0

        self._setup_ui()

    def _setup_ui(self):
        """Set up the user interface."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        # Toolbar
        toolbar = QHBoxLayout()
        toolbar.setContentsMargins(4, 2, 4, 2)

        self._auto_scale_btn = QPushButton("Auto Scale")
        self._auto_scale_btn.clicked.connect(self.auto_scale)
        toolbar.addWidget(self._auto_scale_btn)

        self._grid_btn = QPushButton("Grid")
        self._grid_btn.setCheckable(True)
        self._grid_btn.setChecked(True)
        self._grid_btn.clicked.connect(self._toggle_grid)
        toolbar.addWidget(self._grid_btn)

        self._export_btn = QPushButton("Export")
        self._export_btn.clicked.connect(self._export_image)
        toolbar.addWidget(self._export_btn)

        toolbar.addStretch()

        self._clear_btn = QPushButton("Clear All")
        self._clear_btn.clicked.connect(self.clear)
        toolbar.addWidget(self._clear_btn)

        layout.addLayout(toolbar)

        # Splitter for magnitude and phase plots
        splitter = QSplitter(Qt.Orientation.Vertical)

        # Magnitude plot
        self._mag_plot = pg.PlotWidget()
        self._mag_plot.setBackground('w')
        self._mag_plot.showGrid(x=True, y=True, alpha=0.3)
        self._mag_plot.setLabel('left', 'Magnitude', 'dB')
        self._mag_plot.setLabel('bottom', 'Frequency', 'Hz')
        self._mag_plot.setLogMode(x=True, y=False)
        self._mag_plot.setTitle("Magnitude")
        self._mag_legend = self._mag_plot.addLegend()
        splitter.addWidget(self._mag_plot)

        # Phase plot
        self._phase_plot = pg.PlotWidget()
        self._phase_plot.setBackground('w')
        self._phase_plot.showGrid(x=True, y=True, alpha=0.3)
        self._phase_plot.setLabel('left', 'Phase', '°')
        self._phase_plot.setLabel('bottom', 'Frequency', 'Hz')
        self._phase_plot.setLogMode(x=True, y=False)
        self._phase_plot.setTitle("Phase")
        self._phase_legend = self._phase_plot.addLegend()
        splitter.addWidget(self._phase_plot)

        # Link X axes
        self._phase_plot.setXLink(self._mag_plot)

        splitter.setSizes([300, 300])
        layout.addWidget(splitter)

    def add_signal(
        self,
        name: str,
        frequencies: List[float],
        magnitude_db: List[float],
        phase_deg: List[float],
        color: Optional[str] = None,
    ):
        """
        Add an AC signal to the Bode plot.

        Args:
            name: Signal name (e.g., "V(out)")
            frequencies: Frequency values in Hz
            magnitude_db: Magnitude values in dB
            phase_deg: Phase values in degrees
            color: Optional color (hex string)
        """
        # Remove existing signal with same name
        if name in self._signals:
            self.remove_signal(name)

        # Get color
        if color is None:
            color = DEFAULT_COLORS[self._color_index % len(DEFAULT_COLORS)]
            self._color_index += 1

        # Convert to numpy arrays
        freq_arr = np.array(frequencies)
        mag_arr = np.array(magnitude_db)
        phase_arr = np.array(phase_deg)

        # Create plot items
        pen = pg.mkPen(color=color, width=2)

        mag_plot = self._mag_plot.plot(
            freq_arr, mag_arr,
            pen=pen,
            name=name,
        )

        phase_plot = self._phase_plot.plot(
            freq_arr, phase_arr,
            pen=pen,
            name=name,
        )

        # Store signal data
        self._signals[name] = AcSignalData(
            name=name,
            frequencies=freq_arr,
            magnitude_db=mag_arr,
            phase_deg=phase_arr,
            color=color,
            visible=True,
            mag_plot=mag_plot,
            phase_plot=phase_plot,
        )

        self.signal_added.emit(name)

    def remove_signal(self, name: str):
        """Remove a signal from the Bode plot."""
        if name in self._signals:
            signal = self._signals[name]
            if signal.mag_plot:
                self._mag_plot.removeItem(signal.mag_plot)
            if signal.phase_plot:
                self._phase_plot.removeItem(signal.phase_plot)
            del self._signals[name]
            self.signal_removed.emit(name)

    def set_signal_visible(self, name: str, visible: bool):
        """Set signal visibility."""
        if name in self._signals:
            signal = self._signals[name]
            signal.visible = visible
            if signal.mag_plot:
                signal.mag_plot.setVisible(visible)
            if signal.phase_plot:
                signal.phase_plot.setVisible(visible)

    def get_signal_names(self) -> List[str]:
        """Get list of signal names."""
        return list(self._signals.keys())

    def clear(self):
        """Clear all signals."""
        for name in list(self._signals.keys()):
            self.remove_signal(name)
        self._color_index = 0

    def auto_scale(self):
        """Auto-scale both plots to fit all visible data."""
        self._mag_plot.enableAutoRange()
        self._mag_plot.autoRange()
        self._phase_plot.enableAutoRange()
        self._phase_plot.autoRange()

    def _toggle_grid(self, checked: bool):
        """Toggle grid visibility."""
        self._mag_plot.showGrid(x=checked, y=checked, alpha=0.3)
        self._phase_plot.showGrid(x=checked, y=checked, alpha=0.3)

    def _export_image(self):
        """Export plots to image file."""
        path, _ = QFileDialog.getSaveFileName(
            self, "Export Bode Plot",
            "bode.png",
            "PNG Files (*.png);;All Files (*)"
        )
        if path:
            # Export magnitude plot
            exporter = pg.exporters.ImageExporter(self._mag_plot.plotItem)
            mag_path = path.replace('.png', '_magnitude.png')
            exporter.export(mag_path)

            # Export phase plot
            exporter = pg.exporters.ImageExporter(self._phase_plot.plotItem)
            phase_path = path.replace('.png', '_phase.png')
            exporter.export(phase_path)

    def get_magnitude_plot(self) -> pg.PlotWidget:
        """Get the magnitude PlotWidget."""
        return self._mag_plot

    def get_phase_plot(self) -> pg.PlotWidget:
        """Get the phase PlotWidget."""
        return self._phase_plot
