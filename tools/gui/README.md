# MySpice GUI

Graphical user interface for MySpice circuit simulator, built with PySide6 (Qt for Python).

## Features

- **Netlist Editor**: Edit SPICE netlists with syntax support
- **Simulation Control**: Run OP, DC, TRAN, AC analyses
- **Results Panel**: View operating point and analysis results
- **Console Output**: Colored log messages with timestamps
- **Dockable Panels**: Flexible layout customization

## Installation

### Prerequisites

- Python 3.10 or later
- sim-api server running (from the main MySpice project)

### Install from source

```bash
cd tools/gui
pip install -e .
```

### Install with development dependencies

```bash
pip install -e ".[dev]"
```

## Usage

### 1. Start the sim-api server

In a separate terminal:

```bash
# From the project root
cargo run -p sim-api -- --addr 127.0.0.1:3000
```

### 2. Launch the GUI

```bash
# Default server (localhost:3000)
myspice-gui

# Connect to specific server
myspice-gui --server http://192.168.1.100:3000

# Open a netlist file
myspice-gui my_circuit.cir
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+N | New netlist |
| Ctrl+O | Open file |
| Ctrl+S | Save file |
| Ctrl+Shift+S | Save as |
| F5 | Run simulation |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Ctrl+Q | Quit |

## Screenshot

```
┌─────────────────────────────────────────────────────────────────────┐
│  MySpice - rc_lowpass.cir                                      [─][□][×]
├─────────────────────────────────────────────────────────────────────┤
│  File  Edit  Simulate  View  Help                                   │
├─────────────────────────────────────────────────────────────────────┤
│  [New] [Open] [Save] │ [Run ▶]                                      │
├───────────────────┬─────────────────────────────────────────────────┤
│                   │                                                 │
│   * RC circuit    │   Waveform Viewer (Phase 4)                     │
│   V1 in 0 5       │                                                 │
│   R1 in out 1k    ├─────────────────────────────────────────────────┤
│   C1 out 0 100n   │   Operating Point Analysis                      │
│   .tran 10n 50u   │   ========================================      │
│   .end            │   Status: Success                               │
│                   │   V(in)  = 5.000000 V                           │
│                   │   V(out) = 3.333333 V                           │
├───────────────────┴─────────────────────────────────────────────────┤
│ Console                                                             │
│ [12:34:56] Connected to server at http://127.0.0.1:3000            │
│ [12:34:58] Running OP analysis...                                   │
│ [12:34:58] OP analysis completed successfully                       │
└─────────────────────────────────────────────────────────────────────┘
```

## Architecture

```
myspice_gui/
├── __init__.py       # Package exports
├── __main__.py       # Entry point
├── main_window.py    # Main window and panels
├── client.py         # HTTP client for sim-api
└── console/          # Console output widget
    ├── __init__.py
    └── console.py
```

## Development

### Running tests

```bash
pytest
```

### Code formatting

```bash
black myspice_gui/
ruff check myspice_gui/
```

## Roadmap

- [x] **Phase 1**: Core infrastructure, main window, HTTP client, console
- [ ] **Phase 2**: Netlist editor with syntax highlighting
- [ ] **Phase 3**: Simulation control panel improvements
- [ ] **Phase 4**: Waveform viewer with pyqtgraph
- [ ] **Phase 5**: Results table, Bode plot, polish
- [ ] **Phase 6**: Advanced features (cursors, FFT, themes)

## License

MIT License
