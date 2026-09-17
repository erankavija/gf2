	.intel_syntax noprefix
	.file	"ldpc_intrinsics_probe.ba30df1800a1b3f1-cgu.0"
	.section	.text._ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E,@function
_ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E:
.Lfunc_begin0:
	.cfi_startproc
	.file	1 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/ptr/mod.rs"
	.loc	1 547 14 prologue_end
	vmovups	ymm0, ymmword ptr [rsi]
.Ltmp0:
	.file	2 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/../../stdarch/crates/core_arch/src/x86/avx2.rs"
	.loc	2 2456 14
	vpermps	ymm0, ymm0, ymmword ptr [rdi]
.Ltmp1:
	.loc	1 547 14
	vmovups	ymmword ptr [rdx], ymm0
.Ltmp2:
	.file	3 "/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-9fb40c83/dev/active/f63a2464/survey/intrinsics-probe" "src/lib.rs"
	.loc	3 246 2
	vzeroupper
	ret
.Ltmp3:
.Lfunc_end0:
	.size	_ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E, .Lfunc_end0-_ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E
	.cfi_endproc
	.file	4 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/../../stdarch/crates/core_arch/src/x86/avx.rs"
	.file	5 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/ptr/mut_ptr.rs"

	.section	.text._ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE,@function
_ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE:
.Lfunc_begin1:
	.cfi_startproc
	.file	6 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/../../stdarch/crates/core_arch/src/simd.rs"
	.loc	6 56 18 prologue_end
	vmovd	xmm0, esi
	vpbroadcastb	ymm0, xmm0
.Ltmp4:
	.loc	3 124 35
	neg	sil
.Ltmp5:
	.loc	6 56 18
	vmovd	xmm1, esi
	vpbroadcastb	ymm1, xmm1
.Ltmp6:
	.loc	6 16 5
	vpminsb	ymm0, ymm0, ymmword ptr [rdi]
.Ltmp7:
	.loc	6 9 5
	vpmaxsb	ymm0, ymm1, ymm0
.Ltmp8:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp9:
	.loc	3 128 2
	vzeroupper
	ret
.Ltmp10:
.Lfunc_end1:
	.size	_ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE, .Lfunc_end1-_ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E,@function
_ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E:
.Lfunc_begin2:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp11:
	.loc	2 132 24
	vpaddsb	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp12:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp13:
	.loc	3 37 2
	vzeroupper
	ret
.Ltmp14:
.Lfunc_end2:
	.size	_ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E, .Lfunc_end2-_ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E,@function
_ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E:
.Lfunc_begin3:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp15:
	.loc	2 3370 24
	vpsubsb	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp16:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp17:
	.loc	3 53 2
	vzeroupper
	ret
.Ltmp18:
.Lfunc_end3:
	.size	_ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E, .Lfunc_end3-_ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E,@function
_ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E:
.Lfunc_begin4:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp19:
	.loc	2 70 17
	vpabsb	ymm1, ymmword ptr [rdx]
.Ltmp20:
	.loc	6 16 5
	vpminsb	ymm2, ymm0, ymm1
.Ltmp21:
	.loc	6 9 5
	vpmaxsb	ymm0, ymm0, ymm1
.Ltmp22:
	.loc	6 16 5
	vpminsb	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp23:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rcx], ymm2
.Ltmp24:
	.loc	1 547 14 is_stmt 0
	vmovdqu	ymmword ptr [r8], ymm0
.Ltmp25:
	.loc	3 82 2 is_stmt 1
	vzeroupper
	ret
.Ltmp26:
.Lfunc_end4:
	.size	_ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E, .Lfunc_end4-_ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E,@function
_ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E:
.Lfunc_begin5:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp27:
	.loc	2 144 24
	vpaddsw	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp28:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp29:
	.loc	3 143 2
	vzeroupper
	ret
.Ltmp30:
.Lfunc_end5:
	.size	_ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E, .Lfunc_end5-_ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE,@function
_ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE:
.Lfunc_begin6:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp31:
	.loc	2 3357 24
	vpsubsw	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp32:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp33:
	.loc	3 158 2
	vzeroupper
	ret
.Ltmp34:
.Lfunc_end6:
	.size	_ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE, .Lfunc_end6-_ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E,@function
_ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E:
.Lfunc_begin7:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp35:
	.loc	2 54 17
	vpabsw	ymm1, ymmword ptr [rdx]
.Ltmp36:
	.loc	6 16 5
	vpminsw	ymm2, ymm0, ymm1
.Ltmp37:
	.loc	6 9 5
	vpmaxsw	ymm0, ymm0, ymm1
.Ltmp38:
	.loc	6 16 5
	vpminsw	ymm0, ymm0, ymmword ptr [rsi]
.Ltmp39:
	.loc	1 547 14
	vmovdqu	ymmword ptr [rcx], ymm2
.Ltmp40:
	.loc	1 547 14 is_stmt 0
	vmovdqu	ymmword ptr [r8], ymm0
.Ltmp41:
	.loc	3 183 2 is_stmt 1
	vzeroupper
	ret
.Ltmp42:
.Lfunc_end7:
	.size	_ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E, .Lfunc_end7-_ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E
	.cfi_endproc

	.section	.rodata.cst4,"aM",@progbits,4
	.p2align	2, 0x0
.LCPI8_0:
	.long	2147483647
.LCPI8_1:
	.long	2147483648
	.section	.text._ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E,@function
_ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E:
.Lfunc_begin8:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovups	ymm0, ymmword ptr [rdi]
.Ltmp43:
	.loc	4 82 19
	vbroadcastss	ymm1, dword ptr [rip + .LCPI8_0]
	vandps	ymm1, ymm0, ymm1
.Ltmp44:
	.loc	1 547 14
	vmovups	ymmword ptr [rsi], ymm1
.Ltmp45:
	.loc	4 82 19
	vbroadcastss	ymm1, dword ptr [rip + .LCPI8_1]
	vandps	ymm0, ymm0, ymm1
.Ltmp46:
	.loc	1 547 14
	vmovups	ymmword ptr [rdx], ymm0
.Ltmp47:
	.loc	3 207 2
	vzeroupper
	ret
.Ltmp48:
.Lfunc_end8:
	.size	_ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E, .Lfunc_end8-_ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE,@function
_ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE:
.Lfunc_begin9:
	.cfi_startproc
	.loc	2 747 36 prologue_end
	vpxor	xmm0, xmm0, xmm0
	vpcmpgtb	ymm0, ymm0, ymmword ptr [rdi]
