"""Separates Rust code text from comments and literals.

`code_text` gives the lines a comment or blank-line edit leaves equal; `masked`
keeps every byte position and blanks comments and literal bodies, so a search
over the result finds code tokens only.
"""

from __future__ import annotations

import re

RULE = (
    "Rust sources only. Line comments (`//` to the end of the line, doc comments "
    "included) and block comments (`/* */`, nested) outside string, raw-string, "
    "byte-string and character literals are removed; trailing whitespace is "
    "removed from every line; lines left empty are removed. Two files whose "
    "remaining text is equal are comment-or-blank-only. Leading whitespace, "
    "line breaks inside code and `#[doc]` attributes count as code."
)

LEXEME = re.compile(
    r"""
      (?P<line>//[^\n]*)
    | (?P<block>/\*)
    | b?r(?P<hashes>\#*)".*?"(?P=hashes)
    | b?"(?:\\.|[^"\\])*"
    | b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'
    """,
    re.S | re.X,
)
BLOCK_EDGE = re.compile(r"/\*|\*/")


def lexemes(source: str):
    """Yields `(start, end, kind)` for each comment and literal, in order.

    `kind` is `line`, `block` or `literal`.
    """
    position = 0
    while match := LEXEME.search(source, position):
        position = match.end()
        if match["block"]:
            depth = 1
            while depth:
                edge = BLOCK_EDGE.search(source, position)
                if edge is None:
                    raise SystemExit("unterminated block comment")
                depth += 1 if edge[0] == "/*" else -1
                position = edge.end()
            yield match.start(), position, "block"
        else:
            yield match.start(), position, "line" if match["line"] else "literal"


def code_text(source: str) -> list[str]:
    """`source` under RULE."""
    kept, position = [], 0
    for start, end, kind in lexemes(source):
        kept.append(source[position:start])
        if kind == "literal":
            kept.append(source[start:end])
        position = end
    kept.append(source[position:])
    lines = (line.rstrip() for line in "".join(kept).splitlines())
    return [line for line in lines if line]


def masked(source: str) -> str:
    """`source` with every comment and literal character except newlines replaced by a space."""
    kept, position = [], 0
    for start, end, _ in lexemes(source):
        kept.append(source[position:start])
        kept.append(re.sub(r"[^\n]", " ", source[start:end]))
        position = end
    kept.append(source[position:])
    return "".join(kept)
