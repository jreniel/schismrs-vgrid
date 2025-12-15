# VQS Designer - Complete Workflow Demo
# Demonstrates the unified split-screen interface with suggestion mode
Output vqs_designer_workflow.gif

Set Shell bash
Set Theme "Dracula"
Set Width 1200
Set Height 700
Set FontSize 14
Set Padding 10

# Launch the designer with a mesh
Type "target/release/vqs-designer -g hgrid.gr3"
Sleep 1s
Enter
Sleep 2s

# Enter suggestion mode (S)
Type "S"
Sleep 1.5s

# Show different algorithms (1-3)
Type "2"
Sleep 800ms
Type "3"
Sleep 800ms
Type "1"
Sleep 1s

# Adjust number of anchors (increase with >)
Type ">"
Sleep 300ms
Type ">"
Sleep 300ms
Type ">"
Sleep 300ms
Type ">"
Sleep 800ms

# Adjust target levels (+)
Type "+"
Sleep 300ms
Type "+"
Sleep 300ms
Type "+"
Sleep 800ms

# Cycle stretching type (t)
Type "t"
Sleep 800ms
Type "t"
Sleep 800ms

# Adjust stretching parameters
Type "b"
Sleep 300ms
Type "b"
Sleep 300ms
Type "b"
Sleep 800ms

# Adjust bottom min dz (z)
Type "z"
Sleep 300ms
Type "z"
Sleep 800ms

# Accept suggestions (Enter) - switches to Mesh Summary view
Enter
Sleep 1.5s

# Cycle through view modes (v)
Type "v"
Sleep 1s
Type "v"
Sleep 1s
Type "v"
Sleep 1s

# Navigate anchors to show selection
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms
Up
Sleep 800ms

# Show stretching info (i)
Type "i"
Sleep 2s
Escape
Sleep 500ms

# Open export modal (E)
Type "E"
Sleep 1s

# Export vgrid.in (Enter)
Enter
Sleep 800ms

# Confirm overwrite if needed (y)
Type "y"
Sleep 1s

# Quit (q)
Type "q"
Sleep 500ms
