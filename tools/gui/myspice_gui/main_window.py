"""
Main application window for MySpice GUI.

Provides the main window with dockable panels for:
- Netlist editor
- Simulation control
- Waveform viewer
- Results table
- Console output
"""

import asyncio
import cmath
from pathlib import Path
from typing import Optional

from PySide6.QtWidgets import (
    QMainWindow,
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QDockWidget,
    QToolBar,
    QStatusBar,
    QMenuBar,
    QMenu,
    QPlainTextEdit,
    QLabel,
    QPushButton,
    QFileDialog,
    QMessageBox,
    QSplitter,
    QTabWidget,
    QComboBox,
    QDoubleSpinBox,
    QSpinBox,
    QFormLayout,
    QGroupBox,
    QTableWidget,
    QTableWidgetItem,
    QHeaderView,
)
from PySide6.QtGui import QAction, QKeySequence, QFont, QIcon
from PySide6.QtCore import Qt, QTimer, Slot

from .client import MySpiceClient, RunResult, AnalysisType, AcSweepType, ClientError
from .console import ConsoleWidget
from .editor import NetlistEditor
from .viewer import WaveformViewer, BodePlot, SignalListWidget, CursorManager
from .viewer.cursors import CursorControlPanel


class SimulationWorker:
    """
    Handles running simulations asynchronously.

    Uses asyncio to run simulations without blocking the UI.
    """

    def __init__(self, client: MySpiceClient):
        self.client = client
        self._loop: Optional[asyncio.AbstractEventLoop] = None

    def _get_loop(self) -> asyncio.AbstractEventLoop:
        """Get or create event loop."""
        if self._loop is None or self._loop.is_closed():
            try:
                self._loop = asyncio.get_event_loop()
            except RuntimeError:
                self._loop = asyncio.new_event_loop()
                asyncio.set_event_loop(self._loop)
        return self._loop

    def run_sync(self, coro):
        """Run a coroutine synchronously."""
        loop = self._get_loop()
        return loop.run_until_complete(coro)

    def check_connection(self) -> bool:
        """Check server connection."""
        return self.run_sync(self.client.check_connection())

    def run_op(self, netlist: str) -> RunResult:
        """Run operating point analysis."""
        return self.run_sync(self.client.run_op(netlist=netlist))

    def run_dc(self, netlist: str, source: str, start: float, stop: float, step: float) -> RunResult:
        """Run DC sweep analysis."""
        return self.run_sync(self.client.run_dc(
            netlist=netlist, source=source, start=start, stop=stop, step=step
        ))

    def run_tran(self, netlist: str, tstep: float, tstop: float,
                 tstart: float = 0.0, tmax: Optional[float] = None) -> RunResult:
        """Run transient analysis."""
        return self.run_sync(self.client.run_tran(
            netlist=netlist, tstep=tstep, tstop=tstop, tstart=tstart, tmax=tmax
        ))

    def run_ac(self, netlist: str, sweep: AcSweepType, points: int,
               fstart: float, fstop: float) -> RunResult:
        """Run AC analysis."""
        return self.run_sync(self.client.run_ac(
            netlist=netlist, sweep=sweep, points=points, fstart=fstart, fstop=fstop
        ))




