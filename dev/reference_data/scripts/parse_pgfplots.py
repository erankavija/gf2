#!/usr/bin/env python3
"""
Parse pgfplots .tex files from the SO-GRAND paper to extract simulation reference data.

Each .tex file may contain one or more tikzpicture/axis environments. This script
extracts all \addplot ... table[]{x y ...} blocks together with their \addlegendentry
labels and the axis ylabel (to determine if the metric is BLER, BER, etc.).

Output: one CSV per figure with columns:
    eb_n0_db, value, metric, decoder, code_params, style_info
"""

import re
import csv
import os
import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional


# --------------------------------------------------------------------------- #
# Data structures                                                              #
# --------------------------------------------------------------------------- #

@dataclass
class Curve:
    legend: str
    x_values: list
    y_values: list
    style: str          # the pgfplots style string (color, mark, dashed, …)
    metric: str         # BLER / BER / guesses_per_bit / iterations / other
    axis_xlabel: str
    axis_ylabel: str
    figure_id: str      # source filename stem


# --------------------------------------------------------------------------- #
# Helpers                                                                      #
# --------------------------------------------------------------------------- #

def strip_latex(text: str) -> str:
    """Remove common LaTeX markup so labels are readable plain text."""
    # Remove $...$ math delimiters (non-greedy, handle nested via iteration)
    text = re.sub(r'\$([^$]*)\$', r'\1', text)
    # \eqref{...} / \ref{...} -> drop entirely (cross-references add no semantic value)
    text = re.sub(r'\\(?:eq)?ref\{[^}]*\}', '', text)
    # \text{...} -> contents
    text = re.sub(r'\\text\{([^}]*)\}', r'\1', text)
    # \textbf, \textit, \textsc, etc. -> contents
    text = re.sub(r'\\text[a-z]+\{([^}]*)\}', r'\1', text)
    # \scalebox{...}{...} -> second arg
    text = re.sub(r'\\scalebox\{[^}]*\}\{([^}]*)\}', r'\1', text)
    # Remove remaining \cmd{arg} => arg (one level deep)
    text = re.sub(r'\\[a-zA-Z]+\{([^}]*)\}', r'\1', text)
    # Remove remaining bare \cmd sequences
    text = re.sub(r'\\[a-zA-Z]+', '', text)
    # Remove stray braces/brackets
    text = re.sub(r'[{}]', '', text)
    # Collapse multiple spaces
    text = re.sub(r'\s+', ' ', text).strip()
    return text


def infer_metric(ylabel: str) -> str:
    """Heuristically determine the performance metric from the axis y-label."""
    y = ylabel.lower()
    if 'ber' in y and 'bler' in y:
        return 'BLER_or_BER'
    if 'bler' in y or 'fer' in y or 'block' in y:
        return 'BLER'
    if 'ber' in y or 'bit error' in y:
        return 'BER'
    if 'uer' in y or 'undetected' in y:
        return 'UER'
    if 'guess' in y:
        return 'avg_guesses'
    if 'iter' in y:
        return 'avg_iterations'
    if 'simulated' in y or 'empirical' in y:
        return 'simulated_vs_predicted'
    if 'list' in y and ('bler' in y or 'error' in y):
        return 'list_BLER'
    if 'predicted' in y:
        return 'predicted'
    return 'other'


def parse_style(style_str: str) -> dict:
    """Break apart a pgfplots style string into colour, mark, dashed."""
    result = {
        'color': '',
        'mark': '',
        'dashed': False,
    }
    # colour names that pgfplots recognises
    colors = ['black', 'red', 'blue', 'green', 'cyan', 'magenta', 'yellow',
              'brown', 'gray', 'orange', 'purple', 'teal', 'violet']
    for c in colors:
        if re.search(r'\b' + c + r'\b', style_str):
            result['color'] = c
            break
    m = re.search(r'mark\s*=\s*(\S+?)(?:[,\]]|$)', style_str)
    if m:
        result['mark'] = m.group(1)
    if 'dashed' in style_str:
        result['dashed'] = True
    return result


# --------------------------------------------------------------------------- #
# Brace-balanced content extractor                                             #
# --------------------------------------------------------------------------- #

