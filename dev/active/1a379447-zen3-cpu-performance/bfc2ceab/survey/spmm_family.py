"""Committed stages of the sparse product check-cost family (jit:bfc2ceab)."""

import json

import review
from locate import HERE, family_ledger, family_stages, sha256  # noqa: F401

FAMILY = HERE.parent
ADDENDUM = FAMILY / review.GRAIN_FAMILY_ADDENDUM


def pilot_addendum():
    return json.loads(ADDENDUM.read_text())


def ledger():
    return family_ledger(ADDENDUM)


def stages():
    return family_stages(ADDENDUM)
