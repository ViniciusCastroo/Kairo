#!/usr/bin/env bash
set -euo pipefail

failed=0

check_tool() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        "$tool" --version
    else
        printf 'FALTA: %s\n' "$tool" >&2
        failed=1
    fi
}

check_tool rustc
check_tool cargo
check_tool rustfmt
check_tool cc
check_tool glib-compile-resources

if command -v cargo >/dev/null 2>&1; then
    if ! cargo clippy --version; then
        printf 'FALTA: Clippy (rust-clippy no Ubuntu, ou rustup component add clippy)\n' >&2
        failed=1
    fi
fi

if command -v pkg-config >/dev/null 2>&1; then
    for library in gtk4 webkitgtk-6.0 sqlite3; do
        if version=$(pkg-config --modversion "$library"); then
            printf '%s: %s\n' "$library" "$version"
            if [[ "$library" == gtk4 ]] && ! pkg-config --atleast-version=4.10 gtk4; then
                printf 'GTK4 4.10 ou superior é necessário.\n' >&2
                failed=1
            fi
            if [[ "$library" == webkitgtk-6.0 ]] && ! pkg-config --atleast-version=2.46 webkitgtk-6.0; then
                printf 'WebKitGTK 2.46 ou superior é necessário.\n' >&2
                failed=1
            fi
        else
            printf 'FALTA: biblioteca de desenvolvimento %s\n' "$library" >&2
            failed=1
        fi
    done
else
    printf 'FALTA: pkg-config (necessário para verificar GTK4 e WebKitGTK)\n' >&2
    failed=1
fi

if [[ "$failed" -ne 0 ]]; then
    printf '\nAmbiente incompleto. Consulte as instruções no README.md.\n' >&2
    exit 1
fi

printf '\nDependências encontradas. Próximo passo: cargo check\n'
