# Bulk Unzip

A powerful tool for bulk extracting ZIP files and stripping MP3 metadata with both CLI and GUI interfaces.

## Features

### ZIP Extraction
- Bulk extract multiple ZIP files concurrently
- Configurable number of workers for optimal performance
- Skip existing directories to avoid overwrites
- Progress tracking and detailed results

### MP3 Metadata Stripping
- Remove all metadata from MP3 files
- Keep specific metadata fields (title, artist, album, year, track, genre)
- Process files in place or to a separate output directory
- Skip files that already have no metadata
- Dry run mode to preview changes

## Usage

### CLI Interface

The tool provides a command-line interface for both operations:

```bash
# Extract ZIP files
cargo run -- unzip --directory /path/to/zips --output /path/to/extract --workers 4

# Strip MP3 metadata
cargo run -- strip --directory /path/to/mp3s --remove-all --workers 4
```

### GUI Interface (Tauri)

The tool also includes a modern GUI built with Tauri and React:

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build and run the GUI
cargo tauri dev

# Build for production
cargo tauri build
```

## GUI Features

The GUI provides an intuitive interface with:

- **Tabbed Interface**: Switch between ZIP extraction and MP3 metadata stripping
- **Directory Selection**: Browse and select input/output directories
- **File Preview**: See all files that will be processed before starting
- **Progress Tracking**: Real-time feedback during processing
- **Configuration Options**: All CLI options available through the GUI
- **Results Display**: Detailed results and error reporting

## Installation

### Prerequisites

- Rust (latest stable)
- Node.js (16+)
- npm or yarn

### Setup

1. Clone the repository
2. Install Rust dependencies: `cargo build`
3. Install Node.js dependencies: `npm install`
4. Run the GUI: `cargo tauri dev`

## Development

### CLI Development

The CLI is built with Rust using:
- `tokio` for async runtime
- `rayon` for parallel processing
- `zip` for ZIP file handling
- `id3` for MP3 metadata manipulation
- `clap` for command-line argument parsing

### GUI Development

The GUI is built with:
- **Tauri 2.0** for the desktop app framework
- **React 18** with TypeScript for the frontend
- **Vite** for fast development and building
- **Lucide React** for icons

### Project Structure

```
bulk_unzip/
├── src/                    # CLI source code
│   ├── main.rs            # CLI entry point
│   └── metadata_stripper.rs
├── src-tauri/             # Tauri backend
│   ├── src/
│   │   ├── main.rs        # Tauri entry point
│   │   ├── lib.rs         # Backend logic
│   │   └── metadata_stripper.rs
│   └── tauri.conf.json    # Tauri configuration
├── src/                   # Frontend source
│   ├── App.tsx           # Main React component
│   ├── main.tsx          # React entry point
│   └── App.css           # Styles
├── package.json           # Node.js dependencies
└── Cargo.toml            # Rust dependencies
```

## Building

### CLI Only

```bash
cargo build --release
```

### GUI Application

```bash
# Development
cargo tauri dev

# Production build
cargo tauri build
```

## License

This project is open source and available under the MIT License. 