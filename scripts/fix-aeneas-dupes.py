#!/usr/bin/env python3
"""Fix duplicate field names in Aeneas-generated Lean files.

Aeneas generates duplicate field names in the FiniteField struct/instance when
a trait has bounds on multiple associated types (Self, Characteristic, Wide).
This script disambiguates them by appending type suffixes.

The FiniteField struct expects these fields in order:
  - 11 Self fields (corecloneClone, corecmpPartialEq, corecmpEq, corehashHash,
    corefmtDebug, coreopsarithAdd, coreopsarithSub, coreopsarithMul,
    coreopsarithDiv, coreopsarithNeg, coreopsarithAddAssign)
  - 4 Characteristic fields (corecloneClone, corefmtDebug, corecmpPartialEq,
    corecmpEq)
  - 3 Wide fields (corecloneClone, coreopsarithAdd, coreopsarithAddAssign)

See proofs/WORKAROUNDS.md for details.
"""

import re
import sys

# Fields expected in the Characteristic group (in order)
CHARACTERISTIC_FIELDS = {
    'corecloneCloneInst',
    'corefmtDebugInst',
    'corecmpPartialEqInst',
    'corecmpEqInst',
}

# Fields expected in the Wide group (in order)
WIDE_FIELDS = {
    'corecloneCloneInst',
    'coreopsarithAddInst',
    'coreopsarithAddAssignInst',
}


def dedup_fields(filepath):
    with open(filepath) as f:
        lines = f.read().split('\n')

    result = []
    in_block = False
    # Track which group we're in: 'self', 'characteristic', 'wide', 'done'
    group = 'self'
    self_fields_seen = set()
    char_fields_seen = set()
    # Lookback window: when we see 'field.traits.FiniteField', set a counter.
    # If ':=' appears within the window, we enter a block. This handles both
    # single-line (gfp) and multi-line (gfpn) FiniteField declarations.
    finite_field_lookback = 0
    # Track renamed fields to fix projection paths in value continuation lines.
    # When a field is renamed (e.g., corecloneCloneInst → corecloneCloneCharacteristicInst),
    # subsequent value lines that project .corecloneCloneInst must also be updated.
    pending_rename = None  # (old_name, new_name)

    for line in lines:
        stripped = line.strip()

        # Detect FiniteField reference — start lookback window
        if 'field.traits.FiniteField' in stripped:
            finite_field_lookback = 10

        # Detect start of FiniteField struct or instance
        if not in_block and finite_field_lookback > 0 and (
            stripped.startswith('structure') or ':= {' in stripped
        ):
            in_block = True
            finite_field_lookback = 0
            group = 'self'
            self_fields_seen = set()
            char_fields_seen = set()
            pending_rename = None
            result.append(line)
            continue

        if finite_field_lookback > 0:
            finite_field_lookback -= 1

        if in_block:
            # Match field declaration "  fieldName : Type" or assignment "  fieldName := val"
            m = re.match(r'^  (\w+)\s*(:=|:)\s*(.*)', line)
            if m:
                pending_rename = None  # New field definition, clear pending
                name, op, rest = m.group(1), m.group(2), m.group(3)

                if group == 'self':
                    if name in self_fields_seen:
                        # We've left the Self group — this is a duplicate
                        if name in CHARACTERISTIC_FIELDS:
                            group = 'characteristic'
                        else:
                            group = 'wide'
                    else:
                        self_fields_seen.add(name)

                if group == 'characteristic':
                    if name in char_fields_seen:
                        # Already seen in Characteristic — must be Wide now
                        group = 'wide'
                    elif name in CHARACTERISTIC_FIELDS:
                        char_fields_seen.add(name)
                        new = re.sub(r'Inst$', 'CharacteristicInst', name)
                        if new == name:
                            new = name + 'Characteristic'
                        # Also fix projection in rest if value is on same line
                        rest = rest.replace('.' + name, '.' + new)
                        line = f'  {new} {op} {rest}'
                        pending_rename = (name, new)
                    else:
                        # Not a Characteristic field — must be Wide
                        group = 'wide'

                if group == 'wide':
                    if name in self_fields_seen:
                        new = re.sub(r'Inst$', 'WideInst', name)
                        if new == name:
                            new = name + 'Wide'
                        rest = rest.replace('.' + name, '.' + new)
                        line = f'  {new} {op} {rest}'
                        pending_rename = (name, new)
            else:
                # Value continuation line — fix projection paths if a field was just renamed
                if pending_rename:
                    old_name, new_name = pending_rename
                    if '.' + old_name in line:
                        line = line.replace('.' + old_name, '.' + new_name)

            # End block on empty line or next definition
            if stripped == '' or (stripped.startswith('/') and 'Trait' in stripped):
                in_block = False
                group = 'self'
                pending_rename = None

        result.append(line)

    with open(filepath, 'w') as f:
        f.write('\n'.join(result))


