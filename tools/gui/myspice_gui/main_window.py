"""
Main application window for MySpice GUI.

Provides the main window with dockable panels for:
- Netlist editor
- Simulation control
- Waveform viewer
- Results table
- Console output
"""

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
from .simulation import SimulationPanel, SimulationWorker, SimulationTask
from .simulation.worker import AnalysisType as SimAnalysisType, ConnectionChecker


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
        self._worker: Optional[SimulationWorker] = None
        self._connection_checker: Optional[ConnectionChecker] = None
        self._current_file: Optional[Path] = None
        self._modified = False

        self._setup_ui()
        self._setup_menus()
        self._setup_toolbar()
        self._setup_statusbar()
        self._setup_connections()

        # Check server connection (non-blocking)
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

        # Simulation panel connections
        self._sim_panel.run_requested.connect(self._on_simulation_requested)
        self._sim_panel.stop_requested.connect(self._on_simulation_stop_requested)

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
        """Check server connection and update status (non-blocking)."""
        self._server_label.setText("Server: Checking...")
        self._server_label.setStyleSheet("color: gray;")

        # Use ConnectionChecker for non-blocking check
        self._connection_checker = ConnectionChecker(self._client, self)
        self._connection_checker.result.connect(self._on_connection_check_result)
        self._connection_checker.start()

    @Slot(bool)
    def _on_connection_check_result(self, connected: bool):
        """Handle connection check result."""
        if connected:
            self._server_label.setText(f"Server: Connected ({self._server_url})")
            self._server_label.setStyleSheet("color: green;")
            self._sim_panel.set_server_status(True, self._server_url)
            self._console.success(f"Connected to server at {self._server_url}")
        else:
            self._server_label.setText("Server: Disconnected")
            self._server_label.setStyleSheet("color: red;")
            self._sim_panel.set_server_status(False)
            self._console.error(f"Cannot connect to server at {self._server_url}")

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
        """Run selected analysis from menu/toolbar."""
        analysis = self._sim_panel.get_analysis_type()
        params = self._sim_panel.get_params()
        self._start_simulation(analysis, params)

    @Slot(str, dict)
    def _on_simulation_requested(self, analysis: str, params: dict):
        """Handle simulation request from panel."""
        self._start_simulation(analysis, params)

    @Slot()
    def _on_simulation_stop_requested(self):
        """Handle stop request from panel."""
        if self._worker and self._worker.isRunning():
            self._worker.request_stop()
            self._console.warning("Stop requested...")

    def _start_simulation(self, analysis: str, params: dict):
        """Start a simulation in background thread."""
        netlist = self._editor.toPlainText()
        if not netlist.strip():
            self._console.warning("No netlist to simulate")
            self._sim_panel.set_status("No netlist to simulate", is_error=True)
            return

        # Validate parameters
        errors = self._sim_panel.validate()
        if errors:
            self._console.error(f"Validation error: {errors[0]}")
            self._sim_panel.set_status(errors[0], is_error=True)
            return

        # Map analysis string to enum
        analysis_map = {
            "op": SimAnalysisType.OP,
            "dc": SimAnalysisType.DC,
            "tran": SimAnalysisType.TRAN,
            "ac": SimAnalysisType.AC,
        }
        analysis_enum = analysis_map.get(analysis)
        if not analysis_enum:
            self._console.error(f"Unknown analysis type: {analysis}")
            return

        # Create simulation task
        task = SimulationTask(
            analysis=analysis_enum,
            netlist=netlist,
            params=params
        )

        # Create worker and connect signals
        self._worker = SimulationWorker(self._client, self)
        self._worker.simulation_started.connect(self._on_simulation_started)
        self._worker.progress.connect(self._on_simulation_progress)
        self._worker.finished.connect(self._on_simulation_finished)
        self._worker.error.connect(self._on_simulation_error)
        self._worker.stopped.connect(self._on_simulation_stopped)

        # Set task and start
        self._worker.set_task(task)
        self._worker.start()

        # Update UI state
        self._sim_panel.set_running(True)
        self._run_action.setEnabled(False)
        self._console.info(f"Running {analysis.upper()} analysis...")
        self._statusbar.showMessage(f"Running {analysis.upper()}...", 0)

    @Slot(str)
    def _on_simulation_started(self, analysis: str):
        """Handle simulation start."""
        self._sim_panel.set_progress(f"Running {analysis} analysis...")

    @Slot(str)
    def _on_simulation_progress(self, message: str):
        """Handle simulation progress update."""
        self._sim_panel.set_progress(message)

    @Slot(object)
    def _on_simulation_finished(self, result: RunResult):
        """Handle simulation completion."""
        # Reset UI state
        self._sim_panel.set_running(False)
        self._run_action.setEnabled(True)

        analysis = result.analysis.value if hasattr(result.analysis, 'value') else str(result.analysis)

        # Display results based on analysis type
        if result.analysis == AnalysisType.OP:
            self._results.show_op_results(result)
        elif result.analysis == AnalysisType.DC:
            self._results.show_dc_results(result)
            self._plot_dc_results(result)
        elif result.analysis == AnalysisType.TRAN:
            self._results.show_tran_results(result)
            self._plot_tran_results(result)
        elif result.analysis == AnalysisType.AC:
            self._results.show_ac_results(result)
            self._plot_ac_results(result)

        # Update status
        if result.status == "Success":
            self._console.success(f"{analysis} analysis completed successfully")
            self._sim_panel.set_success(f"{analysis} completed")
            if result.message:
                self._console.info(result.message)
        else:
            self._console.error(f"{analysis} analysis: {result.message}")
            self._sim_panel.set_status(result.message or "Failed", is_error=True)

        self._statusbar.showMessage(f"{analysis} completed", 5000)

    @Slot(str, list)
    def _on_simulation_error(self, message: str, details: list):
        """Handle simulation error."""
        # Reset UI state
        self._sim_panel.set_running(False)
        self._run_action.setEnabled(True)

        self._console.error(f"Simulation error: {message}")
        for detail in details:
            self._console.error(f"  - {detail}")

        self._sim_panel.set_status(message, is_error=True)
        self._statusbar.showMessage("Simulation failed", 5000)

    @Slot()
    def _on_simulation_stopped(self):
        """Handle simulation stopped by user."""
        # Reset UI state
        self._sim_panel.set_running(False)
        self._run_action.setEnabled(True)

        self._console.warning("Simulation stopped by user")
        self._sim_panel.set_status("Stopped")
        self._statusbar.showMessage("Simulation stopped", 5000)

    def _run_analysis(self, analysis: str):
        """Run specific analysis type (for menu actions)."""
        # Get params based on analysis type
        if analysis == "op":
            params = {}
        elif analysis == "dc":
            params = self._sim_panel.get_params() if self._sim_panel.get_analysis_type() == "dc" else {
                "source": "V1", "start": 0.0, "stop": 5.0, "step": 0.1
            }
        elif analysis == "tran":
            params = self._sim_panel.get_params() if self._sim_panel.get_analysis_type() == "tran" else {
                "tstep": 1e-9, "tstop": 1e-3, "tstart": 0.0
            }
        elif analysis == "ac":
            params = self._sim_panel.get_params() if self._sim_panel.get_analysis_type() == "ac" else {
                "sweep": AcSweepType.DEC, "points": 10, "fstart": 1.0, "fstop": 1e6
            }
        else:
            params = {}

        self._start_simulation(analysis, params)

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
