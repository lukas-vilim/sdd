:compiler rustc
call BuildToolsClear()
call BuildToolsAdd(':! cargo run')
call BuildToolsAdd(':! cargo build')
call BuildToolsAdd(':Dispatch cargo run')
call BuildToolsAdd(':! cargo run -- -a "127.0.0.1:2001" -o "out.txt"')

aug rust
	au!
	au BufWritePost *.rs silent exec '!cargo fmt -- ' . expand("%") | e
aug END
