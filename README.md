# schismrs-vgrid

Vertical grid generation for SCHISM ocean model.

## Binaries

| Binary | Purpose |
|--------|---------|
| `gen_sz` | Generate SZ (sigma-z) vertical grids |
| `gen_vqs` | Generate VQS (variable quadratic/S-coordinate) grids |
| `vqs-designer` | Interactive TUI for designing VQS master grids |

Use `--help` for detailed options. Use `--release` for 10x speedups.

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

Generate VQS grids using the HSM (Hierarchical Sigma Method) master grid specification.

```bash
cargo run --release --bin gen_vqs -- /path/to/hgrid.gr3 \
    --transform s \
    --dz-bottom-min=1. \
    --a-vqs0=-0.3 \
    --theta-b=0. \
    --theta-f=3. \
    --show-zmas-plot \
    -o vgrid.in \
    hsm --depths 50 60 80 110 150 200 260 330 410 500 600 8426 \
        --nlevels 21 22 23 24 25 26 27 28 29 30 31 32
```

### Parameters

| Parameter | Description |
|-----------|-------------|
| `--transform` | Stretching function: `s` (S-transform) or `quadratic` |
| `--theta-f` | Surface/bottom focusing intensity (0.1-20) |
| `--theta-b` | Bottom layer focusing weight (0-1) |
| `--a-vqs0` | Stretching amplitude (-1 to 1) |
| `--dz-bottom-min` | Minimum bottom layer thickness |

### HSM Master Grid

The `--depths` and `--nlevels` arrays define the master grid:
- Each depth threshold maps to a number of vertical levels
- Levels must be non-decreasing with depth (monotonicity constraint)
- Nodes are assigned to the appropriate master grid based on their depth

## vqs-designer

Interactive TUI for designing VQS master grids using the LSC² framework.

```bash
cargo run --release -p schismrs-vgrid --bin vqs-designer
```

Features:
- Dynamic construction table (depths × min Δz)
- Click or keyboard to select anchor points
- Monotonicity enforcement (levels never decrease with depth)
- Real-time zone statistics with stretching preview
- Export to CLI args, YAML config, or vgrid.in

## Compilation

This crate depends on libproj (C++ library). Use conda to provide dependencies:

```bash
conda create -n schismrs
conda activate schismrs
conda install -c conda-forge compilers clang libclang proj

PROJ_SYS_STATIC=1 \
LD_LIBRARY_PATH=$CONDA_PREFIX/lib:$LD_LIBRARY_PATH \
PKG_CONFIG_PATH=$CONDA_PREFIX/lib/pkgconfig:$PKG_CONFIG_PATH \
cargo build --release
```

The resulting binary is statically compiled and doesn't require conda at runtime.

## License

`SPDX-License-Identifier: LicenseRef-schismrs-license`