def _extract_brace_content(text: str, cmd_pattern: str) -> Optional[str]:
    """
    Find `cmd_pattern` in `text` and return the brace-balanced content of
    the first `{...}` argument that follows it, handling nested braces.
    Returns None if the command is not found.
    """
    m = re.search(cmd_pattern, text)
    if not m:
        return None
    i = m.end()
    # Skip whitespace to reach '{'
    while i < len(text) and text[i] in ' \t\n':
        i += 1
    if i >= len(text) or text[i] != '{':
        return None
    depth = 0
    start = i + 1
    for j in range(i, len(text)):
        if text[j] == '{':
            depth += 1
        elif text[j] == '}':
            depth -= 1
            if depth == 0:
                return text[start:j]
    return None


# --------------------------------------------------------------------------- #
# Core parser                                                                  #
# --------------------------------------------------------------------------- #

def parse_tex_file(filepath: Path) -> list:
    """Return a list of Curve objects extracted from one .tex file."""
    text = filepath.read_text(encoding='utf-8')

    # Remove block comments \begin{comment}...\end{comment}
    text = re.sub(r'\\begin\{comment\}.*?\\end\{comment\}', '', text,
                  flags=re.DOTALL)

    # Remove line comments (% to end of line), but keep the newline
    text = re.sub(r'%[^\n]*', '', text)

    figure_id = filepath.stem
    curves: list[Curve] = []

    # Split into tikzpicture blocks so we can track axis context per block
    tikz_blocks = re.split(r'\\begin\{tikzpicture\}', text)

    for block_idx, block in enumerate(tikz_blocks[1:], start=1):
        # Extract ylabel from this axis block
        ylabel_m = re.search(r'ylabel\s*=\s*\{?([^},\]]+)\}?', block)
        xlabel_m = re.search(r'xlabel\s*=\s*\{?([^},\]]+)\}?', block)
        axis_ylabel = strip_latex(ylabel_m.group(1)) if ylabel_m else ''
        axis_xlabel = strip_latex(xlabel_m.group(1)) if xlabel_m else 'Eb/N0 (dB)'
        metric = infer_metric(axis_ylabel)

        # Find all \addplot[...] ... table[]{x y ...}; blocks
        # The pattern covers optional \n between components.
        addplot_pattern = re.compile(
            r'\\addplot\s*\[([^\]]*)\]'   # \addplot[style]
            r'(?:\s*\\coordinates\s*\{[^}]*\}|'  # \coordinates variant (not used here)
            r'\s*table\[\]\{x y\s*(.*?)\}'  # table[]{x y ...}
            r')',
            re.DOTALL
        )

        # After each addplot we look for \addlegendentry{...}
        # Build a combined scan
        # We'll iterate manually so we can grab legend text after each block.

        pos = 0
        while True:
            m = re.search(
                r'\\addplot\s*\[([^\]]*)\]\s*table\[\]\{x y\s*(.*?)\}\s*;',
                block[pos:],
                re.DOTALL
            )
            if not m:
                break

            style_str = m.group(1).strip()
            data_str = m.group(2).strip()
            end_pos = pos + m.end()

            # Look for \addlegendentry{...} immediately after the ;
            # Use a brace-balanced extractor to handle nested braces (\eqref{}, etc.)
            suffix = block[end_pos:end_pos + 400]
            legend_raw = _extract_brace_content(suffix, r'\\addlegendentry')
            legend = strip_latex(legend_raw) if legend_raw is not None else ''

            # Skip reference lines (gray dashed diagonal, no legend)
            if not legend and 'gray' in style_str:
                pos = end_pos
                continue

            # Parse data points
            x_vals, y_vals = [], []
            for line in data_str.splitlines():
                line = line.strip()
                if not line:
                    continue
                parts = line.split()
                if len(parts) >= 2:
                    try:
                        x_vals.append(float(parts[0]))
                        y_vals.append(float(parts[1]))
                    except ValueError:
                        pass

            if x_vals:
                curves.append(Curve(
                    legend=legend,
                    x_values=x_vals,
                    y_values=y_vals,
                    style=style_str,
                    metric=metric,
                    axis_xlabel=axis_xlabel,
                    axis_ylabel=axis_ylabel,
                    figure_id=figure_id,
                ))

            pos = end_pos

    return curves