class SimulationPanel(QWidget):
    """Panel for simulation controls."""

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._setup_ui()

    def _setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)

        # Analysis type tabs
        self._tabs = QTabWidget()
        self._tabs.addTab(self._create_op_panel(), "OP")
        self._tabs.addTab(self._create_dc_panel(), "DC")
        self._tabs.addTab(self._create_tran_panel(), "TRAN")
        self._tabs.addTab(self._create_ac_panel(), "AC")
        layout.addWidget(self._tabs)

        layout.addStretch()

    def _create_op_panel(self) -> QWidget:
        """Create operating point panel."""
        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.addWidget(QLabel("Operating Point Analysis"))
        layout.addWidget(QLabel("No additional parameters required."))
        layout.addStretch()
        return widget

    def _create_dc_panel(self) -> QWidget:
        """Create DC sweep panel."""
        widget = QWidget()
        layout = QFormLayout(widget)

        self._dc_source = QComboBox()
        self._dc_source.setEditable(True)
        self._dc_source.addItems(["V1", "V2", "I1"])
        layout.addRow("Source:", self._dc_source)

        self._dc_start = QDoubleSpinBox()
        self._dc_start.setRange(-1e9, 1e9)
        self._dc_start.setDecimals(6)
        self._dc_start.setValue(0.0)
        layout.addRow("Start:", self._dc_start)

        self._dc_stop = QDoubleSpinBox()
        self._dc_stop.setRange(-1e9, 1e9)
        self._dc_stop.setDecimals(6)
        self._dc_stop.setValue(5.0)
        layout.addRow("Stop:", self._dc_stop)

        self._dc_step = QDoubleSpinBox()
        self._dc_step.setRange(1e-15, 1e9)
        self._dc_step.setDecimals(6)
        self._dc_step.setValue(0.1)
        layout.addRow("Step:", self._dc_step)

        return widget

    def _create_tran_panel(self) -> QWidget:
        """Create transient panel."""
        widget = QWidget()
        layout = QFormLayout(widget)

        self._tran_tstep = QDoubleSpinBox()
        self._tran_tstep.setRange(1e-18, 1e3)
        self._tran_tstep.setDecimals(12)
        self._tran_tstep.setValue(1e-9)
        self._tran_tstep.setPrefix("tstep: ")
        self._tran_tstep.setSuffix(" s")
        layout.addRow("Time Step:", self._tran_tstep)

        self._tran_tstop = QDoubleSpinBox()
        self._tran_tstop.setRange(1e-18, 1e6)
        self._tran_tstop.setDecimals(12)
        self._tran_tstop.setValue(1e-3)
        self._tran_tstop.setSuffix(" s")
        layout.addRow("Stop Time:", self._tran_tstop)

        self._tran_tstart = QDoubleSpinBox()
        self._tran_tstart.setRange(0, 1e6)
        self._tran_tstart.setDecimals(12)
        self._tran_tstart.setValue(0.0)
        self._tran_tstart.setSuffix(" s")
        layout.addRow("Start Time:", self._tran_tstart)

        return widget

    def _create_ac_panel(self) -> QWidget:
        """Create AC analysis panel."""
        widget = QWidget()
        layout = QFormLayout(widget)

        self._ac_sweep = QComboBox()
        self._ac_sweep.addItems(["DEC", "OCT", "LIN"])
        layout.addRow("Sweep Type:", self._ac_sweep)

        self._ac_points = QSpinBox()
        self._ac_points.setRange(1, 10000)
        self._ac_points.setValue(10)
        layout.addRow("Points:", self._ac_points)

        self._ac_fstart = QDoubleSpinBox()
        self._ac_fstart.setRange(1e-6, 1e15)
        self._ac_fstart.setDecimals(3)
        self._ac_fstart.setValue(1.0)
        self._ac_fstart.setSuffix(" Hz")
        layout.addRow("Start Freq:", self._ac_fstart)

        self._ac_fstop = QDoubleSpinBox()
        self._ac_fstop.setRange(1e-6, 1e15)
        self._ac_fstop.setDecimals(3)
        self._ac_fstop.setValue(1e6)
        self._ac_fstop.setSuffix(" Hz")
        layout.addRow("Stop Freq:", self._ac_fstop)

        return widget

    def get_analysis_type(self) -> str:
        """Get selected analysis type."""
        return ["op", "dc", "tran", "ac"][self._tabs.currentIndex()]

    def get_dc_params(self) -> dict:
        """Get DC sweep parameters."""
        return {
            "source": self._dc_source.currentText(),
            "start": self._dc_start.value(),
            "stop": self._dc_stop.value(),
            "step": self._dc_step.value(),
        }

    def get_tran_params(self) -> dict:
        """Get transient parameters."""
        return {
            "tstep": self._tran_tstep.value(),
            "tstop": self._tran_tstop.value(),
            "tstart": self._tran_tstart.value(),
        }

    def get_ac_params(self) -> dict:
        """Get AC analysis parameters."""
        sweep_map = {"DEC": AcSweepType.DEC, "OCT": AcSweepType.OCT, "LIN": AcSweepType.LIN}
        return {
            "sweep": sweep_map[self._ac_sweep.currentText()],
            "points": self._ac_points.value(),
            "fstart": self._ac_fstart.value(),
            "fstop": self._ac_fstop.value(),
        }


