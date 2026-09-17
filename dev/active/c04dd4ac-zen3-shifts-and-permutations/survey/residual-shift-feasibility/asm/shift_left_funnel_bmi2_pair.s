shift_left_funnel_bmi2_pair:
	.cfi_startproc
	push	r14
	.cfi_def_cfa_offset 16
	push	rbx
	.cfi_def_cfa_offset 24
	.cfi_offset rbx, -24
	.cfi_offset r14, -16
	mov	eax, 64
	sub	eax, ecx
	lea	r8, [rdx + 3]
	cmp	rsi, r8
	jb	.LBB7_3
	mov	r9d, ecx
	and	r9d, 63
	mov	r10d, eax
	and	r10d, 63
	lea	rbx, [8*rdx]
	mov	r11, rdi
	sub	r11, rbx
	add	r11, -8
	.p2align	4
.LBB7_2:
	shlx	rbx, qword ptr [r11 + 8*rsi], r9
	shrx	r14, qword ptr [r11 + 8*rsi - 8], r10
	or	r14, rbx
	mov	qword ptr [rdi + 8*rsi - 8], r14
	shlx	rbx, qword ptr [r11 + 8*rsi - 8], r9
	shrx	r14, qword ptr [r11 + 8*rsi - 16], r10
	or	r14, rbx
	mov	qword ptr [rdi + 8*rsi - 16], r14
	add	rsi, -2
	cmp	rsi, r8
	jae	.LBB7_2
.LBB7_3:
	lea	r8, [rdx + 1]
	cmp	rsi, r8
	jbe	.LBB7_6
	and	ecx, 63
	and	eax, 63
	add	rdi, -8
	shl	rdx, 3
	mov	r9, rdi
	sub	r9, rdx
	.p2align	4
.LBB7_5:
	shlx	rdx, qword ptr [r9 + 8*rsi], rcx
	shrx	r10, qword ptr [r9 + 8*rsi - 8], rax
	or	r10, rdx
	mov	qword ptr [rdi + 8*rsi], r10
	dec	rsi
	cmp	rsi, r8
	ja	.LBB7_5
.LBB7_6:
	pop	rbx
	.cfi_def_cfa_offset 16
	pop	r14
	.cfi_def_cfa_offset 8
	ret
.Lfunc_end7:
	.size	shift_left_funnel_bmi2_pair, .Lfunc_end7-shift_left_funnel_bmi2_pair
