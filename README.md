# EmpirePorter

EmpirePorter is a Windows desktop tool for importing and exporting Stellaris empire designs.

Current release: `0.1.0`

It reads Stellaris `user_empire_designs_v3.4.txt` files, lets you select full empire design blocks, previews the raw output, and writes cleanly formatted import/export files.

## Features

- Export one or more full Stellaris empire designs to a shareable text file.
- Import selected empire designs into an existing Stellaris user empire file.
- Choose how duplicate empires are handled: skip, replace, or append.
- Preserve safety with automatic backups, temporary writes, and parse validation before replacing files.
- Normalize generated files with clean CRLF line endings and blank lines between empire blocks.
- Use a 4K-friendly interface with zoom controls and a resizable preview split.

## Default Paths

- Export file: `%USERPROFILE%\Downloads\empire_porter_export.txt`
- Stellaris target file: `%USERPROFILE%\Documents\Paradox Interactive\Stellaris\user_empire_designs_v3.4.txt`

## Usage

1. Launch `empire-porter.exe`.
2. Use the `Export` tab to open a Stellaris empire file, select empires, and save a shareable export.
3. Use the `Import` tab to open an EmpirePorter export file and import selected empires into your Stellaris target file.
4. Pick a conflict policy before importing if the target file already contains matching empires.

## Building

Install Rust, then build from the repository root:

```powershell
cargo build --release
```

The optimized executable is written to:

```text
target\release\empire-porter.exe
```

For development checks:

```powershell
cargo fmt
cargo check
cargo test
```

## License

Copyright (C) 2026 Alex Hurshman.

EmpirePorter is licensed under the GNU General Public License v3.0 only. See `LICENSE` for the full license text.

Source files also include SPDX license identifiers for clearer reuse and compliance tracking.

See `NOTICE` for the project copyright notice and disclaimer.

## Warranty

EmpirePorter is provided without warranty, including without the implied warranties of merchantability or fitness for a particular purpose.

## Disclaimer

EmpirePorter is an unofficial tool and is not affiliated with Paradox Interactive or Stellaris.
