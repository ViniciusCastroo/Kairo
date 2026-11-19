# Comparação de navegadores — 2026-11-19

5 execuções por navegador e cenário, perfis novos, 15s de estabilização,
5 amostras de 1s. 7.0.0-43-generic; 12th Gen Intel(R) Core(TM) i5-1235U.

| Cenário | Navegador | PSS mediana (MiB) | CPU mediana (%) | Processos |
| --- | --- | ---: | ---: | ---: |
| empty | Kairo 0.1.0 release | 296 | 0.0 | 8 |
| empty | Google Chrome 157.0.8103.50 | 416 | 7.2 | 16 |
| empty | Brave Browser 157.1.99.56 | 387 ¹ | 0.0 | 13 |
| empty | Mozilla Firefox 158.0 | 665 | 3.8 | 12 |
| one-site | Kairo 0.1.0 release | 365 | 21.2 | 8 |
| one-site | Google Chrome 157.0.8103.50 | 485 | 41.0 | 17 |
| one-site | Brave Browser 157.1.99.56 | 394 ¹ | 25.0 | 13 |
| one-site | Mozilla Firefox 158.0 | 758 | 34.9 | 12 |
| five-sites | Kairo 0.1.0 release | 897 | 71.7 | 32 |
| five-sites | Google Chrome 157.0.8103.50 | 725 | 29.5 | 21 |
| five-sites | Brave Browser 157.1.99.56 | 386 ¹ | 22.9 | 17 |
| five-sites | Mozilla Firefox 158.0 | 1129 | 58.4 | 17 |

¹ Sem o PSS de processos ilegíveis para o usuário (auxiliar setuid `chrome-sandbox`, ~2 MiB de RSS).

PSS soma a memória de toda a árvore de processos, dividindo páginas compartilhadas.
CPU: 100% = um núcleo. Resumos com IQR em <navegador>/<cenário>/summary.md.
Sites reais mudam entre execuções; trate diferenças pequenas como ruído.
