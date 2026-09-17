shift_right_funnel_bmi2_dp:
	.cfi_startproc
	mov	eax, ecx
	mov	r8, rdx
	not	r8
	add	r8, rsi
	cmp	r8, 2
	jae	.LBB9_6
	xor	esi, esi
	jmp	.LBB9_2
.LBB9_6:
	mov	ecx, eax
	and	ecx, 63
	lea	r9, [rdi + 8*rdx]
	add	r9, 16
	xor	r10d, r10d
	.p2align	4
.LBB9_7:
	mov	rsi, qword ptr [r9 + 8*r10 - 16]
	mov	r11, qword ptr [r9 + 8*r10 - 8]
	shrd	rsi, r11, cl
	mov	qword ptr [rdi + 8*r10], rsi
	mov	rsi, qword ptr [r9 + 8*r10 - 8]
	mov	r11, qword ptr [r9 + 8*r10]
	shrd	rsi, r11, cl
	mov	qword ptr [rdi + 8*r10 + 8], rsi
	lea	rsi, [r10 + 2]
	add	r10, 4
	cmp	r10, r8
	mov	r10, rsi
	jbe	.LBB9_7
.LBB9_2:
	cmp	rsi, r8
	jae	.LBB9_5
	and	eax, 63
	lea	rdx, [rdi + 8*rdx]
	add	rdx, 8
	.p2align	4
.LBB9_4:
	mov	r9, qword ptr [rdx + 8*rsi - 8]
	mov	r10, qword ptr [rdx + 8*rsi]
	mov	ecx, eax
	shrd	r9, r10, cl
	mov	qword ptr [rdi + 8*rsi], r9
	inc	rsi
	cmp	rsi, r8
	jb	.LBB9_4
.LBB9_5:
	ret
.Lfunc_end9:
	.size	shift_right_funnel_bmi2_dp, .Lfunc_end9-shift_right_funnel_bmi2_dp