class ResultsPanel(QWidget):
    """Panel for displaying results with table and text views."""

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._setup_ui()

    def _setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)

        # Tab widget for different result views
        self._tabs = QTabWidget()
        layout.addWidget(self._tabs)

        # Table view for OP/DC results
        self._table = QTableWidget()
        self._table.setColumnCount(2)
        self._table.setHorizontalHeaderLabels(["Variable", "Value"])
        self._table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self._table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        self._table.setAlternatingRowColors(True)
        self._tabs.addTab(self._table, "Table")

        # Text view for detailed output
        self._text = QPlainTextEdit()
        self._text.setReadOnly(True)
        self._text.setFont(QFont("Consolas", 9))
        self._text.setPlaceholderText("Results will appear here after simulation...")
        self._tabs.addTab(self._text, "Details")

    def _format_value(self, value: float, unit: str = "") -> str:
        """Format a value with engineering notation."""
        if abs(value) == 0:
            return f"0 {unit}".strip()

        prefixes = [
            (1e15, "P"), (1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k"),
            (1, ""), (1e-3, "m"), (1e-6, "u"), (1e-9, "n"), (1e-12, "p"), (1e-15, "f"),
        ]

        abs_value = abs(value)
        for scale, prefix in prefixes:
            if abs_value >= scale:
                return f"{value / scale:.4g} {prefix}{unit}".strip()

        return f"{value:.4g} {unit}".strip()

    def show_op_results(self, result: RunResult):
        """Display operating point results."""
        # Update table
        self._table.setRowCount(0)
        row = 0
        for node, value in zip(result.nodes, result.solution):
            if node != "0":  # Skip ground
                self._table.insertRow(row)
                self._table.setItem(row, 0, QTableWidgetItem(f"V({node})"))
                self._table.setItem(row, 1, QTableWidgetItem(self._format_value(value, "V")))
                row += 1

        # Update text view
        lines = ["Operating Point Analysis", "=" * 40, ""]
        lines.append(f"Status: {result.status}")
        lines.append(f"Iterations: {result.iterations}")
        lines.append("")
        lines.append("Node Voltages:")
        lines.append("-" * 30)

        for node, value in zip(result.nodes, result.solution):
            if node != "0":
                lines.append(f"  V({node:12s}) = {self._format_value(value, 'V'):>15s}")

        self._text.setPlainText("\n".join(lines))
        self._tabs.setCurrentIndex(0)  # Show table

    def show_dc_results(self, result: RunResult):
        """Display DC sweep results."""
        # Show summary in table
        self._table.setRowCount(3)
        self._table.setItem(0, 0, QTableWidgetItem("Sweep Variable"))
        self._table.setItem(0, 1, QTableWidgetItem(result.sweep_var or "N/A"))
        self._table.setItem(1, 0, QTableWidgetItem("Points"))
        self._table.setItem(1, 1, QTableWidgetItem(str(len(result.sweep_values))))
        self._table.setItem(2, 0, QTableWidgetItem("Range"))
        if result.sweep_values:
            range_str = f"{result.sweep_values[0]:.3g} to {result.sweep_values[-1]:.3g}"
        else:
            range_str = "N/A"
        self._table.setItem(2, 1, QTableWidgetItem(range_str))

        # Update text view
        lines = ["DC Sweep Analysis", "=" * 40, ""]
        lines.append(f"Sweep Variable: {result.sweep_var}")
        lines.append(f"Points: {len(result.sweep_values)}")
        lines.append("")

        self._text.setPlainText("\n".join(lines))
        self._tabs.setCurrentIndex(1)  # Show details

    def show_tran_results(self, result: RunResult):
        """Display transient results."""
        # Show summary in table
        self._table.setRowCount(3)
        self._table.setItem(0, 0, QTableWidgetItem("Time Points"))
        self._table.setItem(0, 1, QTableWidgetItem(str(len(result.tran_times))))
        self._table.setItem(1, 0, QTableWidgetItem("Start Time"))
        self._table.setItem(1, 1, QTableWidgetItem(
            self._format_value(result.tran_times[0], "s") if result.tran_times else "N/A"
        ))
        self._table.setItem(2, 0, QTableWidgetItem("Stop Time"))
        self._table.setItem(2, 1, QTableWidgetItem(
            self._format_value(result.tran_times[-1], "s") if result.tran_times else "N/A"
        ))

        # Update text view
        lines = ["Transient Analysis", "=" * 40, ""]
        lines.append(f"Time Points: {len(result.tran_times)}")
        if result.tran_times:
            lines.append(f"Time Range: {self._format_value(result.tran_times[0], 's')} to "
                        f"{self._format_value(result.tran_times[-1], 's')}")
        lines.append("")

        self._text.setPlainText("\n".join(lines))
        self._tabs.setCurrentIndex(1)  # Show details

    def show_ac_results(self, result: RunResult):
        """Display AC analysis results."""
        # Show summary in table
        self._table.setRowCount(3)
        self._table.setItem(0, 0, QTableWidgetItem("Frequency Points"))
        self._table.setItem(0, 1, QTableWidgetItem(str(len(result.ac_frequencies))))
        self._table.setItem(1, 0, QTableWidgetItem("Start Frequency"))
        self._table.setItem(1, 1, QTableWidgetItem(
            self._format_value(result.ac_frequencies[0], "Hz") if result.ac_frequencies else "N/A"
        ))
        self._table.setItem(2, 0, QTableWidgetItem("Stop Frequency"))
        self._table.setItem(2, 1, QTableWidgetItem(
            self._format_value(result.ac_frequencies[-1], "Hz") if result.ac_frequencies else "N/A"
        ))

        # Update text view
        lines = ["AC Analysis", "=" * 40, ""]
        lines.append(f"Frequency Points: {len(result.ac_frequencies)}")
        if result.ac_frequencies:
            lines.append(f"Frequency Range: {self._format_value(result.ac_frequencies[0], 'Hz')} to "
                        f"{self._format_value(result.ac_frequencies[-1], 'Hz')}")
        lines.append("")

        self._text.setPlainText("\n".join(lines))
        self._tabs.setCurrentIndex(1)  # Show details

    def clear(self):
        """Clear results."""
        self._table.setRowCount(0)
        self._text.clear()


class ViewerPanel(QWidget):
    """
    Panel for waveform and Bode plot viewers.

    Provides tabbed interface for:
    - Time-domain waveform viewer (TRAN, DC)
    - Bode plot viewer (AC)
    """

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._setup_ui()

    def _setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)

        # Tab widget for different viewers
        self._tabs = QTabWidget()
        layout.addWidget(self._tabs)

        # Waveform viewer for time-domain (TRAN) and DC
        self._waveform = WaveformViewer()
        self._tabs.addTab(self._waveform, "Waveform")

        # Bode plot for AC analysis
        self._bode = BodePlot()
        self._tabs.addTab(self._bode, "Bode")

    def get_waveform_viewer(self) -> WaveformViewer:
        """Get the waveform viewer widget."""
        return self._waveform

    def get_bode_plot(self) -> BodePlot:
        """Get the Bode plot widget."""
        return self._bode

    def show_waveform_tab(self):
        """Switch to waveform tab."""
        self._tabs.setCurrentIndex(0)

    def show_bode_tab(self):
        """Switch to Bode plot tab."""
        self._tabs.setCurrentIndex(1)

    def clear_all(self):
        """Clear all viewers."""
        self._waveform.clear()
        self._bode.clear()


