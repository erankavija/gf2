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


if __name__ == '__main__':
    for path in sys.argv[1:]:
        dedup_fields(path)