.Ltmp49:
	.loc	2 3344 35
	vpxor	ymm1, ymm0, ymmword ptr [rsi]
	.loc	2 3344 24 is_stmt 0
	vpsubb	ymm1, ymm1, ymm0
.Ltmp50:
	.loc	1 547 14 is_stmt 1
	vmovdqu	ymmword ptr [rdx], ymm0
.Ltmp51:
	.loc	1 547 14 is_stmt 0
	vmovdqu	ymmword ptr [rcx], ymm1
.Ltmp52:
	.loc	3 111 2 is_stmt 1
	vzeroupper
	ret
.Ltmp53:
.Lfunc_end9:
	.size	_ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE, .Lfunc_end9-_ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E,@function
_ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E:
.Lfunc_begin10:
	.cfi_startproc
	.loc	3 284 14 prologue_end
	vmovdqu	ymm0, ymmword ptr [rdi]
.Ltmp54:
	.file	7 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/../../stdarch/crates/core_arch/src/macros.rs"
	.loc	7 157 9
	vpermq	ymm1, ymm0, 198
.Ltmp55:
	.loc	7 157 9 is_stmt 0
	vpalignr	ymm0, ymm1, ymm0, 3
.Ltmp56:
	.loc	1 547 14 is_stmt 1
	vmovdqu	ymmword ptr [rsi], ymm0
.Ltmp57:
	.loc	3 285 2
	vzeroupper
	ret
.Ltmp58:
.Lfunc_end10:
	.size	_ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E, .Lfunc_end10-_ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE,@function
_ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE:
.Lfunc_begin11:
	.cfi_startproc
	.loc	1 547 14 prologue_end
	vmovups	ymm0, ymmword ptr [rdi]
.Ltmp59:
	.loc	4 871 14
	vxorps	xmm1, xmm1, xmm1
	vcmpltps	ymm0, ymm0, ymm1
.Ltmp60:
	.loc	1 547 14
	vmovups	ymmword ptr [rsi], ymm0
.Ltmp61:
	.loc	3 226 2
	vzeroupper
	ret
.Ltmp62:
.Lfunc_end11:
	.size	_ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE, .Lfunc_end11-_ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE,@function
_ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE:
.Lfunc_begin12:
	.cfi_startproc
	.loc	7 157 9 prologue_end
	vpermpd	ymm0, ymmword ptr [rdi], 78
.Ltmp63:
	.loc	1 547 14
	vmovups	ymmword ptr [rsi], ymm0
.Ltmp64:
	.loc	3 296 2
	vzeroupper
	ret
.Ltmp65:
.Lfunc_end12:
	.size	_ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE, .Lfunc_end12-_ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE,@function
_ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE:
.Lfunc_begin13:
	.loc	3 318 0
	.cfi_startproc
	push	rax
	.cfi_def_cfa_offset 16
	mov	byte ptr [rsp + 7], dil
.Ltmp66:
	.loc	3 319 9 prologue_end
	cmp	dil, -128
	je	.LBB13_2
.Ltmp67:
	.file	8 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/num/int_macros.rs"
	.loc	8 3608 16
	mov	eax, edi
	sar	al, 7
	xor	dil, al
	sub	dil, al
.Ltmp68:
	.loc	3 321 6
	mov	eax, edi
	.loc	3 321 6 epilogue_begin is_stmt 0
	pop	rcx
	.cfi_def_cfa_offset 8
	ret
.LBB13_2:
	.cfi_def_cfa_offset 16
.Ltmp69:
	.loc	3 319 9 is_stmt 1
	lea	rdx, [rip + .Lanon.fa1f34f804581cd11c810b97f41bac6b.0]
	lea	rcx, [rip + .Lanon.fa1f34f804581cd11c810b97f41bac6b.1]
	lea	r9, [rip + .Lanon.fa1f34f804581cd11c810b97f41bac6b.3]
	lea	rsi, [rsp + 7]
	mov	r8d, 79
	mov	edi, 1
	call	qword ptr [rip + _ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E@GOTPCREL]
.Ltmp70:
.Lfunc_end13:
	.size	_ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE, .Lfunc_end13-_ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE
	.cfi_endproc

	.section	.text._ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE,"ax",@progbits
	.globl	_ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE
	.p2align	4
	.type	_ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE,@function
_ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE:
.Lfunc_begin14:
	.cfi_startproc
	.file	9 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/cmp.rs"
	.loc	9 1076 12 prologue_end
	cmp	edx, edi
	mov	eax, edi
	cmovl	eax, edx
.Ltmp71:
	.loc	9 1037 12
	cmovle	edx, edi
.Ltmp72:
	.loc	9 1076 12
	cmp	edx, esi
	cmovge	edx, esi
.Ltmp73:
	.loc	3 327 6
	ret
.Ltmp74:
.Lfunc_end14:
	.size	_ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE, .Lfunc_end14-_ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE
	.cfi_endproc

	.section	".text._ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E","ax",@progbits
	.p2align	4
	.type	_ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E,@function
_ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E:
.Lfunc_begin15:
	.cfi_startproc
	.file	10 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/fmt/mod.rs"
	.loc	10 2865 71 prologue_end
	mov	rdi, qword ptr [rdi]
.Ltmp75:
	.loc	10 2398 9
	mov	eax, dword ptr [rsi + 16]
.Ltmp76:
	.file	11 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/fmt/num.rs"
	.loc	11 86 24
	test	eax, 33554432
	jne	.LBB15_3
	.loc	11 88 31
	test	eax, 67108864
	jne	.LBB15_2
	.loc	11 91 25
	jmp	qword ptr [rip + _RNvXs_NtNtNtCsgEmfK2I1SDS_4core3fmt3num3impaNtB8_7Display3fmt@GOTPCREL]
.LBB15_3:
	.loc	11 87 25
	jmp	qword ptr [rip + _RNvXsf_NtNtCsgEmfK2I1SDS_4core3fmt3numaNtB7_8LowerHex3fmt@GOTPCREL]
.LBB15_2:
	.loc	11 89 25
	jmp	qword ptr [rip + _RNvXsh_NtNtCsgEmfK2I1SDS_4core3fmt3numaNtB7_8UpperHex3fmt@GOTPCREL]
.Ltmp77:
.Lfunc_end15:
	.size	_ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E, .Lfunc_end15-_ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E
	.cfi_endproc

	.section	.text.unlikely._ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E,"ax",@progbits
	.globl	_ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E
	.type	_ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E,@function
_ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E:
.Lfunc_begin16:
	.file	12 "/rustc/59807616e1fa2540724bfbac14d7976d7e4a3860" "library/core/src/panicking.rs"
	.loc	12 384 0
	.cfi_startproc
	sub	rsp, 24
	.cfi_def_cfa_offset 32
	mov	rax, r9
	mov	r10, r8
	mov	r9, rcx
	lea	r8, [rsp + 8]
	mov	qword ptr [r8], rsi
	lea	rcx, [rsp + 16]
	mov	qword ptr [rcx], rdx
.Ltmp78:
	.loc	12 394 5 prologue_end
	lea	rdx, [rip + .Lanon.fa1f34f804581cd11c810b97f41bac6b.4]
	mov	rsi, r8
	mov	r8, rdx
	push	rax
	.cfi_adjust_cfa_offset 8
	push	r10
	.cfi_adjust_cfa_offset 8
	call	qword ptr [rip + _RNvNtCsgEmfK2I1SDS_4core9panicking19assert_failed_inner@GOTPCREL]
.Ltmp79:
.Lfunc_end16:
	.size	_ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E, .Lfunc_end16-_ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E
	.cfi_endproc

	.type	.Lanon.fa1f34f804581cd11c810b97f41bac6b.0,@object
	.section	.rodata..Lanon.fa1f34f804581cd11c810b97f41bac6b.0,"a",@progbits
.Lanon.fa1f34f804581cd11c810b97f41bac6b.0:
	.byte	128
	.size	.Lanon.fa1f34f804581cd11c810b97f41bac6b.0, 1

	.type	.Lanon.fa1f34f804581cd11c810b97f41bac6b.1,@object
	.section	.rodata..Lanon.fa1f34f804581cd11c810b97f41bac6b.1,"a",@progbits
.Lanon.fa1f34f804581cd11c810b97f41bac6b.1:
	.ascii	"the symmetric alphabet excludes i8::MIN"
	.size	.Lanon.fa1f34f804581cd11c810b97f41bac6b.1, 39

	.type	.Lanon.fa1f34f804581cd11c810b97f41bac6b.2,@object
	.section	.rodata.str1.1,"aMS",@progbits,1
.Lanon.fa1f34f804581cd11c810b97f41bac6b.2:
	.asciz	"src/lib.rs"
	.size	.Lanon.fa1f34f804581cd11c810b97f41bac6b.2, 11

	.type	.Lanon.fa1f34f804581cd11c810b97f41bac6b.3,@object
	.section	.data.rel.ro..Lanon.fa1f34f804581cd11c810b97f41bac6b.3,"aw",@progbits
	.p2align	3, 0x0
.Lanon.fa1f34f804581cd11c810b97f41bac6b.3:
	.quad	.Lanon.fa1f34f804581cd11c810b97f41bac6b.2
	.asciz	"\n\000\000\000\000\000\000\000?\001\000\000\t\000\000"
	.size	.Lanon.fa1f34f804581cd11c810b97f41bac6b.3, 24

	.type	.Lanon.fa1f34f804581cd11c810b97f41bac6b.4,@object
	.section	.data.rel.ro..Lanon.fa1f34f804581cd11c810b97f41bac6b.4,"aw",@progbits
	.p2align	3, 0x0
.Lanon.fa1f34f804581cd11c810b97f41bac6b.4:
	.asciz	"\000\000\000\000\000\000\000\000\b\000\000\000\000\000\000\000\b\000\000\000\000\000\000"
	.quad	_ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E
	.size	.Lanon.fa1f34f804581cd11c810b97f41bac6b.4, 32

	.section	.debug_abbrev,"",@progbits
	.byte	1
	.byte	17
	.byte	1
	.byte	37
	.byte	14
	.byte	19
	.byte	5
	.byte	3
	.byte	14
	.byte	16
	.byte	23
	.byte	27
	.byte	14
	.byte	17
	.byte	1
	.byte	85
	.byte	23
	.byte	0
	.byte	0
	.byte	2
	.byte	46
	.byte	0
	.byte	3
	.byte	14
	.byte	32
	.byte	11
	.byte	0
	.byte	0
	.byte	3
	.byte	46
	.byte	1
	.byte	17
	.byte	1
	.byte	18
	.byte	6
	.byte	3
	.byte	14
	.byte	0
	.byte	0
	.byte	4
	.byte	29
	.byte	1
	.byte	49
	.byte	19
	.byte	17
	.byte	1
	.byte	18
	.byte	6
	.byte	88
	.byte	11
	.byte	89
	.byte	11
	.byte	87
	.byte	11
	.byte	0
	.byte	0
	.byte	5
	.byte	29
	.byte	0
	.byte	49
	.byte	19
	.byte	17
	.byte	1
	.byte	18
	.byte	6
	.byte	88
	.byte	11
	.byte	89
	.byte	5
	.byte	87
	.byte	11
	.byte	0
	.byte	0
	.byte	6
	.byte	29
	.byte	0
	.byte	49
	.byte	19
	.byte	17
	.byte	1
	.byte	18
	.byte	6
	.byte	88
	.byte	11
	.byte	89
	.byte	11
	.byte	87
	.byte	11
	.byte	0
	.byte	0
	.byte	7
	.byte	29
	.byte	1
	.byte	49
	.byte	19
	.byte	17
	.byte	1
	.byte	18
	.byte	6
	.byte	88
	.byte	11
	.byte	89
	.byte	5
	.byte	87
	.byte	11
	.byte	0
	.byte	0
	.byte	0
	.section	.debug_info,"",@progbits
.Lcu_begin0:
	.long	.Ldebug_info_end0-.Ldebug_info_start0
