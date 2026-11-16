# Records docs/images/demo.gif and docs/images/ui.png for the README and
# Terminal Trove. Run from the repo root with `demo/record.sh`.
Output docs/images/demo.gif

Require whambam

Set Shell "bash"
Set FontSize 15
Set Width 1300
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

Hide
Type "export PS1='$ '" Enter
Type "clear" Enter
Show

Type "whambam http://127.0.0.1:8080/api -z 24s -c 64 --qps 1000"
Sleep 600ms
Enter
Sleep 4s
# Raise the rate live: ] is +10% per press.
Type@350ms "]]]]]"
Sleep 3s
Type@350ms "]]]]]"
Sleep 3s
Screenshot docs/images/ui.png
Sleep 200ms
Screenshot website/public/images/ui-tab-1.png
Sleep 200ms
Screenshot website/public/images/ui.png
Sleep 500ms
Type "2"
Sleep 4s
Screenshot website/public/images/ui-tab-2.png
Sleep 500ms
Type "3"
Sleep 3s
Screenshot website/public/images/ui-tab-3.png
Sleep 500ms
Type "1"
Sleep 7s
Type "q"
Sleep 3s
