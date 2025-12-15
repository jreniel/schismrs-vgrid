# VQS Designer - Anchor Editing and Stretching Functions
# Demonstrates manual anchor editing and stretching parameter adjustment
Output vqs_designer_anchors.gif

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

# Add anchors manually (a)
Type "a"
Sleep 500ms
Type "10"
Enter
Sleep 300ms
Type "5"
Enter
Sleep 800ms

Type "a"
Sleep 500ms
Type "50"
Enter
Sleep 300ms
Type "15"
Enter
Sleep 800ms

Type "a"
Sleep 500ms
Type "100"
Enter
Sleep 300ms
Type "25"
Enter
Sleep 1s

# Navigate and edit an anchor (e)
Down
Sleep 400ms
Type "e"
Sleep 500ms
Type "30"
Enter
Sleep 300ms
Type "10"
Enter
Sleep 800ms

# Cycle through stretching functions (t)
Type "t"
Sleep 800ms
Type "t"
Sleep 800ms
Type "t"
Sleep 800ms
Type "t"
Sleep 800ms
Type "t"
Sleep 800ms

# Show stretching info for current function (i)
Type "i"
Sleep 2s
Escape
Sleep 500ms

# Switch to S-transform and adjust parameters
Type "t"
Sleep 500ms
Type "t"
Sleep 500ms

# Adjust theta_f (f)
Type "f"
Sleep 300ms
Type "f"
Sleep 300ms
Type "f"
Sleep 500ms

# Adjust theta_b (b)
Type "b"
Sleep 300ms
Type "b"
Sleep 500ms

# Show Multi-Depth view to see stats
Type "v"
Sleep 1.5s

# Delete an anchor (d)
Type "d"
Sleep 800ms

Type "q"
Sleep 500ms