.Ldebug_info_start0:
	.short	4
	.long	.debug_abbrev
	.byte	8
	.byte	1
	.long	.Linfo_string0
	.short	28
	.long	.Linfo_string1
	.long	.Lline_table_start0
	.long	.Linfo_string2
	.quad	0
	.long	.Ldebug_ranges0
	.byte	2
	.long	.Linfo_string3
	.byte	1
	.byte	2
	.long	.Linfo_string4
	.byte	1
	.byte	2
	.long	.Linfo_string5
	.byte	1
	.byte	2
	.long	.Linfo_string3
	.byte	1
	.byte	2
	.long	.Linfo_string6
	.byte	1
	.byte	2
	.long	.Linfo_string6
	.byte	1
	.byte	2
	.long	.Linfo_string7
	.byte	1
	.byte	3
	.quad	.Lfunc_begin0
	.long	.Lfunc_end0-.Lfunc_begin0
	.long	.Linfo_string41
	.byte	4
	.long	48
	.quad	.Lfunc_begin0
	.long	.Ltmp0-.Lfunc_begin0
	.byte	3
	.byte	243
	.byte	19
	.byte	5
	.long	42
	.quad	.Lfunc_begin0
	.long	.Ltmp0-.Lfunc_begin0
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	54
	.quad	.Ltmp0
	.long	.Ltmp1-.Ltmp0
	.byte	3
	.byte	244
	.byte	44
	.byte	4
	.long	78
	.quad	.Ltmp1
	.long	.Ltmp2-.Ltmp1
	.byte	3
	.byte	244
	.byte	9
	.byte	7
	.long	72
	.quad	.Ltmp1
	.long	.Ltmp2-.Ltmp1
	.byte	4
	.short	1671
	.byte	31
	.byte	7
	.long	66
	.quad	.Ltmp1
	.long	.Ltmp2-.Ltmp1
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	60
	.quad	.Ltmp1
	.long	.Ltmp2-.Ltmp1
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string8
	.byte	1
	.byte	2
	.long	.Linfo_string9
	.byte	1
	.byte	2
	.long	.Linfo_string10
	.byte	1
	.byte	2
	.long	.Linfo_string11
	.byte	1
	.byte	2
	.long	.Linfo_string12
	.byte	1
	.byte	2
	.long	.Linfo_string13
	.byte	1
	.byte	2
	.long	.Linfo_string3
	.byte	1
	.byte	2
	.long	.Linfo_string14
	.byte	1
	.byte	2
	.long	.Linfo_string14
	.byte	1
	.byte	2
	.long	.Linfo_string15
	.byte	1
	.byte	3
	.quad	.Lfunc_begin1
	.long	.Lfunc_end1-.Lfunc_begin1
	.long	.Linfo_string42
	.byte	4
	.long	256
	.quad	.Lfunc_begin1
	.long	.Ltmp4-.Lfunc_begin1
	.byte	3
	.byte	123
	.byte	18
	.byte	5
	.long	250
	.quad	.Lfunc_begin1
	.long	.Ltmp4-.Lfunc_begin1
	.byte	4
	.short	2774
	.byte	5
	.byte	0
	.byte	4
	.long	256
	.quad	.Ltmp5
	.long	.Ltmp6-.Ltmp5
	.byte	3
	.byte	124
	.byte	18
	.byte	5
	.long	250
	.quad	.Ltmp5
	.long	.Ltmp6-.Ltmp5
	.byte	4
	.short	2774
	.byte	5
	.byte	0
	.byte	4
	.long	268
	.quad	.Ltmp6
	.long	.Ltmp7-.Ltmp6
	.byte	3
	.byte	125
	.byte	43
	.byte	5
	.long	262
	.quad	.Ltmp6
	.long	.Ltmp7-.Ltmp6
	.byte	2
	.short	2108
	.byte	14
	.byte	0
	.byte	4
	.long	280
	.quad	.Ltmp7
	.long	.Ltmp8-.Ltmp7
	.byte	3
	.byte	125
	.byte	23
	.byte	5
	.long	274
	.quad	.Ltmp7
	.long	.Ltmp8-.Ltmp7
	.byte	2
	.short	2030
	.byte	14
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp8
	.long	.Ltmp9-.Ltmp8
	.byte	3
	.byte	126
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp8
	.long	.Ltmp9-.Ltmp8
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp8
	.long	.Ltmp9-.Ltmp8
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp8
	.long	.Ltmp9-.Ltmp8
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string16
	.byte	1
	.byte	3
	.quad	.Lfunc_begin2
	.long	.Lfunc_end2-.Lfunc_begin2
	.long	.Linfo_string43
	.byte	4
	.long	48
	.quad	.Lfunc_begin2
	.long	.Ltmp11-.Lfunc_begin2
	.byte	3
	.byte	33
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin2
	.long	.Ltmp11-.Lfunc_begin2
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	582
	.quad	.Ltmp11
	.long	.Ltmp12-.Ltmp11
	.byte	3
	.byte	35
	.byte	54
	.byte	4
	.long	304
	.quad	.Ltmp12
	.long	.Ltmp13-.Ltmp12
	.byte	3
	.byte	35
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp12
	.long	.Ltmp13-.Ltmp12
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp12
	.long	.Ltmp13-.Ltmp12
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp12
	.long	.Ltmp13-.Ltmp12
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string17
	.byte	1
	.byte	3
	.quad	.Lfunc_begin3
	.long	.Lfunc_end3-.Lfunc_begin3
	.long	.Linfo_string44
	.byte	4
	.long	48
	.quad	.Lfunc_begin3
	.long	.Ltmp15-.Lfunc_begin3
	.byte	3
	.byte	49
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin3
	.long	.Ltmp15-.Lfunc_begin3
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	754
	.quad	.Ltmp15
	.long	.Ltmp16-.Ltmp15
	.byte	3
	.byte	51
	.byte	54
	.byte	4
	.long	304
	.quad	.Ltmp16
	.long	.Ltmp17-.Ltmp16
	.byte	3
	.byte	51
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp16
	.long	.Ltmp17-.Ltmp16
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp16
	.long	.Ltmp17-.Ltmp16
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp16
	.long	.Ltmp17-.Ltmp16
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string18
	.byte	1
	.byte	2
	.long	.Linfo_string11
	.byte	1
	.byte	2
	.long	.Linfo_string13
	.byte	1
	.byte	3
	.quad	.Lfunc_begin4
	.long	.Lfunc_end4-.Lfunc_begin4
	.long	.Linfo_string45
	.byte	4
	.long	48
	.quad	.Lfunc_begin4
	.long	.Ltmp19-.Lfunc_begin4
	.byte	3
	.byte	74
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin4
	.long	.Ltmp19-.Lfunc_begin4
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	926
	.quad	.Ltmp19
	.long	.Ltmp20-.Ltmp19
	.byte	3
	.byte	76
	.byte	19
	.byte	4
	.long	932
	.quad	.Ltmp20
	.long	.Ltmp21-.Ltmp20
	.byte	3
	.byte	77
	.byte	20
	.byte	5
	.long	262
	.quad	.Ltmp20
	.long	.Ltmp21-.Ltmp20
	.byte	2
	.short	2108
	.byte	14
	.byte	0
	.byte	4
	.long	938
	.quad	.Ltmp21
	.long	.Ltmp22-.Ltmp21
	.byte	3
	.byte	78
	.byte	40
	.byte	5
	.long	274
	.quad	.Ltmp21
	.long	.Ltmp22-.Ltmp21
	.byte	2
	.short	2030
	.byte	14
	.byte	0
	.byte	4
	.long	932
	.quad	.Ltmp22
	.long	.Ltmp23-.Ltmp22
	.byte	3
	.byte	78
	.byte	20
	.byte	5
	.long	262
	.quad	.Ltmp22
	.long	.Ltmp23-.Ltmp22
	.byte	2
	.short	2108
	.byte	14
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp23
	.long	.Ltmp24-.Ltmp23
	.byte	3
	.byte	79
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp23
	.long	.Ltmp24-.Ltmp23
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp23
	.long	.Ltmp24-.Ltmp23
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp23
	.long	.Ltmp24-.Ltmp23
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp24
	.long	.Ltmp25-.Ltmp24
	.byte	3
	.byte	80
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp24
	.long	.Ltmp25-.Ltmp24
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp24
	.long	.Ltmp25-.Ltmp24
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp24
	.long	.Ltmp25-.Ltmp24
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string19
	.byte	1
	.byte	3
	.quad	.Lfunc_begin5
	.long	.Lfunc_end5-.Lfunc_begin5
	.long	.Linfo_string46
	.byte	4
	.long	48
	.quad	.Lfunc_begin5
	.long	.Ltmp27-.Lfunc_begin5
	.byte	3
	.byte	139
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin5
	.long	.Ltmp27-.Lfunc_begin5
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	1322
	.quad	.Ltmp27
	.long	.Ltmp28-.Ltmp27
	.byte	3
	.byte	141
	.byte	54
	.byte	4
	.long	304
	.quad	.Ltmp28
	.long	.Ltmp29-.Ltmp28
	.byte	3
	.byte	141
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp28
	.long	.Ltmp29-.Ltmp28
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp28
	.long	.Ltmp29-.Ltmp28
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp28
	.long	.Ltmp29-.Ltmp28
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string20
	.byte	1
	.byte	3
	.quad	.Lfunc_begin6
	.long	.Lfunc_end6-.Lfunc_begin6
	.long	.Linfo_string47
	.byte	4
	.long	48
	.quad	.Lfunc_begin6
	.long	.Ltmp31-.Lfunc_begin6
	.byte	3
	.byte	154
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin6
	.long	.Ltmp31-.Lfunc_begin6
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	1494
	.quad	.Ltmp31
	.long	.Ltmp32-.Ltmp31
	.byte	3
	.byte	156
	.byte	54
	.byte	4
	.long	304
	.quad	.Ltmp32
	.long	.Ltmp33-.Ltmp32
	.byte	3
	.byte	156
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp32
	.long	.Ltmp33-.Ltmp32
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp32
	.long	.Ltmp33-.Ltmp32
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp32
	.long	.Ltmp33-.Ltmp32
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string21
	.byte	1
	.byte	2
	.long	.Linfo_string22
	.byte	1
	.byte	2
	.long	.Linfo_string23
	.byte	1
	.byte	2
	.long	.Linfo_string24
	.byte	1
	.byte	2
	.long	.Linfo_string25
	.byte	1
	.byte	3
	.quad	.Lfunc_begin7
	.long	.Lfunc_end7-.Lfunc_begin7
	.long	.Linfo_string48
	.byte	4
	.long	48
	.quad	.Lfunc_begin7
	.long	.Ltmp35-.Lfunc_begin7
	.byte	3
	.byte	175
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin7
	.long	.Ltmp35-.Lfunc_begin7
	.byte	4
	.short	1719
	.byte	5
	.byte	0
	.byte	6
	.long	1666
	.quad	.Ltmp35
	.long	.Ltmp36-.Ltmp35
	.byte	3
	.byte	177
	.byte	19
	.byte	4
	.long	1678
	.quad	.Ltmp36
	.long	.Ltmp37-.Ltmp36
	.byte	3
	.byte	178
	.byte	20
	.byte	5
	.long	1672
	.quad	.Ltmp36
	.long	.Ltmp37-.Ltmp36
	.byte	2
	.short	2082
	.byte	14
	.byte	0
	.byte	4
	.long	1690
	.quad	.Ltmp37
	.long	.Ltmp38-.Ltmp37
	.byte	3
	.byte	179
	.byte	41
	.byte	5
	.long	1684
	.quad	.Ltmp37
	.long	.Ltmp38-.Ltmp37
	.byte	2
	.short	2004
	.byte	14
	.byte	0
	.byte	4
	.long	1678
	.quad	.Ltmp38
	.long	.Ltmp39-.Ltmp38
	.byte	3
	.byte	179
	.byte	20
	.byte	5
	.long	1672
	.quad	.Ltmp38
	.long	.Ltmp39-.Ltmp38
	.byte	2
	.short	2082
	.byte	14
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp39
	.long	.Ltmp40-.Ltmp39
	.byte	3
	.byte	180
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp39
	.long	.Ltmp40-.Ltmp39
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp39
	.long	.Ltmp40-.Ltmp39
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp39
	.long	.Ltmp40-.Ltmp39
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp40
	.long	.Ltmp41-.Ltmp40
	.byte	3
	.byte	181
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp40
	.long	.Ltmp41-.Ltmp40
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp40
	.long	.Ltmp41-.Ltmp40
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp40
	.long	.Ltmp41-.Ltmp40
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string26
	.byte	1
	.byte	2
	.long	.Linfo_string27
	.byte	1
	.byte	3
	.quad	.Lfunc_begin8
	.long	.Lfunc_end8-.Lfunc_begin8
	.long	.Linfo_string49
	.byte	4
	.long	2074
	.quad	.Lfunc_begin8
	.long	.Ltmp43-.Lfunc_begin8
	.byte	3
	.byte	201
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin8
	.long	.Ltmp43-.Lfunc_begin8
	.byte	4
	.short	1652
	.byte	5
	.byte	0
	.byte	6
	.long	2080
	.quad	.Ltmp43
	.long	.Ltmp44-.Ltmp43
	.byte	3
	.byte	204
	.byte	54
	.byte	4
	.long	78
	.quad	.Ltmp44
	.long	.Ltmp45-.Ltmp44
	.byte	3
	.byte	204
	.byte	9
	.byte	7
	.long	72
	.quad	.Ltmp44
	.long	.Ltmp45-.Ltmp44
	.byte	4
	.short	1671
	.byte	31
	.byte	7
	.long	66
	.quad	.Ltmp44
	.long	.Ltmp45-.Ltmp44
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	60
	.quad	.Ltmp44
	.long	.Ltmp45-.Ltmp44
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	6
	.long	2080
	.quad	.Ltmp45
	.long	.Ltmp46-.Ltmp45
	.byte	3
	.byte	205
	.byte	54
	.byte	4
	.long	78
	.quad	.Ltmp46
	.long	.Ltmp47-.Ltmp46
	.byte	3
	.byte	205
	.byte	9
	.byte	7
	.long	72
	.quad	.Ltmp46
	.long	.Ltmp47-.Ltmp46
	.byte	4
	.short	1671
	.byte	31
	.byte	7
	.long	66
	.quad	.Ltmp46
	.long	.Ltmp47-.Ltmp46
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	60
	.quad	.Ltmp46
	.long	.Ltmp47-.Ltmp46
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string28
	.byte	1
	.byte	2
	.long	.Linfo_string29
	.byte	1
	.byte	3
	.quad	.Lfunc_begin9
	.long	.Lfunc_end9-.Lfunc_begin9
	.long	.Linfo_string50
	.byte	6
	.long	2358
	.quad	.Lfunc_begin9
	.long	.Ltmp49-.Lfunc_begin9
	.byte	3
	.byte	104
	.byte	20
	.byte	6
	.long	2364
	.quad	.Ltmp49
	.long	.Ltmp50-.Ltmp49
	.byte	3
	.byte	107
	.byte	22
	.byte	4
	.long	304
	.quad	.Ltmp50
	.long	.Ltmp51-.Ltmp50
	.byte	3
	.byte	108
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp50
	.long	.Ltmp51-.Ltmp50
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp50
	.long	.Ltmp51-.Ltmp50
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp50
	.long	.Ltmp51-.Ltmp50
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	4
	.long	304
	.quad	.Ltmp51
	.long	.Ltmp52-.Ltmp51
	.byte	3
	.byte	109
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp51
	.long	.Ltmp52-.Ltmp51
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp51
	.long	.Ltmp52-.Ltmp51
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp51
	.long	.Ltmp52-.Ltmp51
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string30
	.byte	1
	.byte	2
	.long	.Linfo_string31
	.byte	1
	.byte	2
	.long	.Linfo_string32
	.byte	1
	.byte	2
	.long	.Linfo_string33
	.byte	1
	.byte	3
	.quad	.Lfunc_begin10
	.long	.Lfunc_end10-.Lfunc_begin10
	.long	.Linfo_string51
	.byte	7
	.long	2612
	.quad	.Ltmp54
	.long	.Ltmp57-.Ltmp54
	.byte	3
	.short	284
	.byte	14
	.byte	7
	.long	2606
	.quad	.Ltmp54
	.long	.Ltmp55-.Ltmp54
	.byte	3
	.short	263
	.byte	23
	.byte	5
	.long	2600
	.quad	.Ltmp54
	.long	.Ltmp55-.Ltmp54
	.byte	2
	.short	2418
	.byte	5
	.byte	0
	.byte	5
	.long	2618
	.quad	.Ltmp55
	.long	.Ltmp56-.Ltmp55
	.byte	3
	.short	266
	.byte	13
	.byte	7
	.long	304
	.quad	.Ltmp56
	.long	.Ltmp57-.Ltmp56
	.byte	3
	.short	264
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp56
	.long	.Ltmp57-.Ltmp56
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp56
	.long	.Ltmp57-.Ltmp56
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp56
	.long	.Ltmp57-.Ltmp56
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string34
	.byte	1
	.byte	3
	.quad	.Lfunc_begin11
	.long	.Lfunc_end11-.Lfunc_begin11
	.long	.Linfo_string52
	.byte	4
	.long	2074
	.quad	.Lfunc_begin11
	.long	.Ltmp59-.Lfunc_begin11
	.byte	3
	.byte	222
	.byte	18
	.byte	5
	.long	42
	.quad	.Lfunc_begin11
	.long	.Ltmp59-.Lfunc_begin11
	.byte	4
	.short	1652
	.byte	5
	.byte	0
	.byte	6
	.long	2815
	.quad	.Ltmp59
	.long	.Ltmp60-.Ltmp59
	.byte	3
	.byte	224
	.byte	49
	.byte	4
	.long	78
	.quad	.Ltmp60
	.long	.Ltmp61-.Ltmp60
	.byte	3
	.byte	224
	.byte	9
	.byte	7
	.long	72
	.quad	.Ltmp60
	.long	.Ltmp61-.Ltmp60
	.byte	4
	.short	1671
	.byte	31
	.byte	7
	.long	66
	.quad	.Ltmp60
	.long	.Ltmp61-.Ltmp60
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	60
	.quad	.Ltmp60
	.long	.Ltmp61-.Ltmp60
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string31
	.byte	1
	.byte	2
	.long	.Linfo_string35
	.byte	1
	.byte	3
	.quad	.Lfunc_begin12
	.long	.Lfunc_end12-.Lfunc_begin12
	.long	.Linfo_string53
	.byte	7
	.long	2993
	.quad	.Lfunc_begin12
	.long	.Ltmp64-.Lfunc_begin12
	.byte	3
	.short	295
	.byte	14
	.byte	7
	.long	2987
	.quad	.Lfunc_begin12
	.long	.Ltmp63-.Lfunc_begin12
	.byte	3
	.short	263
	.byte	23
	.byte	5
	.long	2600
	.quad	.Lfunc_begin12
	.long	.Ltmp63-.Lfunc_begin12
	.byte	2
	.short	2418
	.byte	5
	.byte	0
	.byte	7
	.long	304
	.quad	.Ltmp63
	.long	.Ltmp64-.Ltmp63
	.byte	3
	.short	264
	.byte	9
	.byte	7
	.long	298
	.quad	.Ltmp63
	.long	.Ltmp64-.Ltmp63
	.byte	4
	.short	1737
	.byte	14
	.byte	7
	.long	292
	.quad	.Ltmp63
	.long	.Ltmp64-.Ltmp63
	.byte	5
	.short	1478
	.byte	18
	.byte	5
	.long	286
	.quad	.Ltmp63
	.long	.Ltmp64-.Ltmp63
	.byte	1
	.short	2003
	.byte	9
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	0
	.byte	2
	.long	.Linfo_string36
	.byte	1
	.byte	3
	.quad	.Lfunc_begin13
	.long	.Lfunc_end13-.Lfunc_begin13
	.long	.Linfo_string54
	.byte	5
	.long	3169
	.quad	.Ltmp67
	.long	.Ltmp68-.Ltmp67
	.byte	3
	.short	320
	.byte	11
	.byte	0
	.byte	2
	.long	.Linfo_string37
	.byte	1
	.byte	2
	.long	.Linfo_string38
	.byte	1
	.byte	3
	.quad	.Lfunc_begin14
	.long	.Lfunc_end14-.Lfunc_begin14
	.long	.Linfo_string55
	.byte	5
	.long	3214
	.quad	.Lfunc_begin14
	.long	.Ltmp71-.Lfunc_begin14
	.byte	3
	.short	326
	.byte	15
	.byte	5
	.long	3220
	.quad	.Ltmp71
	.long	.Ltmp72-.Ltmp71
	.byte	3
	.short	326
	.byte	45
	.byte	5
	.long	3214
	.quad	.Ltmp72
	.long	.Ltmp73-.Ltmp72
	.byte	3
	.short	326
	.byte	36
	.byte	0
	.byte	2
	.long	.Linfo_string39
	.byte	1
	.byte	2
	.long	.Linfo_string40
	.byte	1
	.byte	3
	.quad	.Lfunc_begin15
	.long	.Lfunc_end15-.Lfunc_begin15
	.long	.Linfo_string56
	.byte	7
	.long	3313
	.quad	.Ltmp75
	.long	.Ltmp77-.Ltmp75
	.byte	10
	.short	2865
	.byte	62
	.byte	6
	.long	3307
	.quad	.Ltmp75
	.long	.Ltmp76-.Ltmp75
	.byte	11
	.byte	86
	.byte	26
	.byte	0
	.byte	0
	.byte	0
