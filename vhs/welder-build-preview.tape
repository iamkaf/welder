Output "vhs/welder-build-preview.gif"
Set FontSize 20
Set Width 1100
Set Height 650
Set Framerate 24
Set TypingSpeed 80ms

Type "cd examples/basic-pack"
Enter
Sleep 200ms
Type "rm -rf dist"
Enter
Sleep 200ms
Type "welder build"
Enter
Sleep 1s
Type "welder preview"
Enter
Sleep 2s
Type "find dist -maxdepth 3 -type f | sort"
Enter
Sleep 2s
