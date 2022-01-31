start:
    

dma_print:
    ld bc, $0001
    
    ld a, $7d  ; a -> b
    out (c), a
    
    ld a, (hl) ; from $0000
    out (c), a
    inc hl
    ld a, (hl)
    out (c), a
    
    out (c), e ; len 5
    out (c), d
    
    ld a, $
    
    
    
hello:
    db "Hello World!", 10
hello_len equ $ - hello