.Ldebug_info_end0:
	.section	.text._ZN21ldpc_intrinsics_probe16rotate_lanes_f3217hf2ceead5e3b8cc07E,"ax",@progbits
.Lsec_end0:
	.section	.text._ZN21ldpc_intrinsics_probe17clip_symmetric_i817hdde6a9234763c22aE,"ax",@progbits
.Lsec_end1:
	.section	.text._ZN21ldpc_intrinsics_probe17saturating_add_i817h0d0695b9ca1df843E,"ax",@progbits
.Lsec_end2:
	.section	.text._ZN21ldpc_intrinsics_probe17saturating_sub_i817hcf4a8049e48b1d42E,"ax",@progbits
.Lsec_end3:
	.section	.text._ZN21ldpc_intrinsics_probe18fold_two_minima_i817h66fa37ba554ab459E,"ax",@progbits
.Lsec_end4:
	.section	.text._ZN21ldpc_intrinsics_probe18saturating_add_i1617h0f8961910f9d44a9E,"ax",@progbits
.Lsec_end5:
	.section	.text._ZN21ldpc_intrinsics_probe18saturating_sub_i1617hf72bd60c12ea4fedE,"ax",@progbits
.Lsec_end6:
	.section	.text._ZN21ldpc_intrinsics_probe19fold_two_minima_i1617hf1a0de09b7006087E,"ax",@progbits
