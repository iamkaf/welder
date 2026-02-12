Output "vhs/welder-package-publish.gif"
Set FontSize 20
Set Width 1100
Set Height 650
Set Framerate 24
Set TypingSpeed 80ms

Type "cd examples/basic-pack"
Enter
Sleep 200ms
Type "welder package"
Enter
Sleep 1s
Type "ls -la dist/package"
Enter
Sleep 1s
Type "welder publish --dry-run"
Enter
Sleep 2s
