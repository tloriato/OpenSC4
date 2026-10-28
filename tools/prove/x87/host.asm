; x87host(op, cw, st, out): run one x87 transcendental on the host CPU (x64 still has the x87 unit).
; st:  8 x 10-byte registers in stack order (ST0 first) plus the 2-byte status word at +80
; out: same layout, the stack after the instruction, with its status word
; op: 0 fsin, 1 fcos, 2 fptan, 3 fpatan, 4 fyl2x, 5 f2xm1, 6 fsincos, 7 fyl2xp1, 8 fscale, 9 fprem, 10 fprem1
.code
x87host PROC
    sub rsp, 8
    mov word ptr [rsp], dx
    fninit
    fldcw word ptr [rsp]
    ; load ST7..ST0 so the stack ends up as given (only ST0 and ST1 matter for these instructions)
    fld tbyte ptr [r8 + 10]
    fld tbyte ptr [r8]
    cmp ecx, 0
    jne @1
    fsin
    jmp done
@1: cmp ecx, 1
    jne @2
    fcos
    jmp done
@2: cmp ecx, 2
    jne @3
    fptan
    jmp done
@3: cmp ecx, 3
    jne @4
    fpatan
    jmp done
@4: cmp ecx, 4
    jne @5
    fyl2x
    jmp done
@5: cmp ecx, 5
    jne @6
    f2xm1
    jmp done
@6: cmp ecx, 6
    jne @7
    fsincos
    jmp done
@7: cmp ecx, 7
    jne @8
    fyl2xp1
    jmp done
@8: cmp ecx, 8
    jne @9
    fscale
    jmp done
@9: cmp ecx, 9
    jne @10
    fprem
    jmp done
@10:
    fprem1
done:
    fnstsw ax
    mov word ptr [r9 + 80], ax
    ; store up to three registers (pushes leave three: fptan/fsincos on a two-deep stack)
    fstp tbyte ptr [r9]
    fstp tbyte ptr [r9 + 10]
    fstp tbyte ptr [r9 + 20]
    fninit
    add rsp, 8
    ret
x87host ENDP
END