.Lsec_end7:
	.section	.text._ZN21ldpc_intrinsics_probe22magnitude_and_sign_f3217h2a6df523831d0609E,"ax",@progbits
.Lsec_end8:
	.section	.text._ZN21ldpc_intrinsics_probe22sign_mask_and_apply_i817h0623d82ad0e977feE,"ax",@progbits
.Lsec_end9:
	.section	.text._ZN21ldpc_intrinsics_probe24rotate_bytes_i8_by_three17he200418661337542E,"ax",@progbits
.Lsec_end10:
	.section	.text._ZN21ldpc_intrinsics_probe26negative_by_comparison_f3217h01caf0eaba63442aE,"ax",@progbits
.Lsec_end11:
	.section	.text._ZN21ldpc_intrinsics_probe26rotate_bytes_i8_by_sixteen17h8cc09a31a5549baaE,"ax",@progbits
.Lsec_end12:
	.section	.text._ZN21ldpc_intrinsics_probe9reference12magnitude_i817h86562cf2b0df91beE,"ax",@progbits
.Lsec_end13:
	.section	.text._ZN21ldpc_intrinsics_probe9reference15fold_two_minima17h540b54da7bceed1cE,"ax",@progbits
.Lsec_end14:
	.section	".text._ZN42_$LT$$RF$T$u20$as$u20$core..fmt..Debug$GT$3fmt17hd432242faafbf514E","ax",@progbits
