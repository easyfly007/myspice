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
)
from PySide6.QtGui import QAction, QKeySequence, QFont, QIcon
from PySide6.QtCore import Qt, QTimer, Slot

from .client import MySpiceClient, RunResult, AnalysisType, AcSweepType, ClientError
from .console import ConsoleWidget
from .editor import NetlistEditor


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
    """Panel for displaying results."""

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        self._setup_ui()

    def _setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)

        self._text = QPlainTextEdit()
        self._text.setReadOnly(True)
        self._text.setFont(QFont("Consolas", 9))
        self._text.setPlaceholderText("Results will appear here after simulation...")
        layout.addWidget(self._text)

    def show_op_results(self, result: RunResult):
        """Display operating point results."""
        lines = ["Operating Point Analysis", "=" * 40, ""]
        lines.append(f"Status: {result.status}")
        lines.append(f"Iterations: {result.iterations}")
        lines.append("")
        lines.append("Node Voltages:")
        lines.append("-" * 30)

        for node, value in zip(result.nodes, result.solution):
            if node != "0":  # Skip ground
                lines.append(f"  V({node:12s}) = {value:12.6g} V")

        self._text.setPlainText("\n".join(lines))

    def show_dc_results(self, result: RunResult):
        """Display DC sweep results."""
        lines = ["DC Sweep Analysis", "=" * 40, ""]
        lines.append(f"Sweep Variable: {result.sweep_var}")
        lines.append(f"Points: {len(result.sweep_values)}")
        lines.append("")

        self._text.setPlainText("\n".join(lines))

    def show_tran_results(self, result: RunResult):
        """Display transient results."""
        lines = ["Transient Analysis", "=" * 40, ""]
        lines.append(f"Time Points: {len(result.tran_times)}")
        if result.tran_times:
            lines.append(f"Time Range: {result.tran_times[0]:.3e} to {result.tran_times[-1]:.3e} s")
        lines.append("")

        self._text.setPlainText("\n".join(lines))

    def show_ac_results(self, result: RunResult):
        """Display AC analysis results."""
        lines = ["AC Analysis", "=" * 40, ""]
        lines.append(f"Frequency Points: {len(result.ac_frequencies)}")
        if result.ac_frequencies:
            lines.append(f"Frequency Range: {result.ac_frequencies[0]:.3e} to {result.ac_frequencies[-1]:.3e} Hz")
        lines.append("")

        self._text.setPlainText("\n".join(lines))

    def clear(self):
        """Clear results."""
        self._text.clear()


class WaveformPlaceholder(QWidget):
    """Placeholder for waveform viewer (will be implemented in Phase 4)."""

    def __init__(self, parent: Optional[QWidget] = None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        label = QLabel("Waveform Viewer\n(Coming in Phase 4)")
        label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        label.setStyleSheet("color: #888; font-size: 14px;")
        layout.addWidget(label)


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

        self._waveform = WaveformPlaceholder()
        right_splitter.addWidget(self._waveform)

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

        self._view_console_action = self._console_dock.toggleViewAction()
        self._view_console_action.setText("&Console")
        view_menu.addAction(self._view_console_action)

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
            elif analysis == "tran":
                params = self._sim_panel.get_tran_params()
                result = self._worker.run_tran(netlist, **params)
                self._results.show_tran_results(result)
            elif analysis == "ac":
                params = self._sim_panel.get_ac_params()
                result = self._worker.run_ac(netlist, **params)
                self._results.show_ac_results(result)
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
