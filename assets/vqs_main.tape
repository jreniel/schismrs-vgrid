# VQS Designer - Unified Interface Overview
# Shows the split-screen layout with anchor list and profile views
Output vqs_designer_main.gif

Set Shell bash
Set Theme "Dracula"
Set Width 1200
Set Height 700
Set FontSize 14
Set Padding 10

# Launch with mesh
Type "target/release/vqs-designer -g hgrid.gr3"
Sleep 1s
Enter
Sleep 2s

# Enter suggestion mode and quickly accept defaults
Type "S"
Sleep 1s
Enter
Sleep 1.5s

# Now in unified view with Mesh Summary (default after accepting)
# Show navigation
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms

# Cycle to Single Depth view (v)
Type "v"
Sleep 1.5s

# Navigate to show layer bars
Up
Sleep 500ms
Up
Sleep 500ms

# Cycle to Multi-Depth view (v)
Type "v"
Sleep 1.5s

# Navigate
Down
Sleep 400ms
Down
Sleep 400ms

# Back to Mesh Summary (v)
Type "v"
Sleep 1.5s

# Resize panels with { key
Type "{"
Sleep 300ms
Type "{"
Sleep 300ms
Type "{"
Sleep 500ms

# Expand back with }
Type "}"
Sleep 300ms
Type "}"
Sleep 300ms
Type "}"
Sleep 800ms

# Show help overlay (?)
Type "?"
Sleep 2s
Escape
Sleep 500ms

Type "q"
Sleep 500ms
