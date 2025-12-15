# schismrs-vgrid

Vertical grid generation for SCHISM ocean model, featuring an interactive TUI designer.

## VQS Designer

The star of this crate is **vqs-designer** - an interactive terminal application for designing VQS (Variable Quadratic/S-coordinate) master grids.

![VQS Designer Workflow](./assets/vqs_designer_workflow.gif)

```bash
cargo run --release --bin vqs-designer -- -g /path/to/hgrid.gr3
```

### Interface Overview

The designer features a **unified split-screen interface**:

- **Left Panel**: Anchor list showing depth and number of levels (N) for each anchor point
- **Right Panel**: Profile visualization with three view modes
- **Draggable Divider**: Resize panels with mouse or `{`/`}` keys

### View Modes

Press `v` to cycle through profile view modes:

| Mode | Description |
|------|-------------|
| **Single Depth** | Bar chart showing layer thicknesses at selected anchor |
| **Multi-Depth** | Table with min/avg/max Δz statistics for all anchors |
| **Mesh Summary** | Depth percentiles and mesh coverage analysis |

### Stretching Functions

Press `t` to cycle through five stretching functions:

| Function | Best For | Key Parameters |
|----------|----------|----------------|
| **Quadratic** | Simple applications, uniform distribution | `a` (-1 to 1) |
| **S-transform** | General SCHISM, estuarine modeling | `θf`, `θb` |
| **Shchepetkin2005** | ROMS compatibility, shelf/slope | `θs`, `θb`, `hc` |
| **Shchepetkin2010** | Deep ocean, dual surface/bottom focus | `θs`, `θb`, `hc` |
| **Geyer** | Shallow coastal, bottom boundary layer | `θs`, `θb`, `hc` |

Press `i` for detailed help on the current stretching function and its parameters.

### Mesh-Aware Suggestions

Press `S` to enter suggestion mode (requires mesh loaded with `-g`):

- **Exponential**: Concentrates resolution in shallow depths
- **Uniform**: Even spacing across depth range
- **Percentile**: Based on mesh depth distribution

Real-time preview shows:
- Layer count (N) with truncation indicators (e.g., `10→8` when bottom truncation applies)
- Min/avg/max Δz statistics computed from actual mesh nodes
- Adaptive precision for small values (mm-scale shown as `0.003m`, not `0.00m`)

### Keyboard Controls

#### Navigation & Editing
| Key | Action |
|-----|--------|
| `↑`/`↓` or `j`/`k` | Navigate anchors |
| `a` | Add new anchor (depth, then N) |
| `d` | Delete selected anchor |
| `e` or `Enter` | Edit selected anchor |
| `c` | Clear all anchors |

#### View & Export
| Key | Action |
|-----|--------|
| `v` | Cycle view mode (Single/Multi-Depth/Mesh Summary) |
| `E` | Open export modal |
| `S` | Enter suggestion mode (requires mesh) |
| `?` or `F1` | Toggle help overlay |

#### Stretching Controls
| Key | Action |
|-----|--------|
| `t` | Cycle stretching type |
| `i` | Show stretching function info |
| `f`/`F` | Adjust θf ±0.5 (S-transform) |
| `b`/`B` | Adjust θb ±0.1 |
| `s` | Adjust θs +0.5 (ROMS transforms) |
| `h`/`H` | Adjust hc ±1m (ROMS transforms) |
| `A` | Adjust a_vqs0 +0.1 (Quadratic) |
| `z`/`Z` | Adjust dz_bottom_min ±0.1m |

#### Suggestion Mode
| Key | Action |
|-----|--------|
| `1`-`3` | Select algorithm |
| `+`/`-` | Adjust target levels |
| `[`/`]` | Adjust surface Δz |
| `<`/`>` | Adjust number of anchors |
| `↑`/`↓` | Adjust shallow levels |
| `Enter` | Accept suggestions |
| `Esc` | Cancel |

#### Panel Resize
| Key | Action |
|-----|--------|
| `{`/`}` | Shrink/expand left panel |
| Mouse drag | Drag divider to resize |

### Example Workflow

1. **Load mesh**: `vqs-designer -g mesh.gr3`
2. **Get suggestions**: Press `S`, adjust parameters
3. **Accept**: Press `Enter` (switches to Mesh Summary view)
4. **Fine-tune**: Edit individual anchors with `e`
5. **Adjust stretching**: Press `t` to try different functions, `i` for info
6. **Export**: Press `E`, then `Enter` to write `vgrid.in`

---

## Other Binaries

| Binary | Purpose |
|--------|---------|
| `gen_sz` | Generate SZ (sigma-z) vertical grids |
| `gen_vqs` | Generate VQS grids from CLI arguments |
| `vqs-designer` | Interactive TUI for designing VQS master grids |

## gen_sz

Generate sigma-z coordinate grids with configurable stretching.

```bash
cargo run --release --bin gen_sz -- /path/to/hgrid.gr3 \
    --slevels=20 \
    --theta-f=5 \
    --theta-b=0.7 \
    --critical-depth=5. \
    --show-plot \
    -o vgrid.in
```

![sz-20levels](./assets/sz_20levels.png)

## gen_vqs

Generate VQS grids using CLI arguments (for scripting/automation).

```bash
cargo run --release --bin gen_vqs -- /path/to/hgrid.gr3 \
    --depths 50 100 200 500 1000 \
    --nlevels 10 15 20 25 30 \
    --transform s \
    --theta-f=3. \
    --theta-b=0.5 \
    --dz-bottom-min=1. \
    -o vgrid.in
```

### Parameters

| Parameter | Description |
|-----------|-------------|
| `--transform` | Stretching function: `s`, `quadratic`, `shchepetkin2005`, `shchepetkin2010`, `geyer` |
| `--theta-f` | Surface/bottom focusing intensity (0.1-20) |
| `--theta-b` | Bottom layer focusing weight (0-1 for S, 0-4 for ROMS) |
| `--theta-s` | Surface stretching (0-10, ROMS transforms) |
| `--hc` | Critical depth in meters (ROMS transforms) |
| `--a-vqs0` | Stretching amplitude (-1 to 1, Quadratic) |
| `--dz-bottom-min` | Minimum bottom layer thickness |

---

## Compilation

This crate depends on libproj. Use conda to provide dependencies:

```bash
conda create -n schismrs
conda activate schismrs
conda install -c conda-forge compilers clang libclang proj

PROJ_SYS_STATIC=1 \
LD_LIBRARY_PATH=$CONDA_PREFIX/lib:$LD_LIBRARY_PATH \
PKG_CONFIG_PATH=$CONDA_PREFIX/lib/pkgconfig:$PKG_CONFIG_PATH \
cargo build --release
```

## License

`SPDX-License-Identifier: LicenseRef-schismrs-license`
