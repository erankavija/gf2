#!/usr/bin/env python3
"""Compose the d06a99ed before/after plan with the existing family generator."""

import argparse
import copy
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def load_generator(path):
    spec = importlib.util.spec_from_file_location("ldpc_family", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def compose(generator, declaration, before, after, out, confirmation=False):
    records, aliases = [], {}
    catalogue = None
    for side, original in (("before", before), ("after", after)):
        record = copy.deepcopy(original)
        source = json.loads((generator.smoke.ROOT / record["catalogue"]["path"]).read_text())
        if catalogue is None:
            catalogue = {"codes": source["codes"], "arms": {}}
        name = f"gf2-batch-tuned-{side}"
        row = record["arms"]["gf2-batch-tuned"]
        row["path"] = str(Path(record["build_root"]) / row["path"])
        record["arms"] = {name: row}
        records.append(record)
        aliases[name] = source["arms"]["gf2-batch-tuned"]
    catalogue["arms"] = aliases
    profiles = generator.tuning_profiles(declaration, aliases)
    out.mkdir(parents=True, exist_ok=True)
    manifests = generator.bundles(catalogue, out)
    cells = [generator.merged(declaration["cell"], cell) for cell in declaration["cells"]]
    addendum = out / generator.ADDENDUM
    if confirmation:
        frozen = json.loads(addendum.read_text())
        declared = {cell["cell_id"]: cell for cell in cells}
        cells = [declared[cell["cell_id"]] for cell in frozen["cells"]]
    else:
        schema = generator.schema()
        generator.write(addendum, generator.addendum(
            declaration, cells, manifests, aliases, schema,
            generator.shared_settings(schema), profiles))
    generator.write(out / generator.PLAN, generator.plan(
        declaration, cells, manifests, catalogue, records, profiles,
        generator.repository_path(addendum), "confirmation" if confirmation else "pilot"))
    return out / generator.PLAN


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("generator", type=Path)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--confirmation", action="store_true")
    args = parser.parse_args()
    declaration = json.loads(Path(__file__).with_name("declaration.json").read_text())
    print(compose(load_generator(args.generator.resolve()), declaration,
                  json.loads(args.before.read_text()), json.loads(args.after.read_text()),
                  args.output.resolve(), args.confirmation))


if __name__ == "__main__":
    main()
