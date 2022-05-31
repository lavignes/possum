syn case ignore

" Common Z80 Assembly instructions
syn keyword z80Instruction adc add and bit ccf cp cpd cpdr cpi cpir cpl
syn keyword z80Instruction daa di djnz ei exx halt im in
syn keyword z80Instruction ind ini indr inir jp jr ld ldd lddr ldi ldir
syn keyword z80Instruction neg nop or otdr otir out outd outi
syn keyword z80Instruction res rl rla rlc rlca rld
syn keyword z80Instruction rr rra rrc rrca rrd sbc scf set sla sll sra
syn keyword z80Instruction srl sub xor
" syn keyword z80Instruction push pop call ret reti retn inc dec ex rst

" Any other stuff
syn match z80Identifier		"[a-z_][a-z0-9_]*"

" Instructions changing stack
syn keyword z80SpecInst push pop call ret reti retn rst
syn match z80Instruction "\<inc\>"
syn match z80Instruction "\<dec\>"
syn match z80Instruction "\<ex\>"
syn match z80SpecInst "\<inc\s\+sp\>"me=s+3
syn match z80SpecInst "\<dec\s\+sp\>"me=s+3
syn match z80SpecInst "\<ex\s\+(\s*sp\s*)\s*,\s*hl\>"me=s+2

"Labels
syn match z80Label		"[a-z_\.][a-z0-9_\.]*:?"

" PreProcessor commands
syn match z80PreProc	"@org"
syn match z80PreProc	"@here"
syn match z80PreProc	"@macro"
syn match z80PreProc	"@endm"
syn match z80PreProc	"@enum"
syn match z80PreProc	"@ende"
syn match z80PreProc	"@struct"
syn match z80PreProc	"@ends"
syn match z80PreProc	"@def"
syn match z80PreProc	"@db"
syn match z80PreProc	"@dw"
syn match z80PreProc	"@ds"
syn match z80PreProc	"@echo"
syn match z80Include	"@include"
syn match z80Include	"@incbin"
syn match z80PreCondit	"@if"
syn match z80PreCondit	"@ifdef"
syn match z80PreCondit	"@ifndef"
syn match z80PreCondit	"@else"
syn match z80PreCondit	"@endif"
syn match z80PreCondit	"@die"
syn match z80PreCondit	"@assert"

" Common strings
syn match z80String		"\".*\""
syn match z80String		"\'.*\'"

" Numbers
syn match z80Number		"[0-9]\+"
syn match z80Number		"\$[0-9a-fA-F]\+"
syn match z80Number		"%[01]\+"
" Comments
syn match z80Comment	";.*"

syn case match

" Define the default highlighting.
" Only when an item doesn't have highlighting yet

hi def link z80Section		Special
hi def link z80Label		Label
hi def link z80Comment		Comment
hi def link z80Instruction	Statement
hi def link z80SpecInst		Statement
hi def link z80Include		Include
hi def link z80PreCondit	PreCondit
hi def link z80PreProc		PreProc
hi def link z80Number		Number
hi def link z80String		String

let b:current_syntax = "z80"
set ts=8
set sw=8
set noet