.Lsec_end15:
	.section	.text.unlikely._ZN4core9panicking13assert_failed17ha2519999bdb0d7e3E,"ax",@progbits
.Lsec_end16:
	.section	.debug_aranges,"",@progbits
	.long	300
	.short	2
	.long	.Lcu_begin0
	.byte	8
	.byte	0
	.zero	4,255
	.quad	.Lfunc_begin0
	.quad	.Lsec_end0-.Lfunc_begin0
	.quad	.Lfunc_begin1
	.quad	.Lsec_end1-.Lfunc_begin1
	.quad	.Lfunc_begin2
	.quad	.Lsec_end2-.Lfunc_begin2
	.quad	.Lfunc_begin3
	.quad	.Lsec_end3-.Lfunc_begin3
	.quad	.Lfunc_begin4
	.quad	.Lsec_end4-.Lfunc_begin4
	.quad	.Lfunc_begin5
	.quad	.Lsec_end5-.Lfunc_begin5
	.quad	.Lfunc_begin6
	.quad	.Lsec_end6-.Lfunc_begin6
	.quad	.Lfunc_begin7
	.quad	.Lsec_end7-.Lfunc_begin7
	.quad	.Lfunc_begin8
	.quad	.Lsec_end8-.Lfunc_begin8
	.quad	.Lfunc_begin9
	.quad	.Lsec_end9-.Lfunc_begin9
	.quad	.Lfunc_begin10
	.quad	.Lsec_end10-.Lfunc_begin10
	.quad	.Lfunc_begin11
	.quad	.Lsec_end11-.Lfunc_begin11
	.quad	.Lfunc_begin12
	.quad	.Lsec_end12-.Lfunc_begin12
	.quad	.Lfunc_begin13
	.quad	.Lsec_end13-.Lfunc_begin13
	.quad	.Lfunc_begin14
	.quad	.Lsec_end14-.Lfunc_begin14
	.quad	.Lfunc_begin15
	.quad	.Lsec_end15-.Lfunc_begin15
	.quad	.Lfunc_begin16
	.quad	.Lsec_end16-.Lfunc_begin16
	.quad	0
	.quad	0
	.section	.debug_ranges,"",@progbits
