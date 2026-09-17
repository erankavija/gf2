shift_right_funnel_bmi2_pair:
	.cfi_startproc
	push	r14
	.cfi_def_cfa_offset 16
	push	rbx
	.cfi_def_cfa_offset 24
	.cfi_offset rbx, -24
	.cfi_offset r14, -16
	mov	eax, 64
	sub	eax, ecx
	mov	r8, rdx
	not	r8
	add	r8, rsi
	cmp	r8, 2
	jae	.LBB10_6
	xor	esi, esi
	jmp	.LBB10_2
.LBB10_6:
	mov	r9d, ecx
	and	r9d, 63
	mov	r10d, eax
	and	r10d, 63
	lea	r11, [rdi + 8*rdx]
	add	r11, 16
	xor	ebx, ebx
	.p2align	4
.LBB10_7:
	shrx	rsi, qword ptr [r11 + 8*rbx - 16], r9
	shlx	r14, qword ptr [r11 + 8*rbx - 8], r10
	or	r14, rsi
	mov	qword ptr [rdi + 8*rbx], r14
	shrx	rsi, qword ptr [r11 + 8*rbx - 8], r9
	shlx	r14, qword ptr [r11 + 8*rbx], r10
	or	r14, rsi
	mov	qword ptr [rdi + 8*rbx + 8], r14
	lea	rsi, [rbx + 2]
	add	rbx, 4
	cmp	rbx, r8
	mov	rbx, rsi
	jbe	.LBB10_7
.LBB10_2:
	cmp	rsi, r8
	jae	.LBB10_5
	and	ecx, 63
	and	eax, 63
	lea	rdx, [rdi + 8*rdx]
	add	rdx, 8
	.p2align	4
.LBB10_4:
	shrx	r9, qword ptr [rdx + 8*rsi - 8], rcx
	shlx	r10, qword ptr [rdx + 8*rsi], rax
	or	r10, r9
	mov	qword ptr [rdi + 8*rsi], r10
	inc	rsi
	cmp	rsi, r8
	jb	.LBB10_4
.LBB10_5:
	pop	rbx
	.cfi_def_cfa_offset 16
	pop	r14
	.cfi_def_cfa_offset 8
	ret
.Lfunc_end10:
	.size	shift_right_funnel_bmi2_pair, .Lfunc_end10-shift_right_funnel_bmi2_pair
