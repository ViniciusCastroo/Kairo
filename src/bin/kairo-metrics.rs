use kairo::performance::processes;

use std::{
    env,
    error::Error,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Erro: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut pid = None;
    let mut samples = 5usize;
    let mut interval = 1.0f64;
    let mut browser = None;
    let mut scenario = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(option) = args.next() {
        if option == "--help" {
            println!(
                "kairo-metrics --pid PID --browser 'Kairo 0.1.0 release' --scenario empty --output run-1.csv [--samples 5] [--interval 1]\nColeta uma execução. Repita em cinco processos novos para comparar execuções independentes."
            );
            return Ok(());
        }
        let value = args.next().ok_or("Opção sem valor")?;
        match option.as_str() {
            "--pid" => pid = Some(value.parse::<u32>()?),
            "--samples" => samples = value.parse()?,
            "--interval" => interval = value.parse()?,
            "--browser" => browser = Some(value),
            "--scenario" => scenario = Some(value),
            "--output" => output = Some(PathBuf::from(value)),
            _ => return Err(format!("Opção desconhecida: {option}").into()),
        }
    }
    if samples == 0 || !interval.is_finite() || !(0.1..=3600.0).contains(&interval) {
        return Err("Use ao menos uma amostra e intervalo entre 0.1 e 3600 segundos".into());
    }
    let pid = pid.filter(|pid| *pid > 0).ok_or("Informe --pid PID")?;
    let browser = browser.ok_or("Informe --browser com nome, versão e tipo de build")?;
    let scenario = scenario.ok_or("Informe --scenario")?;
    let output = output.ok_or("Informe --output")?;
    let mut before = processes::snapshot(pid)?;
    // Nunca sobrescreve resultados de outra execução.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(processes::Snapshot::csv_header().as_bytes())?;
    for sample in 1..=samples {
        std::thread::sleep(Duration::from_secs_f64(interval));
        let after = processes::snapshot(pid)?;
        if after.root_start != before.root_start {
            return Err(io::Error::other("PID raiz foi reutilizado; coleta interrompida").into());
        }
        let cpu = processes::cpu_percent(&before, &after);
        file.write_all(after.csv_rows(sample, &browser, &scenario, cpu).as_bytes())?;
        file.flush()?;
        println!("Amostra {sample}/{samples}\n{}", after.report(cpu));
        before = after;
    }
    println!("Dados salvos em {}", output.display());
    Ok(())
}