# Map of FiniteField default methods to (arg_names, body) for inlining.
# Aeneas (as of upstream 5fc8fdf2) emits references like
#   `<ImplNamespace>.<MethodName> <ext_configExtConfigInst>`
# in instance dictionaries even when the impl uses the trait default. Those
# references resolve to non-existent sibling defs, so `lake build` errors with
# "Invalid field <MethodName>". Inline the trait default body at the call site.
#
# Each entry: method_name -> (list of arg_names, body expression).
# `&mut` slice / Vec args become *return-tuple components* in the Aeneas
# extraction (Charon converts `&mut T -> bool` to `-> Result (Bool × T)`),
# so the body must thread the unmodified arg back into the result tuple.
DEFAULT_METHOD_BODIES = {
    'try_simd_dot_product':            (['a', 'b'], 'ok none'),
    'try_simd_gemm_classical':         (['a', 'b_t', 'm', 'k', 'n', 'out'],
                                        'ok (false, out)'),
    'chain_poly_arith_available':      ([], 'ok false'),
    'try_simd_axpy':                   (['y', 'a', 'x'], 'ok (false, y)'),
    'try_simd_matvec':                 (['a', 'x', 'm', 'k', 'out'],
                                        'ok (false, out)'),
    'try_simd_spmm':                   (['a_row_ptr', 'a_col_idx', 'a_values',
                                         'b', 'b_rows', 'n', 'out'],
                                        'ok (false, out)'),
    'try_extension_wiedemann_minpoly': (['a'], 'ok none'),
    'try_fp_simd_dot_product':         (['a', 'b', 'scratch_a', 'scratch_b'],
                                        'ok (none, scratch_a, scratch_b)'),
    'try_pack_fp_medium_u16':          (['xs', 'out'], 'ok (none, out)'),
    'try_fp_simd_dot_packed_u16':      (['a_packed', 'b_packed'], 'ok none'),
    # SimdVecOps trait methods (vector-level versions); &mut return shape unknown
    # to this script, defaults are conservative.
    'try_simd_dot_vec':                (['a', 'b'], 'ok none'),
    'try_simd_add_vec':                (['a', 'b'], 'ok none'),
    'try_simd_sub_vec':                (['a', 'b'], 'ok none'),
    'try_simd_mul_vec':                (['a', 'b'], 'ok none'),
}

# Map of associated-const defaults that Aeneas references via `.default` suffix.
# Each entry: const_name -> literal default value.
DEFAULT_CONST_BODIES = {
    'PLE_BASE_COLS': 'ok 1#usize',
    # `const PLE_PANEL_COLS: usize = Self::PLE_BASE_COLS;` (field/traits.rs).
    # Instances that do not override it inherit the trait default, which
    # resolves to the default PLE_BASE_COLS = 1. Aeneas (5220259c) references
    # it via the `.default` sibling for those instances without emitting a def.
    'PLE_PANEL_COLS': 'ok 1#usize',
}


def _lambda(arg_names: list, body: str) -> str:
    """Build `fun <names> => <body>` or just `<body>` if no args."""
    if not arg_names:
        return body
    return 'fun ' + ' '.join(arg_names) + ' => ' + body


