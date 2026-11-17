use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub parent: u32,
    pub name: String,
    pub start_ticks: u64,
    pub cpu_ticks: u64,
    pub pss_kib: Option<u64>,
    pub rss_kib: Option<u64>,
}

#[derive(Clone)]
pub struct Snapshot {
    pub root: u32,
    pub root_start: u64,
    pub boot_id: String,
    pub kernel: String,
    pub unix_ms: u128,
    pub processes: Vec<Process>,
    pub collection_ms: f64,
    captured: Instant,
    clock_ticks: u64,
}

impl Snapshot {
    pub fn pss_kib(&self) -> Option<u64> {
        self.processes.iter().map(|process| process.pss_kib).sum()
    }

    pub fn rss_kib(&self) -> Option<u64> {
        self.processes.iter().map(|process| process.rss_kib).sum()
    }

    pub fn report(&self, cpu: Option<f64>) -> String {
        let memory = |value: Option<u64>| {
            value.map_or_else(
                || "indisponível: coleta incompleta".into(),
                |kib| format!("{:.1} MiB", kib as f64 / 1024.0),
            )
        };
        let mut report = format!(
            "Processos: {}\nPSS total: {}\nRSS total: {}\nCPU no intervalo: {}\nTempo de coleta: {:.1} ms\n",
            self.processes.len(),
            memory(self.pss_kib()),
            memory(self.rss_kib()),
            cpu.map_or_else(
                || "indisponível: processos mudaram".into(),
                |value| format!("{value:.1}% (100% = um núcleo)")
            ),
            self.collection_ms
        );
        report.push_str("\nPID / pai — processo — PSS / RSS\n");
        for process in &self.processes {
            report.push_str(&format!(
                "{} / {} — {} — {} / {}\n",
                process.pid,
                process.parent,
                process.name,
                memory(process.pss_kib),
                memory(process.rss_kib)
            ));
        }
        report
    }

    pub fn csv_header() -> &'static str {
        "kind,sample,browser,scenario,unix_ms,boot_id,kernel,root_pid,root_start_ticks,pid,ppid,name,processes,pss_kib,rss_kib,cpu_percent,collection_ms\n"
    }

    pub fn csv_rows(
        &self,
        sample: usize,
        browser: &str,
        scenario: &str,
        cpu: Option<f64>,
    ) -> String {
        let field = |value: &str| format!("\"{}\"", value.replace('"', "\"\""));
        let integer =
            |value: Option<u64>| value.map(|number| number.to_string()).unwrap_or_default();
        let common = format!(
            "{sample},{},{},{},{},{},{},{}",
            field(browser),
            field(scenario),
            self.unix_ms,
            field(&self.boot_id),
            field(&self.kernel),
            self.root,
            self.root_start
        );
        let mut rows = format!(
            "total,{common},{},,,{},{},{},{},{:.3}\n",
            self.root,
            self.processes.len(),
            integer(self.pss_kib()),
            integer(self.rss_kib()),
            cpu.map(|value| format!("{value:.3}")).unwrap_or_default(),
            self.collection_ms
        );
        for process in &self.processes {
            rows.push_str(&format!(
                "process,{common},{},{},{},,{},{},,\n",
                process.pid,
                process.parent,
                field(&process.name),
                integer(process.pss_kib),
                integer(process.rss_kib)
            ));
        }
        rows
    }
}

/// CPU só é publicada quando as mesmas identidades de processo existem nas duas amostras.
pub fn cpu_percent(before: &Snapshot, after: &Snapshot) -> Option<f64> {
    if before.root != after.root
        || before.root_start != after.root_start
        || before.boot_id != after.boot_id
        || before.clock_ticks != after.clock_ticks
    {
        return None;
    }
    let previous: HashMap<_, _> = before
        .processes
        .iter()
        .map(|process| ((process.pid, process.start_ticks), process.cpu_ticks))
        .collect();
    if previous.len() != after.processes.len() {
        return None;
    }
    let delta: Option<u64> = after
        .processes
        .iter()
        .map(|process| {
            process
                .cpu_ticks
                .checked_sub(*previous.get(&(process.pid, process.start_ticks))?)
        })
        .sum();
    let seconds = after
        .captured
        .checked_duration_since(before.captured)?
        .as_secs_f64();
    if seconds == 0.0 {
        return None;
    }
    Some(delta? as f64 / before.clock_ticks as f64 / seconds * 100.0)
}

pub fn snapshot(root: u32) -> io::Result<Snapshot> {
    collect(Path::new("/proc"), root)
}

