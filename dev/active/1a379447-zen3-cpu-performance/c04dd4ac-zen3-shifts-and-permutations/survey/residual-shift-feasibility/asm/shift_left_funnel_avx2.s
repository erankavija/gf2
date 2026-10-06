shift_left_funnel_avx2:
	.cfi_startproc
	mov	eax, ecx
	mov	r8d, 64
	sub	r8d, ecx
	lea	rcx, [rdx + 8]
	cmp	rsi, rcx
	jb	.LBB5_3
	vmovd	xmm0, eax
	vmovd	xmm1, r8d
	lea	r9, [rdi - 32]
	lea	r11, [8*rdx]
	mov	r10, r9
	sub	r10, r11
	.p2align	4
.LBB5_2:
	vmovdqu	ymm2, ymmword ptr [r10 + 8*rsi]
	vpblendd	ymm3, ymm2, ymmword ptr [r10 + 8*rsi - 32], 192
	vpermq	ymm3, ymm3, 147
	vpsllq	ymm2, ymm2, xmm0
	vpsrlq	ymm3, ymm3, xmm1
	vpor	ymm2, ymm3, ymm2
	vmovdqu	ymmword ptr [r9 + 8*rsi], ymm2
	add	rsi, -4
	cmp	rsi, rcx
	jae	.LBB5_2
.LBB5_3:
	lea	r9, [rdx + 1]
	cmp	rsi, r9
	jbe	.LBB5_6
	and	eax, 63
	and	r8d, 63
	add	rdi, -8
	shl	rdx, 3
	mov	r10, rdi
	sub	r10, rdx
	.p2align	4
.LBB5_5:
	mov	rdx, qword ptr [r10 + 8*rsi - 8]
	mov	r11, qword ptr [r10 + 8*rsi]
	mov	ecx, eax
	shl	r11, cl
	mov	ecx, r8d
	shr	rdx, cl
	or	rdx, r11
	mov	qword ptr [rdi + 8*rsi], rdx
	dec	rsi
	cmp	rsi, r9
	ja	.LBB5_5
.LBB5_6:
	vzeroupper
	ret
.Lfunc_end5:
	.size	shift_left_funnel_avx2, .Lfunc_end5-shift_left_funnel_avx2
