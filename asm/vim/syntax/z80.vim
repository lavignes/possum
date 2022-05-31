syn case ignore

set isk=a-z,A-Z,48-57,',.,_,@

syn sync fromstart

" Common Z80 Assembly instructions
syn keyword z80Inst adc add and bit call ccf cp cpd cpdr cpi cpir cpl
syn keyword z80Inst daa dec di djnz ei ex exx halt im in
syn keyword z80Inst inc ind ini indr inir jp jr ld ldd lddr ldi ldir
syn keyword z80Inst neg nop or otdr otir out outd outi push pop
syn keyword z80Inst res ret reti retn rl rla rlc rlca rld
syn keyword z80Inst rr rra rrc rrca rrd rst sbc scf set sla sll sra
syn keyword z80Inst srl sub xor

" Registers
syn keyword z80Reg af af' bc de hl ix ixh ixl iy iyh iyl
syn keyword z80Reg sp pc a b c d e f h l i r

" Directives
syn keyword z80PreProc @org @here @macro @endm @enum @ende @struct @ends
syn keyword z80PreProc @def @if @ifdef @ifndef @else @endif @echo @die
syn keyword z80PreProc @assert @db @dw @ds @include

" Strings
syn region z80String start=/"/ skip=/\\"/ end=/"/ oneline
syn region z80String start=/'/ end=/'/ oneline

" Labels
syn match z80Lbl "[A-Z_.?][A-Z_.?0-9]*:\="
syn region z80Lbl2 start="(" end=")" oneline contains=z80Number,z80Lbl,z80Lbl2,z80Other

" Operators
syn match z80Other "[~!%^&*-=|<>/?]"

" Numbers
syn match z80Number "\<\$[0-9a-fA-F]\+\>"
syn match z80Number "\<%[01]\+\>"
syn match z80Number "\<\d\+\>"

" Indirect register access
syn region z80Reg start=/(ix/ end=/)/ keepend oneline contains=z80Lbl,z80Number,z80Reg,z80Other
syn region z80Reg start=/(iy/ end=/)/ keepend oneline contains=z80Lbl,z80Number,z80Reg,z80Other
syn match z80Reg "(b\=c)"
syn match z80Reg "(de)"
syn match z80Reg "(hl)"
syn match z80Reg "(sp)"

" Todo
syn keyword	cTodo		contained TODO FIXME XXX

" Comments
syn match z80Comment ";.*$" contains=cTodo

hi def link cTodo		Todo

" Define the default highlighting.
" For version 5.7 and earlier: only when not done already
" For version 5.8 and later: only when an item doesn't have highlighting yet
if version >= 508 || !exists("did_z80_syntax_inits")
if version < 508
let did_z80_syntax_inits = 1
command -nargs=+ HiLink hi link <args>
else
command -nargs=+ HiLink hi def link <args>
endif

HiLink z80Reg Constant
HiLink z80Lbl Type
HiLink z80Lbl2 Type
HiLink z80Comment Comment
HiLink z80Inst Statement
HiLink z80Include Include
HiLink z80PreProc PreProc
HiLink z80Number Number
HiLink z80String String
HiLink z80Other Operator
HiLink z80Todo Todo

delcommand HiLink
endif

let b:current_syntax = "z80"
set ts=8
set sw=8
set noet