fn collect(proc: &Path, root: u32) -> io::Result<Snapshot> {
    let started = Instant::now();
    let mut all = HashMap::new();
    for entry in fs::read_dir(proc)? {
        let Ok(entry) = entry else {
            continue;
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(stat) = fs::read_to_string(entry.path().join("stat"))
            && let Ok(process) = parse_stat(pid, &stat)
        {
            all.insert(pid, process);
        }
    }
    let root_start = all
        .get(&root)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "PID raiz não encontrado ou sem acesso",
            )
        })?
        .start_ticks;
    let ids = descendants(&all, root);
    let mut processes = Vec::new();
    for pid in ids {
        let mut process = all.remove(&pid).expect("processo selecionado existe");
        let directory = proc.join(pid.to_string());
        if let Ok(memory) = fs::read_to_string(directory.join("smaps_rollup")) {
            process.pss_kib = memory_field(&memory, "Pss:");
            process.rss_kib = memory_field(&memory, "Rss:");
        }
        if process.rss_kib.is_none() {
            process.rss_kib = fs::read_to_string(directory.join("status"))
                .ok()
                .and_then(|status| memory_field(&status, "VmRSS:"));
        }
        // Evita combinar stat de um processo com memória de outro após reutilização do PID.
        if fs::read_to_string(directory.join("stat"))
            .ok()
            .and_then(|stat| parse_stat(pid, &stat).ok())
            .is_none_or(|current| current.start_ticks != process.start_ticks)
        {
            process.pss_kib = None;
            process.rss_kib = None;
        }
        processes.push(process);
    }
    processes.sort_by_key(|process| process.pid);
    // sysconf não recebe ponteiros nem modifica memória Rust.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks <= 0 {
        return Err(io::Error::other("Não foi possível consultar CLK_TCK"));
    }
    Ok(Snapshot {
        root,
        root_start,
        processes,
        boot_id: fs::read_to_string(proc.join("sys/kernel/random/boot_id"))
            .unwrap_or_default()
            .trim()
            .into(),
        kernel: fs::read_to_string(proc.join("sys/kernel/osrelease"))
            .unwrap_or_default()
            .trim()
            .into(),
        unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis(),
        captured: Instant::now(),
        collection_ms: started.elapsed().as_secs_f64() * 1000.0,
        clock_ticks: ticks as u64,
    })
}

fn descendants(all: &HashMap<u32, Process>, root: u32) -> HashSet<u32> {
    let mut selected = HashSet::from([root]);
    loop {
        let size = selected.len();
        for process in all.values() {
            if selected.contains(&process.parent) {
                selected.insert(process.pid);
            }
        }
        if selected.len() == size {
            return selected;
        }
    }
}

fn parse_stat(pid: u32, stat: &str) -> io::Result<Process> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Conteúdo inválido em /proc/PID/stat",
        )
    };
    let first = stat.find('(').ok_or_else(invalid)?;
    let last = stat.rfind(')').ok_or_else(invalid)?;
    if first >= last {
        return Err(invalid());
    }
    let fields: Vec<_> = stat[last + 1..].split_whitespace().collect();
    let number = |index| {
        fields
            .get(index)
            .and_then(|value: &&str| value.parse::<u64>().ok())
            .ok_or_else(invalid)
    };
    Ok(Process {
        pid,
        parent: u32::try_from(number(1)?).map_err(|_| invalid())?,
        name: stat[first + 1..last].into(),
        start_ticks: number(19)?,
        cpu_ticks: number(11)?.saturating_add(number(12)?),
        pss_kib: None,
        rss_kib: None,
    })
}

fn memory_field(text: &str, name: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        if fields.next()? != name {
            return None;
        }
        let value = fields.next()?.parse().ok()?;
        (fields.next()? == "kB").then_some(value)
    })
}

#[cfg(test)]
mod tests {
    fn process(pid: u32, parent: u32) -> super::Process {
        super::Process {
            pid,
            parent,
            name: "test".into(),
            start_ticks: 10,
            cpu_ticks: 1,
            pss_kib: Some(10),
            rss_kib: Some(20),
        }
    }

    #[test]
    fn parser_handles_spaces_parentheses_and_cpu_fields() {
        let mut fields = vec!["0"; 22];
        fields[0] = "S";
        fields[1] = "7";
        fields[11] = "23";
        fields[12] = "11";
        fields[19] = "999";
        let process =
            super::parse_stat(42, &format!("42 (test ) process) {}", fields.join(" "))).unwrap();
        assert_eq!(process.name, "test ) process");
        assert_eq!(process.parent, 7);
        assert_eq!(process.cpu_ticks, 34);
        assert_eq!(process.start_ticks, 999);
        assert!(super::parse_stat(42, "bad data").is_err());
    }

    #[test]
    fn tree_includes_grandchildren_and_excludes_unrelated_processes() {
        let all = [
            (1, process(1, 0)),
            (2, process(2, 1)),
            (3, process(3, 2)),
            (4, process(4, 0)),
        ]
        .into_iter()
        .collect();
        assert_eq!(super::descendants(&all, 1), [1, 2, 3].into_iter().collect());
        assert_eq!(
            super::memory_field("Pss_Anon: 100 kB\nPss: 25 kB\n", "Pss:"),
            Some(25)
        );
        assert_eq!(super::memory_field("Pss: unavailable", "Pss:"), None);
    }

    #[test]
    fn partial_pss_is_not_reported_as_a_complete_total() {
        let mut snapshot = super::snapshot(std::process::id()).unwrap();
        snapshot.processes = vec![process(1, 0), process(2, 1)];
        assert_eq!(snapshot.pss_kib(), Some(20));
        snapshot.processes[1].pss_kib = None;
        assert_eq!(snapshot.pss_kib(), None);
        assert_eq!(snapshot.rss_kib(), Some(40));
    }

    #[test]
    fn cpu_detects_churn_and_pid_reuse() {
        let before = super::snapshot(std::process::id()).unwrap();
        let mut after = before.clone();
        after.captured += std::time::Duration::from_secs(1);
        after.processes[0].cpu_ticks += after.clock_ticks;
        assert!((super::cpu_percent(&before, &after).unwrap() - 100.0).abs() < 0.001);
        after.processes[0].start_ticks += 1;
        assert!(super::cpu_percent(&before, &after).is_none());
        after.processes.pop();
        assert!(super::cpu_percent(&before, &after).is_none());
    }
}
