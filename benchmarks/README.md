# Medições locais

Menu → Desempenho mede a árvore de processos visível em `/proc` e permite salvar
um relatório de texto. A coleta é feita sob demanda em uma thread de trabalho;
não existe envio remoto nem amostragem contínua. Há contadores de carregamentos,
falhas, cancelamentos e tempo até mapear a primeira janela, medido a partir do
início de `main`. Mapear a janela é uma aproximação de startup, não uma prova de
que a primeira página já está utilizável.

## Coletor externo

```bash
cargo build --release --locked
pgrep -a -x kairo
./target/release/kairo-metrics --pid 12345 --browser 'Kairo 0.1.0 release' --scenario empty --samples 5 --interval 1 --output run-1.csv
```

Substitua o PID pelo processo principal correto. O coletor não inicia nem
encerra navegadores, não modifica perfis e não sobrescreve resultados. Um
comando interrompido pode deixar CSV parcial: descarte-o antes de resumir.
Repita a coleta em cinco **novos processos**, em condições iguais. Cinco
amostras do mesmo processo não equivalem a cinco execuções independentes.

```bash
python3 scripts/summarize-benchmarks.py run-1.csv run-2.csv run-3.csv run-4.csv run-5.csv --output summary.md
```

O resumo calcula primeiro a mediana de cada execução, depois a mediana,
IQR e intervalo entre execuções. Arquivos com o mesmo PID/início/boot são
recusados, assim como misturas de navegador/build, cenário ou kernel.

## Protocolo de comparação

Use builds release, o mesmo computador, sistema, energia, resolução e conjunto
de URLs. Registre versões, hardware e perfis limpos, sem extensões. Aguarde o
carregamento e um intervalo fixo de estabilização antes de iniciar cada coleta.
Encerre a instância antes de cada nova repetição e confirme o PID principal;
navegadores podem encaminhar uma segunda execução para uma instância existente.

Faça cinco repetições para janela vazia, um site simples, cinco sites, dez abas
com nove inativas e sessenta minutos de navegação. Registre também falhas e
compatibilidade. O coletor cobre memória e CPU; startup e carregamento requerem
instrumentação equivalente em cada navegador para uma comparação justa.

PSS é preferida para memória compartilhada; somar RSS pode contar as mesmas
páginas várias vezes. Se qualquer processo estiver inacessível ou desaparecer
durante a leitura, o total correspondente fica indisponível. CPU usa deltas
apenas quando as identidades dos processos coincidem entre amostras; 100%
representa um núcleo. Serviços compartilhados fora da árvore, processos ocultos
por permissões e processos que nascem e morrem entre amostras não são capturados.
`/proc` não fornece uma fotografia atômica de toda a árvore. O painel interno
inclui o custo do próprio medidor; prefira o coletor externo para benchmarks.

Fonte do formato e das definições: [documentação do kernel Linux sobre /proc](https://docs.kernel.org/filesystems/proc.html).

## Comparação com outros navegadores

```bash
scripts/compare-browsers.sh [execuções=5] [estabilização_s=15]
```

Abre Kairo (release), Chrome, Brave e Firefox, os que estiverem instalados, cada um
com perfil novo e descartável, janela de 1100 × 740 e as mesmas URLs, em três
cenários: `about:blank`, um artigo da Wikipédia e cinco sites em abas. Depois de
estabilizar, coleta 5 amostras com `kairo-metrics`, encerra o grupo de processos e
apaga o perfil. As execuções são intercaladas entre navegadores. O resultado vai para
`benchmarks/results/<data>-compare/`: CSVs, um `summary.md` por navegador/cenário e
uma tabela comparativa. O Brave usa um auxiliar setuid-root (`chrome-sandbox`) cujo
PSS não é legível. A tabela soma apenas o PSS legível e marca esses casos; o resumo
oficial daquele grupo mostra o PSS como indisponível.

Limites: sites reais variam entre execuções; navegadores Chromium podem adiar o
carregamento de abas em segundo plano; a sessão gráfica não é isolada de outros
aplicativos. Feche outras instâncias pesadas antes de rodar.

## Verificação inicial

[Dados e resumo de cinco execuções](results/2026-11-13-smoke/summary.md) validam o
fluxo de coleta: dez abas `about:blank` no executável de integração GTK **debug**,
perfil SQLite em memória, um segundo de estabilização e uma amostra após mais
um segundo, por execução. CPU Intel i5-1235U, aproximadamente 14,8 GiB de RAM,
Ubuntu 26.04.1, kernel 7.0.0-43, GTK 4.22.4, WebKitGTK 2.52.6 e Rust 1.93.1.
O sandbox permaneceu habilitado. O ensaio foi feito na sessão gráfica de
desenvolvimento, sem controle da carga dos demais aplicativos.

Esses números são uma referência do teste, não um benchmark release do produto
nem uma comparação com Firefox/Chromium. Os cenários reais e comparativos
continuam pendentes; não há promessa pública de consumo de RAM.

Para reproduzir uma execução do teste:

```bash
cargo test --locked --test gtk-tabs -- --run-gtk --metrics-output /tmp/kairo-smoke-run-1.csv
```
