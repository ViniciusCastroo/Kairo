#!/usr/bin/env bash
# Compara memória (PSS) e CPU do Kairo com Chrome, Brave e Firefox.
# Cada execução usa um perfil novo e descartável, a mesma janela e as mesmas URLs.
# Uso: scripts/compare-browsers.sh [execuções=5] [estabilização_s=15]
set -euo pipefail
cd "$(dirname "$0")/.."
runs=${1:-5}
settle=${2:-15}
out="benchmarks/results/$(date +%F)-compare"
[[ -e $out ]] && { echo "$out já existe; mova ou apague antes." >&2; exit 1; }
cargo build --release --locked -q

declare -A scenarios=(
    [empty]="about:blank"
    [one-site]="https://pt.wikipedia.org/wiki/Linux"
    [five-sites]="https://pt.wikipedia.org/wiki/Linux https://github.com/rust-lang/rust https://developer.mozilla.org/pt-BR/docs/Web/HTML https://news.ycombinator.com https://www.bbc.com/news"
)
browsers=(kairo)
command -v google-chrome >/dev/null && browsers+=(chrome)
command -v brave-browser >/dev/null && browsers+=(brave)
command -v firefox >/dev/null && browsers+=(firefox)

label() {
    case $1 in
        kairo) echo "Kairo $(cargo pkgid | sed 's/.*[#@]//') release" ;;
        chrome) google-chrome --version ;;
        brave) brave-browser --version ;;
        firefox) firefox --version 2>/dev/null ;;
    esac
}

# Perfil do Firefox snap precisa ficar em ~/snap; os demais aceitam qualquer pasta.
profile_root="$HOME/snap/firefox/common/kairo-bench"
trap 'rm -rf "$profile_root"' EXIT

launch() { # navegador perfil urls... → imprime PID
    local browser=$1 profile=$2; shift 2
    case $browser in
        kairo) XDG_DATA_HOME=$profile/data XDG_CACHE_HOME=$profile/cache XDG_CONFIG_HOME=$profile/config \
            exec setsid ./target/release/kairo "$@" ;;
        chrome | brave)
            local bin=google-chrome; [[ $browser == brave ]] && bin=brave-browser
            exec setsid "$bin" --user-data-dir="$profile" --no-first-run --no-default-browser-check \
                --window-size=1100,740 "$@" ;;
        firefox)
            mkdir -p "$profile"
            cat >"$profile/user.js" <<'EOF'
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
user_pref("trailhead.firstrun.didSeeAboutWelcome", true);
EOF
            exec setsid firefox --new-instance --profile "$profile" -width 1100 -height 740 "$@" ;;
    esac >/dev/null 2>&1 &
    echo $!
}

# Execuções intercaladas entre navegadores para diluir variações da máquina.
for run in $(seq "$runs"); do
    for scenario in "${!scenarios[@]}"; do
        for browser in "${browsers[@]}"; do
            dir="$out/$browser/$scenario"; mkdir -p "$dir"
            profile="$profile_root/$browser-$scenario-$run"
            # shellcheck disable=SC2086
            pid=$(launch "$browser" "$profile" ${scenarios[$scenario]})
            sleep "$settle"
            echo "[$run/$runs] $browser $scenario (pid $pid)"
            ./target/release/kairo-metrics --pid "$pid" --browser "$(label "$browser")" \
                --scenario "$scenario" --samples 5 --interval 1 --output "$dir/run-$run.csv" ||
                echo "  coleta falhou" >&2
            kill -TERM -- "-$pid" 2>/dev/null || true
            sleep 3
            kill -KILL -- "-$pid" 2>/dev/null || true
            rm -rf "$profile"
        done
    done
done

# Um resumo por navegador/cenário e uma tabela comparativa (PSS e CPU medianos).
{
    echo "# Comparação de navegadores — $(date +%F)"
    echo
    echo "$runs execuções por navegador e cenário, perfis novos, ${settle}s de estabilização,"
    echo "5 amostras de 1s. $(uname -r); $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs)."
    echo
    for browser in "${browsers[@]}"; do
        for scenario in "${!scenarios[@]}"; do
            python3 scripts/summarize-benchmarks.py "$out/$browser/$scenario"/run-*.csv \
                --output "$out/$browser/$scenario/summary.md" >/dev/null 2>&1 || true
        done
    done
    # Mediana entre execuções da mediana por execução, como o resumo oficial, mas
    # somando só o PSS legível: auxiliares setuid-root (chrome-sandbox) não são legíveis.
    python3 - "$out" "${browsers[@]}" <<'EOF'
import csv, glob, statistics, sys
out, browsers = sys.argv[1], sys.argv[2:]
print("| Cenário | Navegador | PSS mediana (MiB) | CPU mediana (%) | Processos |")
print("| --- | --- | ---: | ---: | ---: |")
note = False
for scenario in ("empty", "one-site", "five-sites"):
    for browser in browsers:
        runs, name = [], browser
        for path in sorted(glob.glob(f"{out}/{browser}/{scenario}/run-*.csv")):
            samples = {}
            for row in csv.DictReader(open(path, newline="", encoding="utf-8")):
                name = row["browser"].strip()
                sample = samples.setdefault(row["sample"], {"pss": 0, "hidden": 0})
                if row["kind"] == "total":
                    sample["cpu"], sample["procs"] = row["cpu_percent"], int(row["processes"])
                elif row["pss_kib"]:
                    sample["pss"] += int(row["pss_kib"])
                else:
                    sample["hidden"] += 1
            samples = [s for s in samples.values() if s.get("cpu")]
            if samples:
                runs.append([statistics.median(float(s[k]) for s in samples) for k in ("pss", "cpu", "procs", "hidden")])
        if not runs:
            print(f"| {scenario} | {name} | falhou | — | — |")
            continue
        pss, cpu, procs, hidden = (statistics.median(column) for column in zip(*runs))
        mark = " ¹" if hidden else ""
        note |= bool(hidden)
        print(f"| {scenario} | {name} | {pss / 1024:.0f}{mark} | {cpu:.1f} | {procs:.0f} |")
if note:
    print("\n¹ Sem o PSS de processos ilegíveis para o usuário (auxiliar setuid `chrome-sandbox`, ~2 MiB de RSS).")
EOF
    echo
    echo "PSS soma a memória de toda a árvore de processos, dividindo páginas compartilhadas."
    echo "CPU: 100% = um núcleo. Resumos com IQR em <navegador>/<cenário>/summary.md."
    echo "Sites reais mudam entre execuções; trate diferenças pequenas como ruído."
} >"$out/summary.md"
cat "$out/summary.md"
