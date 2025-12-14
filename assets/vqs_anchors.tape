# VQS Designer - Anchor View
Output vqs_designer_anchors.gif

Set Shell bash
Set Theme "Dracula"
Set Width 1200
Set Height 700
Set FontSize 14
Set Padding 10

Type "/home/jreniel/schismrs/target/release/vqs-designer -g /home/jreniel/schismrs/src/schismrs/dev/hgrid.gr3"
Enter
Sleep 3s

# Accept suggestions to populate the table
Enter
Sleep 2s

# Switch to anchor view
Type "v"
Sleep 2s

# Navigate and show editing
Down
Sleep 500ms
Down
Sleep 500ms

Type "q"
Sleep 500ms
