shift_left_funnel_bmi2_dp:
	.cfi_startproc
	mov	eax, ecx
	lea	r8, [rdx + 3]
	cmp	rsi, r8
	jb	.LBB6_3
	mov	ecx, eax
	and	ecx, 63
	lea	r10, [8*rdx]
	mov	r9, rdi
	sub	r9, r10
	add	r9, -8
	.p2align	4
.LBB6_2:
	mov	r10, qword ptr [r9 + 8*rsi - 8]
	mov	r11, qword ptr [r9 + 8*rsi]
	shld	r11, r10, cl
	mov	qword ptr [rdi + 8*rsi - 8], r11
	mov	r10, qword ptr [r9 + 8*rsi - 16]
	mov	r11, qword ptr [r9 + 8*rsi - 8]
	shld	r11, r10, cl
	mov	qword ptr [rdi + 8*rsi - 16], r11
	add	rsi, -2
	cmp	rsi, r8
	jae	.LBB6_2
.LBB6_3:
	lea	r8, [rdx + 1]
	cmp	rsi, r8
	jbe	.LBB6_6
	and	eax, 63
	add	rdi, -8
	shl	rdx, 3
	mov	r9, rdi
	sub	r9, rdx
	.p2align	4
.LBB6_5:
	mov	rdx, qword ptr [r9 + 8*rsi - 8]
	mov	r10, qword ptr [r9 + 8*rsi]
	mov	ecx, eax
	shld	r10, rdx, cl
	mov	qword ptr [rdi + 8*rsi], r10
	dec	rsi
	cmp	rsi, r8
	ja	.LBB6_5
.LBB6_6:
	ret
.Lfunc_end6:
	.size	shift_left_funnel_bmi2_dp, .Lfunc_end6-shift_left_funnel_bmi2_dp
