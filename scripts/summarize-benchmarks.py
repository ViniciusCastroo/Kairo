#!/usr/bin/env python3
"""Resume medianas por execução; exige cinco processos independentes."""
import argparse
import csv
import math
from pathlib import Path
import statistics


def summarize(paths):
    if len(paths) < 5:
        raise ValueError("São necessárias pelo menos cinco execuções independentes.")
    identities = set()
    configuration = None
    runs = []
    for path in paths:
        with Path(path).open(newline="", encoding="utf-8") as source:
            rows = [row for row in csv.DictReader(source) if row.get("kind") == "total"]
        if not rows:
            raise ValueError(f"Nenhuma amostra total em {path}")
        current = (rows[0]["browser"], rows[0]["scenario"], rows[0]["kernel"])
        identity = (rows[0]["boot_id"], rows[0]["root_pid"], rows[0]["root_start_ticks"])
        if not all(identity) or identity in identities:
            raise ValueError("Arquivos repetem o mesmo processo ou não identificam a execução.")
        identities.add(identity)
        if configuration is not None and configuration != current:
            raise ValueError("Não misture navegador/build, cenário ou kernel no mesmo resumo.")
        configuration = current
        values = {key: [] for key in ("pss_kib", "rss_kib", "cpu_percent")}
        for row in rows:
            if (row["browser"], row["scenario"], row["kernel"]) != current or (row["boot_id"], row["root_pid"], row["root_start_ticks"]) != identity:
                raise ValueError("A identidade mudou dentro do arquivo.")
            for key in values:
                value = float(row[key]) if row[key] else None
                if value is not None and (not math.isfinite(value) or value < 0):
                    raise ValueError(f"Valor inválido em {key}")
                values[key].append(value)
        runs.append({key: statistics.median(numbers) if all(number is not None for number in numbers) else None for key, numbers in values.items()})
    lines = [f"# {configuration[0]}", "", f"Cenário: {configuration[1]}", f"Kernel: {configuration[2]}", f"Execuções independentes: {len(runs)}", "", "Mediana das medianas por execução; IQR com quartis inclusivos.", "", "| Métrica | Mediana | IQR | Mínimo–máximo |", "| --- | ---: | ---: | ---: |"]
    for key, label, divisor in [("pss_kib", "PSS (MiB)", 1024), ("rss_kib", "RSS (MiB)", 1024), ("cpu_percent", "CPU (%)", 1)]:
        values = [run[key] / divisor for run in runs if run[key] is not None]
        if len(values) != len(runs):
            lines.append(f"| {label} | indisponível: amostras incompletas | — | — |")
            continue
        q1, _, q3 = statistics.quantiles(values, n=4, method="inclusive")
        lines.append(f"| {label} | {statistics.median(values):.2f} | {q3 - q1:.2f} | {min(values):.2f}–{max(values):.2f} |")
    lines += ["", "CPU: 100% representa um núcleo. RSS pode contar páginas compartilhadas repetidas vezes.", "Os arquivos não comprovam por si só perfis limpos, URLs idênticas ou condições de hardware iguais."]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = summarize(args.files)
        if args.output:
            with args.output.open("x", encoding="utf-8") as destination:
                destination.write(report)
        else:
            print(report, end="")
    except (OSError, ValueError, KeyError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