class MainWindow(QMainWindow):
    """
    Main application window.

    Provides:
    - Menu bar with File, Edit, Simulate, View, Help menus
    - Toolbar with common actions
    - Dockable panels for editor, simulation control, results, console
    - Status bar with server connection status
    """

    def __init__(self, server_url: str = "http://127.0.0.1:3000", parent: Optional[QWidget] = None):
        super().__init__(parent)

        self._server_url = server_url
        self._client = MySpiceClient(server_url)
        self._worker = SimulationWorker(self._client)
        self._current_file: Optional[Path] = None
        self._modified = False

        self._setup_ui()
        self._setup_menus()
        self._setup_toolbar()
        self._setup_statusbar()
        self._setup_connections()

        # Check server connection
        QTimer.singleShot(100, self._check_server)

    def _setup_ui(self):
        """Set up the main UI layout."""
        self.setWindowTitle("MySpice")
        self.setMinimumSize(1024, 768)
        self.resize(1400, 900)

        # Central widget with splitter
        central = QWidget()
        self.setCentralWidget(central)
        layout = QHBoxLayout(central)
        layout.setContentsMargins(0, 0, 0, 0)

        splitter = QSplitter(Qt.Orientation.Horizontal)
        layout.addWidget(splitter)

        # Left panel: Editor
        self._editor = NetlistEditor()
        self._editor.setPlainText("""* RC Low-pass Filter
V1 in 0 DC 5 PULSE(0 5 0 1n 1n 5u 10u)
R1 in out 1k
C1 out 0 100n
.tran 10n 50u
.end
""")
        splitter.addWidget(self._editor)

        # Right panel: Results/Waveform
        right_splitter = QSplitter(Qt.Orientation.Vertical)

        self._viewer_panel = ViewerPanel()
        right_splitter.addWidget(self._viewer_panel)

        self._results = ResultsPanel()
        right_splitter.addWidget(self._results)

        right_splitter.setSizes([400, 200])
        splitter.addWidget(right_splitter)

        splitter.setSizes([500, 700])

        # Dock: Simulation Control
        self._sim_dock = QDockWidget("Simulation", self)
        self._sim_panel = SimulationPanel()
        self._sim_dock.setWidget(self._sim_panel)
        self.addDockWidget(Qt.DockWidgetArea.RightDockWidgetArea, self._sim_dock)

        # Dock: Signal List
        self._signal_dock = QDockWidget("Signals", self)
        self._signal_list = SignalListWidget()
        self._signal_dock.setWidget(self._signal_list)
        self.addDockWidget(Qt.DockWidgetArea.RightDockWidgetArea, self._signal_dock)

        # Dock: Cursor Controls
        self._cursor_dock = QDockWidget("Cursors", self)
        waveform_plot = self._viewer_panel.get_waveform_viewer().get_plot_widget()
        self._cursor_manager = CursorManager(waveform_plot)
        self._cursor_panel = CursorControlPanel(self._cursor_manager)
        self._cursor_dock.setWidget(self._cursor_panel)
        self.addDockWidget(Qt.DockWidgetArea.RightDockWidgetArea, self._cursor_dock)

        # Tab the right-side docks
        self.tabifyDockWidget(self._sim_dock, self._signal_dock)
        self.tabifyDockWidget(self._signal_dock, self._cursor_dock)
        self._sim_dock.raise_()  # Show simulation panel by default

        # Dock: Console
        self._console_dock = QDockWidget("Console", self)
        self._console = ConsoleWidget()
        self._console_dock.setWidget(self._console)
        self.addDockWidget(Qt.DockWidgetArea.BottomDockWidgetArea, self._console_dock)

    def _setup_menus(self):
        """Set up the menu bar."""
        menubar = self.menuBar()

        # File menu
        file_menu = menubar.addMenu("&File")

        self._new_action = QAction("&New", self)
        self._new_action.setShortcut(QKeySequence.StandardKey.New)
        self._new_action.triggered.connect(self._on_new)
        file_menu.addAction(self._new_action)

        self._open_action = QAction("&Open...", self)
        self._open_action.setShortcut(QKeySequence.StandardKey.Open)
        self._open_action.triggered.connect(self._on_open)
        file_menu.addAction(self._open_action)

        self._save_action = QAction("&Save", self)
        self._save_action.setShortcut(QKeySequence.StandardKey.Save)
        self._save_action.triggered.connect(self._on_save)
        file_menu.addAction(self._save_action)

        self._save_as_action = QAction("Save &As...", self)
        self._save_as_action.setShortcut(QKeySequence("Ctrl+Shift+S"))
        self._save_as_action.triggered.connect(self._on_save_as)
        file_menu.addAction(self._save_as_action)

        file_menu.addSeparator()

        self._exit_action = QAction("E&xit", self)
        self._exit_action.setShortcut(QKeySequence.StandardKey.Quit)
        self._exit_action.triggered.connect(self.close)
        file_menu.addAction(self._exit_action)

        # Edit menu
        edit_menu = menubar.addMenu("&Edit")

        self._undo_action = QAction("&Undo", self)
        self._undo_action.setShortcut(QKeySequence.StandardKey.Undo)
        self._undo_action.triggered.connect(self._editor.undo)
        edit_menu.addAction(self._undo_action)

        self._redo_action = QAction("&Redo", self)
        self._redo_action.setShortcut(QKeySequence.StandardKey.Redo)
        self._redo_action.triggered.connect(self._editor.redo)
        edit_menu.addAction(self._redo_action)

        edit_menu.addSeparator()

        self._cut_action = QAction("Cu&t", self)
        self._cut_action.setShortcut(QKeySequence.StandardKey.Cut)
        self._cut_action.triggered.connect(self._editor.cut)
        edit_menu.addAction(self._cut_action)

        self._copy_action = QAction("&Copy", self)
        self._copy_action.setShortcut(QKeySequence.StandardKey.Copy)
        self._copy_action.triggered.connect(self._editor.copy)
        edit_menu.addAction(self._copy_action)

        self._paste_action = QAction("&Paste", self)
        self._paste_action.setShortcut(QKeySequence.StandardKey.Paste)
        self._paste_action.triggered.connect(self._editor.paste)
        edit_menu.addAction(self._paste_action)

        # Simulate menu
        sim_menu = menubar.addMenu("&Simulate")

        self._run_action = QAction("&Run", self)
        self._run_action.setShortcut(QKeySequence("F5"))
        self._run_action.triggered.connect(self._on_run)
        sim_menu.addAction(self._run_action)

        sim_menu.addSeparator()

        self._run_op_action = QAction("Run &OP", self)
        self._run_op_action.triggered.connect(lambda: self._run_analysis("op"))
        sim_menu.addAction(self._run_op_action)

        self._run_dc_action = QAction("Run &DC", self)
        self._run_dc_action.triggered.connect(lambda: self._run_analysis("dc"))
        sim_menu.addAction(self._run_dc_action)

        self._run_tran_action = QAction("Run &TRAN", self)
        self._run_tran_action.triggered.connect(lambda: self._run_analysis("tran"))
        sim_menu.addAction(self._run_tran_action)

        self._run_ac_action = QAction("Run &AC", self)
        self._run_ac_action.triggered.connect(lambda: self._run_analysis("ac"))
        sim_menu.addAction(self._run_ac_action)

        # View menu
        view_menu = menubar.addMenu("&View")

        self._view_sim_action = self._sim_dock.toggleViewAction()
        self._view_sim_action.setText("&Simulation Panel")
        view_menu.addAction(self._view_sim_action)

        self._view_signals_action = self._signal_dock.toggleViewAction()
        self._view_signals_action.setText("Si&gnals Panel")
        view_menu.addAction(self._view_signals_action)

        self._view_cursors_action = self._cursor_dock.toggleViewAction()
        self._view_cursors_action.setText("C&ursors Panel")
        view_menu.addAction(self._view_cursors_action)

        self._view_console_action = self._console_dock.toggleViewAction()
        self._view_console_action.setText("&Console")
        view_menu.addAction(self._view_console_action)

        view_menu.addSeparator()

        # Clear viewers action
        self._clear_viewers_action = QAction("Clear &All Viewers", self)
        self._clear_viewers_action.triggered.connect(self._clear_all_viewers)
        view_menu.addAction(self._clear_viewers_action)

    def _clear_all_viewers(self):
        """Clear all viewer content."""
        self._viewer_panel.clear_all()
        self._signal_list.clear()
        self._cursor_manager.clear()
        self._results.clear()
        self._console.info("Cleared all viewers")

        # Help menu
        help_menu = menubar.addMenu("&Help")

        self._about_action = QAction("&About", self)
        self._about_action.triggered.connect(self._on_about)
        help_menu.addAction(self._about_action)

    def _setup_toolbar(self):
        """Set up the main toolbar."""
        toolbar = QToolBar("Main Toolbar")
        toolbar.setMovable(False)
        self.addToolBar(toolbar)

        toolbar.addAction(self._new_action)
        toolbar.addAction(self._open_action)
        toolbar.addAction(self._save_action)
        toolbar.addSeparator()
        toolbar.addAction(self._run_action)

    def _setup_statusbar(self):
        """Set up the status bar."""
        self._statusbar = QStatusBar()
        self.setStatusBar(self._statusbar)

        # Cursor position indicator
        self._position_label = QLabel("Line 1, Col 1")
        self._statusbar.addWidget(self._position_label)

        # Spacer
        self._statusbar.addWidget(QLabel("  |  "))

        # Mode indicator
        self._mode_label = QLabel("SPICE")
        self._statusbar.addWidget(self._mode_label)

        # Server status indicator (right side)
        self._server_label = QLabel("Server: Checking...")
        self._statusbar.addPermanentWidget(self._server_label)

    def _setup_connections(self):
        """Set up signal connections."""
        self._editor.textChanged.connect(self._on_text_changed)
        self._editor.cursor_position_changed.connect(self._on_cursor_position_changed)

        # Signal list connections
        self._signal_list.signal_visibility_changed.connect(self._on_signal_visibility_changed)
        self._signal_list.signal_color_changed.connect(self._on_signal_color_changed)
        self._signal_list.signal_removed.connect(self._on_signal_removed)

        # Cursor position updates
        waveform = self._viewer_panel.get_waveform_viewer()
        waveform.cursor_moved.connect(self._on_waveform_cursor_moved)

    @Slot(str, bool)
    def _on_signal_visibility_changed(self, name: str, visible: bool):
        """Handle signal visibility change."""
        waveform = self._viewer_panel.get_waveform_viewer()
        bode = self._viewer_panel.get_bode_plot()
        waveform.set_signal_visible(name, visible)
        bode.set_signal_visible(name, visible)

    @Slot(str, str)
    def _on_signal_color_changed(self, name: str, color: str):
        """Handle signal color change."""
        waveform = self._viewer_panel.get_waveform_viewer()
        waveform.set_signal_color(name, color)

    @Slot(str)
    def _on_signal_removed(self, name: str):
        """Handle signal removal."""
        waveform = self._viewer_panel.get_waveform_viewer()
        bode = self._viewer_panel.get_bode_plot()
        waveform.remove_signal(name)
        bode.remove_signal(name)

    @Slot(float, float)
    def _on_waveform_cursor_moved(self, x: float, y: float):
        """Handle cursor movement in waveform viewer."""
        # Update cursor panel readout
        self._cursor_panel.refresh()

    @Slot(int, int)
    def _on_cursor_position_changed(self, line: int, column: int):
        """Handle cursor position changes."""
        self._position_label.setText(f"Line {line}, Col {column}")

    def _check_server(self):
        """Check server connection and update status."""
        try:
            connected = self._worker.check_connection()
            if connected:
                self._server_label.setText(f"Server: Connected ({self._server_url})")
                self._server_label.setStyleSheet("color: green;")
                self._console.success(f"Connected to server at {self._server_url}")
            else:
                self._server_label.setText("Server: Disconnected")
                self._server_label.setStyleSheet("color: red;")
                self._console.error(f"Cannot connect to server at {self._server_url}")
        except Exception as e:
            self._server_label.setText("Server: Error")
            self._server_label.setStyleSheet("color: red;")
            self._console.error(f"Server error: {e}")

    def _on_text_changed(self):
        """Handle editor text changes."""
        self._modified = True
        self._update_title()

    def _update_title(self):
        """Update window title."""
        title = "MySpice"
        if self._current_file:
            title = f"{self._current_file.name} - MySpice"
        if self._modified:
            title = "* " + title
        self.setWindowTitle(title)

    @Slot()
    def _on_new(self):
        """Create new netlist."""
        if self._modified:
            reply = QMessageBox.question(
                self, "Unsaved Changes",
                "Do you want to save changes before creating a new file?",
                QMessageBox.StandardButton.Save |
                QMessageBox.StandardButton.Discard |
                QMessageBox.StandardButton.Cancel
            )
            if reply == QMessageBox.StandardButton.Save:
                self._on_save()
            elif reply == QMessageBox.StandardButton.Cancel:
                return

        self._editor.clear()
        self._current_file = None
        self._modified = False
        self._update_title()
        self._console.info("New netlist created")

    @Slot()
    def _on_open(self):
        """Open netlist file."""
        path, _ = QFileDialog.getOpenFileName(
            self, "Open Netlist",
            "",
            "SPICE Netlists (*.cir *.sp *.spice);;All Files (*)"
        )
        if path:
            self.open_file(path)

    def open_file(self, path: str):
        """Open a specific file."""
        try:
            file_path = Path(path)
            text = file_path.read_text()
            self._editor.setPlainText(text)
            self._current_file = file_path
            self._modified = False
            self._update_title()
            self._console.success(f"Opened: {file_path.name}")
        except Exception as e:
            QMessageBox.critical(self, "Error", f"Cannot open file: {e}")
            self._console.error(f"Failed to open file: {e}")

    @Slot()
    def _on_save(self):
        """Save current netlist."""
        if self._current_file:
            self._save_file(self._current_file)
        else:
            self._on_save_as()

    @Slot()
    def _on_save_as(self):
        """Save netlist with new name."""
        path, _ = QFileDialog.getSaveFileName(
            self, "Save Netlist",
            "",
            "SPICE Netlists (*.cir *.sp *.spice);;All Files (*)"
        )
        if path:
            self._save_file(Path(path))

    def _save_file(self, path: Path):
        """Save to specific file."""
        try:
            path.write_text(self._editor.toPlainText())
            self._current_file = path
            self._modified = False
            self._update_title()
            self._console.success(f"Saved: {path.name}")
        except Exception as e:
            QMessageBox.critical(self, "Error", f"Cannot save file: {e}")
            self._console.error(f"Failed to save file: {e}")

    @Slot()
    def _on_run(self):
        """Run selected analysis."""
        analysis = self._sim_panel.get_analysis_type()
        self._run_analysis(analysis)

    def _run_analysis(self, analysis: str):
        """Run specific analysis type."""
        netlist = self._editor.toPlainText()
        if not netlist.strip():
            self._console.warning("No netlist to simulate")
            return

        self._console.info(f"Running {analysis.upper()} analysis...")
        self._statusbar.showMessage(f"Running {analysis.upper()}...", 0)

        try:
            if analysis == "op":
                result = self._worker.run_op(netlist)
                self._results.show_op_results(result)
            elif analysis == "dc":
                params = self._sim_panel.get_dc_params()
                result = self._worker.run_dc(netlist, **params)
                self._results.show_dc_results(result)
                self._plot_dc_results(result)
            elif analysis == "tran":
                params = self._sim_panel.get_tran_params()
                result = self._worker.run_tran(netlist, **params)
                self._results.show_tran_results(result)
                self._plot_tran_results(result)
            elif analysis == "ac":
                params = self._sim_panel.get_ac_params()
                result = self._worker.run_ac(netlist, **params)
                self._results.show_ac_results(result)
                self._plot_ac_results(result)
            else:
                self._console.error(f"Unknown analysis type: {analysis}")
                return

            if result.status == "Success":
                self._console.success(f"{analysis.upper()} analysis completed successfully")
                if result.message:
                    self._console.info(result.message)
            else:
                self._console.error(f"{analysis.upper()} analysis failed: {result.message}")

            self._statusbar.showMessage(f"{analysis.upper()} completed", 5000)

        except ClientError as e:
            self._console.error(f"Simulation error: {e}")
            if e.details:
                for detail in e.details:
                    self._console.error(f"  - {detail}")
            self._statusbar.showMessage("Simulation failed", 5000)
        except Exception as e:
            self._console.error(f"Error: {e}")
            self._statusbar.showMessage("Error occurred", 5000)

    def _plot_dc_results(self, result: RunResult):
        """Plot DC sweep results in waveform viewer."""
        waveform = self._viewer_panel.get_waveform_viewer()
        waveform.clear()
        self._signal_list.clear()

        if not result.sweep_values or not result.dc_values:
            return

        x_data = result.sweep_values
        waveform.set_labels(result.sweep_var or "Sweep", "V", "Voltage", "V")
        waveform.set_title(f"DC Sweep: {result.sweep_var}")

        for node, values in result.dc_values.items():
            if node != "0":  # Skip ground
                name = f"V({node})"
                waveform.add_signal(name, x_data, values)
                color = waveform._signals[name].color if name in waveform._signals else "#1f77b4"
                self._signal_list.add_signal(name, color)

        waveform.auto_scale()
        self._viewer_panel.show_waveform_tab()
        self._signal_dock.raise_()

    def _plot_tran_results(self, result: RunResult):
        """Plot transient results in waveform viewer."""
        waveform = self._viewer_panel.get_waveform_viewer()
        waveform.clear()
        self._signal_list.clear()

        if not result.tran_times or not result.tran_values:
            return

        x_data = result.tran_times
        waveform.set_labels("Time", "s", "Voltage", "V")
        waveform.set_title("Transient Analysis")

        for node, values in result.tran_values.items():
            if node != "0":  # Skip ground
                name = f"V({node})"
                waveform.add_signal(name, x_data, values)
                color = waveform._signals[name].color if name in waveform._signals else "#1f77b4"
                self._signal_list.add_signal(name, color)

        waveform.auto_scale()
        self._viewer_panel.show_waveform_tab()
        self._signal_dock.raise_()

    def _plot_ac_results(self, result: RunResult):
        """Plot AC results in Bode plot."""
        import math

        bode = self._viewer_panel.get_bode_plot()
        bode.clear()
        self._signal_list.clear()

        if not result.ac_frequencies or not result.ac_values:
            return

        frequencies = result.ac_frequencies

        for node, complex_values in result.ac_values.items():
            if node != "0":  # Skip ground
                name = f"V({node})"
                # Convert complex values to magnitude (dB) and phase (degrees)
                magnitude_db = []
                phase_deg = []
                for cval in complex_values:
                    if isinstance(cval, complex):
                        mag = abs(cval)
                        phase = cmath.phase(cval)
                    else:
                        # Handle as [real, imag] list
                        c = complex(cval[0], cval[1]) if isinstance(cval, (list, tuple)) else complex(cval)
                        mag = abs(c)
                        phase = cmath.phase(c)

                    # Convert to dB (avoid log(0))
                    mag_db = 20 * math.log10(mag) if mag > 0 else -200
                    magnitude_db.append(mag_db)
                    phase_deg.append(math.degrees(phase))

                bode.add_signal(name, frequencies, magnitude_db, phase_deg)
                color = bode._signals[name].color if name in bode._signals else "#1f77b4"
                self._signal_list.add_signal(name, color)

        bode.auto_scale()
        self._viewer_panel.show_bode_tab()
        self._signal_dock.raise_()

    @Slot()
    def _on_about(self):
        """Show about dialog."""
        QMessageBox.about(
            self, "About MySpice",
            """<h2>MySpice GUI</h2>
            <p>Version 0.1.0</p>
            <p>A graphical interface for MySpice circuit simulator.</p>
            <p>Built with PySide6 (Qt for Python)</p>
            """
        )

    def closeEvent(self, event):
        """Handle window close."""
        if self._modified:
            reply = QMessageBox.question(
                self, "Unsaved Changes",
                "Do you want to save changes before closing?",
                QMessageBox.StandardButton.Save |
                QMessageBox.StandardButton.Discard |
                QMessageBox.StandardButton.Cancel
            )
            if reply == QMessageBox.StandardButton.Save:
                self._on_save()
                event.accept()
            elif reply == QMessageBox.StandardButton.Cancel:
                event.ignore()
                return
            else:
                event.accept()
        else:
            event.accept()

        self._console.info("Goodbye!")
