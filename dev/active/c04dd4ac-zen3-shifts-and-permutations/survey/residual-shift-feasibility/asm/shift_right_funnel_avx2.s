shift_right_funnel_avx2:
	.cfi_startproc
	mov	eax, ecx
	mov	r8d, 64
	sub	r8d, ecx
	mov	r9, rdx
	not	r9
	add	r9, rsi
	lea	rcx, [rdx + 8]
	xor	r10d, r10d
	sub	rsi, rcx
	cmovb	rsi, r10
	jb	.LBB8_3
	vmovd	xmm0, eax
	vmovd	xmm1, r8d
	lea	rcx, [rdi + 8*rdx]
	add	rcx, 32
	xor	r10d, r10d
	.p2align	4
.LBB8_2:
	vmovdqu	ymm2, ymmword ptr [rcx + 8*r10 - 32]
	vpblendd	ymm3, ymm2, ymmword ptr [rcx + 8*r10], 3
	vpermq	ymm3, ymm3, 57
	vpsrlq	ymm2, ymm2, xmm0
	vpsllq	ymm3, ymm3, xmm1
	vpor	ymm2, ymm3, ymm2
	vmovdqu	ymmword ptr [rdi + 8*r10], ymm2
	add	r10, 4
	cmp	r10, rsi
	jbe	.LBB8_2
.LBB8_3:
	cmp	r10, r9
	jae	.LBB8_6
	and	eax, 63
	and	r8d, 63
	lea	rdx, [rdi + 8*rdx]
	add	rdx, 8
	.p2align	4
.LBB8_5:
	mov	rsi, qword ptr [rdx + 8*r10 - 8]
	mov	r11, qword ptr [rdx + 8*r10]
	mov	ecx, eax
	shr	rsi, cl
	mov	ecx, r8d
	shl	r11, cl
	or	r11, rsi
	mov	qword ptr [rdi + 8*r10], r11
	inc	r10
	cmp	r10, r9
	jb	.LBB8_5
.LBB8_6:
	vzeroupper
	ret
.Lfunc_end8:
	.size	shift_right_funnel_avx2, .Lfunc_end8-shift_right_funnel_avx2
