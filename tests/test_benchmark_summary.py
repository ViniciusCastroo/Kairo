import csv
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("summary", Path(__file__).resolve().parents[1] / "scripts/summarize-benchmarks.py")
summary = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(summary)


class SummaryTests(unittest.TestCase):
    def fixtures(self, directory):
        paths = []
        for index in range(5):
            path = Path(directory) / f"run-{index}.csv"
            with path.open("w", newline="") as file:
                writer = csv.DictWriter(file, fieldnames=["kind", "browser", "scenario", "kernel", "boot_id", "root_pid", "root_start_ticks", "pss_kib", "rss_kib", "cpu_percent"])
                writer.writeheader()
                for sample in [1, 3]:
                    writer.writerow(dict(kind="total", browser="test", scenario="empty", kernel="test-kernel", boot_id="boot", root_pid=index + 1, root_start_ticks=index, pss_kib=(index + sample) * 1024, rss_kib=4096, cpu_percent=0))
            paths.append(path)
        return paths

    def test_summary_uses_median_of_independent_runs(self):
        with tempfile.TemporaryDirectory() as directory:
            report = summary.summarize(self.fixtures(directory))
            self.assertIn("| PSS (MiB) | 4.00 | 2.00 | 2.00–6.00 |", report)

    def test_repeated_processes_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.fixtures(directory)
            with self.assertRaises(ValueError):
                summary.summarize([paths[0]] * 5)

    def test_missing_pss_never_becomes_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = self.fixtures(directory)
            text = paths[0].read_text().replace("1024,4096", ",4096")
            paths[0].write_text(text)
            self.assertIn("| PSS (MiB) | indisponível", summary.summarize(paths))


if __name__ == "__main__":
    unittest.main()