.Ldebug_ranges0:
	.quad	.Lfunc_begin0
	.quad	.Lfunc_end0
	.quad	.Lfunc_begin1
	.quad	.Lfunc_end1
	.quad	.Lfunc_begin2
	.quad	.Lfunc_end2
	.quad	.Lfunc_begin3
	.quad	.Lfunc_end3
	.quad	.Lfunc_begin4
	.quad	.Lfunc_end4
	.quad	.Lfunc_begin5
	.quad	.Lfunc_end5
	.quad	.Lfunc_begin6
	.quad	.Lfunc_end6
	.quad	.Lfunc_begin7
	.quad	.Lfunc_end7
	.quad	.Lfunc_begin8
	.quad	.Lfunc_end8
	.quad	.Lfunc_begin9
	.quad	.Lfunc_end9
	.quad	.Lfunc_begin10
	.quad	.Lfunc_end10
	.quad	.Lfunc_begin11
	.quad	.Lfunc_end11
	.quad	.Lfunc_begin12
	.quad	.Lfunc_end12
	.quad	.Lfunc_begin13
	.quad	.Lfunc_end13
	.quad	.Lfunc_begin14
	.quad	.Lfunc_end14
	.quad	.Lfunc_begin15
	.quad	.Lfunc_end15
	.quad	.Lfunc_begin16
	.quad	.Lfunc_end16
	.quad	0
	.quad	0
	.section	.debug_str,"MS",@progbits,1
.Linfo_string0:
	.asciz	"clang LLVM (rustc version 1.95.0 (59807616e 2026-04-14))"
.Linfo_string1:
	.asciz	"src/lib.rs/@/ldpc_intrinsics_probe.ba30df1800a1b3f1-cgu.0"
.Linfo_string2:
	.asciz	"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-9fb40c83/dev/active/f63a2464/survey/intrinsics-probe"
.Linfo_string3:
	.asciz	"copy_nonoverlapping<u8>"
.Linfo_string4:
	.asciz	"_mm256_loadu_si256"
.Linfo_string5:
	.asciz	"_mm256_permutevar8x32_ps"
.Linfo_string6:
	.asciz	"write_unaligned<core::core_arch::x86::__m256>"
.Linfo_string7:
	.asciz	"_mm256_storeu_ps"
.Linfo_string8:
	.asciz	"splat<i8, 32>"
.Linfo_string9:
	.asciz	"_mm256_set1_epi8"
.Linfo_string10:
	.asciz	"simd_imin<core::core_arch::simd::Simd<i8, 32>>"
.Linfo_string11:
	.asciz	"_mm256_min_epi8"
.Linfo_string12:
	.asciz	"simd_imax<core::core_arch::simd::Simd<i8, 32>>"
.Linfo_string13:
	.asciz	"_mm256_max_epi8"
.Linfo_string14:
	.asciz	"write_unaligned<core::core_arch::x86::__m256i>"
.Linfo_string15:
	.asciz	"_mm256_storeu_si256"
.Linfo_string16:
	.asciz	"_mm256_adds_epi8"
.Linfo_string17:
	.asciz	"_mm256_subs_epi8"
.Linfo_string18:
	.asciz	"_mm256_abs_epi8"
.Linfo_string19:
	.asciz	"_mm256_adds_epi16"
.Linfo_string20:
	.asciz	"_mm256_subs_epi16"
.Linfo_string21:
	.asciz	"_mm256_abs_epi16"
.Linfo_string22:
	.asciz	"simd_imin<core::core_arch::simd::Simd<i16, 16>>"
.Linfo_string23:
	.asciz	"_mm256_min_epi16"
.Linfo_string24:
	.asciz	"simd_imax<core::core_arch::simd::Simd<i16, 16>>"
.Linfo_string25:
	.asciz	"_mm256_max_epi16"
.Linfo_string26:
	.asciz	"_mm256_loadu_ps"
.Linfo_string27:
	.asciz	"_mm256_and_ps"
.Linfo_string28:
	.asciz	"_mm256_cmpgt_epi8"
.Linfo_string29:
	.asciz	"_mm256_sub_epi8"
.Linfo_string30:
	.asciz	"_mm256_permute2f128_si256<1>"
.Linfo_string31:
	.asciz	"_mm256_permute2x128_si256<1>"
.Linfo_string32:
	.asciz	"rotate_bytes_i8<3>"
.Linfo_string33:
	.asciz	"_mm256_alignr_epi8<3>"
.Linfo_string34:
	.asciz	"_mm256_cmp_ps<1>"
.Linfo_string35:
	.asciz	"rotate_bytes_i8<16>"
.Linfo_string36:
	.asciz	"abs"
.Linfo_string37:
	.asciz	"min<i32>"
.Linfo_string38:
	.asciz	"max<i32>"
.Linfo_string39:
	.asciz	"debug_lower_hex"
.Linfo_string40:
	.asciz	"fmt"
.Linfo_string41:
	.asciz	"rotate_lanes_f32"
.Linfo_string42:
	.asciz	"clip_symmetric_i8"
.Linfo_string43:
	.asciz	"saturating_add_i8"
.Linfo_string44:
	.asciz	"saturating_sub_i8"
.Linfo_string45:
	.asciz	"fold_two_minima_i8"
.Linfo_string46:
	.asciz	"saturating_add_i16"
.Linfo_string47:
	.asciz	"saturating_sub_i16"
.Linfo_string48:
	.asciz	"fold_two_minima_i16"
.Linfo_string49:
	.asciz	"magnitude_and_sign_f32"
.Linfo_string50:
	.asciz	"sign_mask_and_apply_i8"
.Linfo_string51:
	.asciz	"rotate_bytes_i8_by_three"
.Linfo_string52:
	.asciz	"negative_by_comparison_f32"
.Linfo_string53:
	.asciz	"rotate_bytes_i8_by_sixteen"
.Linfo_string54:
	.asciz	"magnitude_i8"
.Linfo_string55:
	.asciz	"fold_two_minima"
.Linfo_string56:
	.asciz	"fmt<i8>"
	.ident	"rustc version 1.95.0 (59807616e 2026-04-14)"
	.section	".note.GNU-stack","",@progbits
	.section	.debug_line,"",@progbits
.Lline_table_start0:
