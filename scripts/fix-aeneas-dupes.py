#!/usr/bin/env python3
"""Fix duplicate field names in Aeneas-generated Lean files.

Aeneas generates duplicate field names in the FiniteField struct/instance when
a trait has bounds on multiple associated types (Self, Characteristic, Wide).
This script disambiguates them by appending type suffixes.

See proofs/WORKAROUNDS.md for details.
"""

import re
import sys


def dedup_fields(filepath):
    with open(filepath) as f:
        lines = f.read().split('\n')

    result = []
    in_block = False
    seen = set()

    for line in lines:
        stripped = line.strip()

        # Detect start of FiniteField struct or instance
        if 'field.traits.FiniteField' in stripped and (
            stripped.startswith('structure') or ':=' in stripped
        ):
            in_block = True
            seen = set()
            result.append(line)
            continue

        if in_block:
            # Match field declaration "  fieldName : Type" or assignment "  fieldName := val"
            m = re.match(r'^  (\w+)\s*(:=|:)\s*(.*)', line)
            if m:
                name, op, rest = m.group(1), m.group(2), m.group(3)
                if name in seen:
                    # Determine suffix from the type/value context
                    if any(k in rest for k in ('Characteristic', 'U64', 'Self_Characteristic')):
                        new = re.sub(r'Inst$', 'CharacteristicInst', name)
                        if new == name:
                            new = name + 'Characteristic'
                    else:
                        new = re.sub(r'Inst$', 'WideInst', name)
                        if new == name:
                            new = name + 'Wide'
                    line = f'  {new} {op} {rest}'
                seen.add(name)

            # End block on empty line or next definition
            if stripped == '' or (stripped.startswith('/') and 'Trait' in stripped):
                in_block = False

        result.append(line)

    with open(filepath, 'w') as f:
        f.write('\n'.join(result))


if __name__ == '__main__':
    for path in sys.argv[1:]:
        dedup_fields(path)