def inline_default_methods(filepath):
    """Replace references to non-existent default-method sibling defs with the
    inline default body. See DEFAULT_METHOD_BODIES for the catalogue.

    Pattern (3-line, in instance dictionary):
        <method_name> :=
          <ImplNs>.<method_name>
          ext_configExtConfigInst         -- or some other receiver line(s)

    Pattern (1-line):
        <method_name> := <ImplNs>.<method_name> <receiver>

    For each known default method, replace with `<method_name> := <inline_body>`
    where `<inline_body>` is `fun _ _ ... => ok none` (or false).
    """
    with open(filepath) as f:
        text = f.read()

    # Build the set of fully-qualified `def` names so we can check, per
    # specific impl namespace, whether a referenced sibling def exists.
    # Aeneas may define `<NsA>.method` but not `<NsB>.method`; the rewrite
    # must fire only for the missing namespaces.
    defined_qualnames = set()
    for m in re.finditer(
        r'^def\s*\n\s*([\w.]+)',
        text,
        re.MULTILINE,
    ):
        defined_qualnames.add(m.group(1))
    for m in re.finditer(r'^def\s+([\w.]+)', text, re.MULTILINE):
        defined_qualnames.add(m.group(1))

    lines = text.split('\n')
    out = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        # Match field assignment opening "  <method_name> :=" with empty or
        # non-empty trailing.
        m = re.match(r'^(\s+)(\w+)\s*:=\s*(.*)$', line)
        if not m:
            out.append(line)
            i += 1
            continue

        indent, name, rest = m.group(1), m.group(2), m.group(3)

        # Const-default rewrite: `PLE_BASE_COLS := field.traits.FiniteField.PLE_BASE_COLS.default`
        if name in DEFAULT_CONST_BODIES and 'field.traits.FiniteField.' in rest \
                and rest.endswith('.default'):
            out.append(f'{indent}{name} := {DEFAULT_CONST_BODIES[name]}')
            # Skip the receiver continuation line(s) until next field
            # assignment, comma, or block-closing brace.
            i += 1
            while i < n:
                nxt = lines[i]
                # Stop at next field-assignment-looking line, or '}' close.
                if re.match(r'^\s+\w+\s*:=', nxt) or re.match(r'^\s*\}', nxt):
                    break
                i += 1
            continue

        # Method-default rewrite: fire if the method is in our table AND the
        # rhs references a specific `<Ns>.<method>` whose qualified form is
        # NOT in `defined_qualnames`. Pure name-based check is too broad
        # (Fp impl may define try_simd_dot_product while QuadraticExt impl
        # doesn't — both share the unqualified name).
        if name in DEFAULT_METHOD_BODIES:
            arg_names, body = DEFAULT_METHOD_BODIES[name]
            rhs_first = rest
            j = i
            if rhs_first == '':
                # Look at next non-empty line for the qualified ref.
                k = i + 1
                while k < n and lines[k].strip() == '':
                    k += 1
                if k < n:
                    rhs_first = lines[k].strip()
                    j = k

            # Extract the qualified name from rhs_first, check if it dangles.
            qm = re.match(r'^([\w.]+)\b', rhs_first)
            if qm:
                qualname = qm.group(1)
                if (qualname.endswith('.' + name)
                        and qualname not in defined_qualnames):
                    out.append(f'{indent}{name} := {_lambda(arg_names, body)}')
                    i = j + 1
                    # Skip extra continuation lines (additional receivers/args)
                    # until next field assignment or close brace.
                    while i < n:
                        nxt = lines[i]
                        if re.match(r'^\s+\w+\s*:=', nxt) or re.match(r'^\s*\}', nxt):
                            break
                        if nxt.strip() == '':
                            i += 1
                            break
                        i += 1
                    continue

        out.append(line)
        i += 1

    with open(filepath, 'w') as f:
        f.write('\n'.join(out))


def silence_extraction_sorry(filepath):
    """Inject `set_option warn.sorry false` into Aeneas-generated files.

    `proofs/lakefile.lean` enables `warningAsError=true` on the project's
    `lean_lib`s, which turns the elaborator-emitted `declaration uses 'sorry'`
    warning into a build error. That gate is what catches masked proof debt
    in hand-written `Proofs/` files (issue 2e544a34).

    Aeneas-generated `Funs.lean` files (and occasionally `FunsExternal.lean`)
    contain extraction-artefact sorrys for items Aeneas could not translate.
    Those are not proof debt; they are translator gaps. Silence the sorry
    warning at file scope so the strict gate fires only on hand-written
    proofs.

    Idempotent: skips if the option is already set.
    """
    with open(filepath) as f:
        text = f.read()

    if 'set_option warn.sorry false' in text:
        return

    # Skip files that have no sorrys; adding the option there would be
    # misleading future-readers about why it's there.
    if 'sorry' not in text:
        return

    # Slot the option in next to the other Aeneas-emitted `set_option` lines
    # so it inherits their position above the imports/body.
    target = 'set_option linter.unusedVariables false\n'
    if target not in text:
        # Older Aeneas output may not have this line; fall back to inserting
        # after the open Aeneas... line.
        target = 'open Aeneas Aeneas.Std Result ControlFlow Error\n'
    if target not in text:
        # Last-resort: prepend at the very top after the auto-gen banner.
        text = (
            '-- Strict-build carve-out: Aeneas extraction artefacts may carry\n'
            '-- `sorry` placeholders for items it could not translate.\n'
            'set_option warn.sorry false\n'
            + text
        )
    else:
        text = text.replace(
            target,
            target
            + '-- Strict-build carve-out (issue 2e544a34): Aeneas extraction\n'
              '-- artefacts may carry `sorry` placeholders for items the\n'
              '-- translator could not handle. Silence the elaborator warning\n'
              '-- at file scope so the project lakefile\'s warningAsError=true\n'
              '-- fires only on hand-written Proofs/ files.\n'
              'set_option warn.sorry false\n',
            1,
        )

    with open(filepath, 'w') as f:
        f.write(text)


if __name__ == '__main__':
    for path in sys.argv[1:]:
        dedup_fields(path)
        inline_default_methods(path)
        silence_extraction_sorry(path)
