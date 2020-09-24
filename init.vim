:compiler rustc

 :BuildToolsAdd 'make', {'makeprg': ':make', 'errorformat': '

aug rust
	au!
	au BufWritePost *.rs silent exec '!cargo fmt -- ' . expand("%") | e
aug END