# --------------------------------------------------------------------------- #
# Decoder / code_params extraction from legend text                           #
# --------------------------------------------------------------------------- #

def extract_decoder(legend: str) -> str:
    """Best-effort extraction of decoder type from legend string."""
    legend_lc = legend.lower()
    # GLDPC variants — check before generic LDPC/eBCH checks
    if 'gldpc' in legend_lc and 'sogrand' in legend_lc:
        return 'GLDPC_SOGRAND'
    if 'gldpc' in legend_lc and ('ebch' in legend_lc or 'bch' in legend_lc):
        return 'eBCH_GLDPC'
    if 'gldpc' in legend_lc:
        return 'GLDPC'
    # GRAND variants
    if 'sogrand' in legend_lc:
        return 'SOGRAND'
    if 'orbgrand' in legend_lc and 'pyndiah' in legend_lc:
        return 'ORBGRAND_Pyndiah'
    if 'orbgrand' in legend_lc:
        return 'ORBGRAND'
    # Turbo / iterative
    if 'ca-scl' in legend_lc:
        return 'CA-SCL'
    if 'bcjr' in legend_lc:
        return 'BCJR'
    if 'pyndiah' in legend_lc:
        return 'Pyndiah'
    # LDPC variants
    if 'min-sum' in legend_lc or 'norm-min-sum' in legend_lc:
        return 'LDPC_normMinSum'
    if 'bp' in legend_lc and 'ldpc' in legend_lc:
        return 'LDPC_BP'
    if 'ldpc' in legend_lc and 'bp' in legend_lc:
        return 'LDPC_BP'
    if 'ldpc' in legend_lc:
        return 'LDPC'
    # Polar
    if 'scl' in legend_lc and 'polar' in legend_lc:
        return 'CA-SCL_polar'
    if 'polar' in legend_lc and 'scl' in legend_lc:
        return 'CA-SCL_polar'
    if 'polar' in legend_lc:
        return 'polar_SCL'
    if 'scl' in legend_lc:
        return 'SCL'
    # Other
    if 'bp' in legend_lc:
        return 'BP'
    if 'viterbi' in legend_lc:
        return 'Viterbi'
    if 'forney' in legend_lc:
        return 'Forney'
    # Product codes
    if 'crc' in legend_lc and 'prod' in legend_lc:
        return 'CRC_prod_SOGRAND'
    if 'ebch' in legend_lc and 'prod' in legend_lc:
        return 'eBCH_prod_SOGRAND'
    if 'drm' in legend_lc and 'prod' in legend_lc:
        return 'dRM_prod_SOGRAND'
    if 'ebch' in legend_lc:
        return 'eBCH_SOGRAND'
    if 'drm' in legend_lc:
        return 'dRM_SOGRAND'
    if 'rlc' in legend_lc:
        return 'RLC'
    return legend  # fallback: keep full label


def extract_code_params(legend: str) -> str:
    """Extract code (n,k) or other parameters from legend if present."""
    # Look for (n,k) style
    m = re.search(r'\((\d+,\d+(?:\^\d+)?)\)', legend)
    if m:
        return m.group(0)
    return ''


# --------------------------------------------------------------------------- #
# Write CSV                                                                    #
# --------------------------------------------------------------------------- #

def write_csv(curves: list, out_path: Path, figure_id: str) -> int:
    """Write curves to CSV; return number of rows written."""
    rows = 0
    with open(out_path, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow([
            'figure', 'metric', 'eb_n0_db', 'value',
            'decoder', 'code_params', 'legend_full',
            'style_color', 'style_dashed'
        ])
        for curve in curves:
            style = parse_style(curve.style)
            decoder = extract_decoder(curve.legend)
            code = extract_code_params(curve.legend)
            for x, y in zip(curve.x_values, curve.y_values):
                # For axes where x is not Eb/N0 (predicted/simulated scatter),
                # use x as-is; the column name is still eb_n0_db for uniformity
                # but the true meaning can be read from 'metric'.
                writer.writerow([
                    figure_id,
                    curve.metric,
                    x,
                    y,
                    decoder,
                    code,
                    curve.legend,
                    style['color'],
                    style['dashed'],
                ])
                rows += 1
    return rows


# --------------------------------------------------------------------------- #
# Main                                                                         #
# --------------------------------------------------------------------------- #

# Mapping from .tex filename stem to a human-readable figure label.
# The paper doesn't expose figure numbers directly in the filenames,
# so we assign them based on content / likely ordering.
FIGURE_MAP = {
    'siso':                    'fig_siso_decoder_block',
    'turboDec_col':            'fig_turbodec_column_diagram',
    'L_E':                     'fig_extrinsic_llr_example',
    'BER':                     'fig_ber_bcjr_orbgrand_pyndiah',
    'BER_evaluation1':         'fig_ber_prediction_vs_simulated_scatter',
    'BER_evaluation2':         'fig_ber_evaluation2',
    'BER_evaluation3':         'fig_ber_evaluation3',
    'BER_prediction':          'fig_ber_prediction',
    'UER_evaluation1':         'fig_uer_evaluation1',
    'UER_evaluation2':         'fig_uer_evaluation2',
    'UER_evaluation3':         'fig_uer_evaluation3',
    'uBLER':                   'fig_bler_uer_orbgrand_polar',
    'uBLER_rev':               'fig_bler_uer_orbgrand_polar_rev',
    'list_BLER':               'fig_list_bler_sogrand_forney',
    'crc-detection':           'fig_crc_detection',
    'crc-detection-rev':       'fig_crc_detection_rev',
    'Duffy_differentpara':     'fig_list_bler_rlc_ebch_vs_snr',
    'Duffy_differentpara_rev': 'fig_list_bler_rlc_ebch_vs_snr_rev',
    'Duffy_Forney':            'fig_duffy_forney',
    'prod_grand_16_11':        'fig_prod_ebch_16x11',
    'prod_grand_25_15':        'fig_prod_crc_25x15',
    'prod_grand_22_13':        'fig_prod_crc_22x13',
    'prod_grand_32_21':        'fig_prod_ebch_drm_32x21',
    'prod_grand_32_21_dRM':    'fig_prod_drm_32x21',
    'prod_grand_32_21_dRM_BLER': 'fig_prod_drm_32x21_bler_only',
    'prod_grand_20_64':        'fig_prod_ebch_64x57_sq',
    'prod_grand_20_64_noP':    'fig_prod_ebch_64x57_sq_noP',
    'prod_grand_256_49':       'fig_prod_ebch_256x49',
    'gldpc_GRAND':             'fig_gldpc_sogrand',
}


def main():
    src_dir = Path(os.environ.get('SRC_DIR',
                   '/home/vkaskivuo/Projects/so-grand/img_PY'))
    out_dir = Path(os.environ.get('OUT_DIR',
                   '/home/vkaskivuo/Projects/gf2/dev/reference_data'))
    out_dir.mkdir(parents=True, exist_ok=True)

    tex_files = sorted(src_dir.glob('*.tex'))
    if not tex_files:
        print(f'ERROR: no .tex files found in {src_dir}', file=sys.stderr)
        sys.exit(1)

    total_curves = 0
    total_rows = 0

    for tex_path in tex_files:
        stem = tex_path.stem
        figure_id = FIGURE_MAP.get(stem, f'fig_{stem}')
        curves = parse_tex_file(tex_path)

        # Skip diagram-only files that have no numerical data
        if not curves:
            print(f'  [skip] {stem}  — no plottable data found')
            continue

        out_csv = out_dir / f'{figure_id}.csv'
        rows = write_csv(curves, out_csv, figure_id)
        total_curves += len(curves)
        total_rows += rows
        print(f'  {stem:35s} -> {out_csv.name}  '
              f'({len(curves)} curves, {rows} data points)')

    print(f'\nDone. {total_curves} curves, {total_rows} total rows '
          f'written to {out_dir}')


if __name__ == '__main__':
    main()
