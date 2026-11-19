# Roteiro inicial

## Estado atual em relação ao guia

| Parte do guia | Estado |
| --- | --- |
| Bootstrap, HTTPS e navegação | Implementados e testados no protótipo |
| Abas e pop-ups | Implementados; casos reais de pop-ups e confirmação de saída ainda precisam de validação |
| Histórico, preferências e sessões | Implementados; reinício confirmado pelo usuário |
| Métricas e benchmark | Comparação release com Chrome, Brave e Firefox em três cenários (`scripts/compare-browsers.sh`); dez abas com nove inativas e sessenta minutos de navegação pendentes |
| Descarte seguro de abas | Restauração sob demanda e descarte após 15 min de inatividade, com exceções para áudio, captura, formulários editados e tarefas ativas; testado no GTK |
| Downloads e permissões | Primeira implementação pronta; testes locais de transferência, cancelamento e recusa de localização passaram; dispositivos e portal ainda exigem aceite manual |
| Privacidade e TLS | Certificado inválido abre aviso com exceção só na sessão e segunda verificação no commit da página; janela privada isolada e limpeza de histórico, sessão e dados dos sites implementadas e testadas localmente |
| Flatpak, CI e alpha | Pendentes, incluindo licença, identidade final e rotina de atualizações |

Extras já implementados: interface escura, tela inicial local, escolha de
Google/DuckDuckGo e favoritos editáveis persistidos em SQLite. Trinta e um
testes de lógica passaram; o teste GTK verificou o fluxo de favoritos,
ordenação, edição, remoção, restauração e bloqueio de chamadas de sites externos.
A marca enviada pelo usuário está integrada à tela inicial e aos recursos de
ícone. Os SVGs originais foram preservados; a identidade do lançador depende do
empacotamento desktop/Flatpak.
Os controles e a busca local usam um subconjunto de 13 SVGs do Lucide adaptados ao GTK (7.950
bytes), embarcado sem React ou novas dependências. Os controles da janela
continuam seguindo o GTK. O consumo total ainda depende do benchmark release.
A tipografia principal é Roboto, com um único TTF latino variável local
(80.500 bytes), compartilhado pelo GTK e pela tela inicial, sem download em
tempo de execução ou instalação global. Os sites mantêm a própria tipografia.

## Detalhes das entregas

Base: guia de arquitetura de 13/11/2026. As semanas representam etapas planejadas,
sem datas de entrega assumidas. O bootstrap compila no ambiente local e passou
por formatação e Clippy. O usuário confirmou a renderização da página HTTPS;
faltam os testes manuais de segunda ativação e fechamento da janela para
concluir o aceite da etapa 1.

A etapa 2 está implementada: barra de endereço com pesquisa no DuckDuckGo,
validação de URLs HTTP/HTTPS, navegação, atalhos e estados de carregamento.
Os testes automatizados cobrem a interpretação da entrada. O usuário confirmou
abertura do site Rust, voltar/avançar, recarregamento e pesquisa. Os demais
cenários gráficos, incluindo erros TLS, estão descritos no README.

A etapa 3 está implementada: modelo de abas independente de GTK, uma WebView
por aba, abertura/fechamento, alternância e pop-ups por interação do usuário.
Sete testes de lógica e um teste GTK com dez abas passaram. O teste gráfico
verificou carregamento, seleção, isolamento de erros e fechamento; links em nova
aba, pop-ups e confirmação de saída aguardam aceite manual com a página local.
O usuário confirmou o funcionamento da versão com abas; os casos específicos
de pop-ups e confirmação de saída permanecem pendentes.

A etapa 4 está implementada: histórico, preferências e sessão em SQLite,
com telas de consulta e configuração. Treze testes de lógica cobrem também
persistência após reabertura, falhas de transação e URLs inseguras no perfil.
O teste GTK verificou restauração de ordem e seleção das abas, histórico e
desativação da restauração com páginas locais. O aceite manual de reinício
no perfil do usuário está descrito no README.

A etapa 5 tem painel de desempenho sob demanda, coletor CSV externo e resumo
de execuções independentes. Dezoito testes Rust, três testes do resumo e os
testes GTK passaram. Foram registradas cinco execuções do ensaio GTK debug
com dez abas vazias; os cenários release e a comparação entre navegadores
seguem pendentes. Protocolo e limites em `benchmarks/README.md`.

A interface foi ajustada à referência visual enviada pelo usuário: abas no
topo da janela, barra escura compacta e campo de endereço arredondado. Histórico,
preferências e desempenho conservam os controles explícitos de fechamento.

A tela inicial local está implementada com busca no DuckDuckGo, relógio/data,
fundo escuro e atalhos recentes. Novas abas a utilizam; páginas iniciais
personalizadas são preservadas. Vinte testes de lógica e a integração GTK
verificaram renderização, relógio, pesquisa com acentos, navegação pelos atalhos
e restauração da tela interna. Não há contadores simulados de bloqueio.

Google foi adicionado como alternativa ao DuckDuckGo nas preferências. A opção
é persistida no esquema SQLite 2, com migração dos dados anteriores. Vinte e dois
testes de lógica e o teste GTK verificaram a troca pela interface, consultas
com acentos nas duas caixas de busca, atualização da tela aberta e reinício.

Downloads, permissões básicas e mensagem de certificado inválido foram
implementados e passaram pelo teste GTK local. Permanecem pendentes o aceite
do seletor/portal, acesso físico a dispositivos e notificações reais no desktop.

Janelas privadas usam sessões efêmeras independentes, sem acesso ao banco do
perfil. A limpeza fecha as abas normais e remove histórico, sessão e todos os
tipos de dados de sites pelo WebKit, preservando favoritos e preferências.
O teste GTK verificou cookies e localStorage isolados, compartilhamento entre
abas privadas, liberação da sessão ao fechar, cancelamento da confirmação e
remoção de cookies, localStorage, sessionStorage, IndexedDB e CacheStorage.
Os testes usam armazenamento temporário. A transação SQLite também foi testada
com falha simulada para verificar rollback. O aceite manual em sites reais ainda
deve conferir login/logout, downloads e permissões nas janelas privadas.

Próximas entregas: cenários de benchmark com abas inativas e navegação longa;
empacotamento Flatpak e CI para o alpha.

| Etapa | Entrega | Critério de aceite |
| --- | --- | --- |
| 1 | Rust, GTK4, WebKitGTK, janela HTTPS e README | Build validado; example.org renderiza; fechar encerra a aplicação |
| 2 | Barra de endereço, voltar, avançar, recarregar e atalhos | Distinguir endereço de pesquisa; validar URLs; restringir esquemas; refletir estado de navegação |
| 3 | Modelo de abas separado da UI e tratamento de pop-ups | Abrir/fechar dez abas; cada aba viva possui uma única WebView |
| 4 | Histórico, preferências e sessões em SQLite | Recuperar sessão após reinício; separar perfil e cache |
| 5 | Métricas locais e benchmark | Medir a árvore de processos; cinco repetições por cenário; publicar mediana e dispersão |
| 6 | Descarte de abas elegíveis | Reduzir memória sem descartar áudio, formulários, downloads ou tarefas ativas |
| 7 | Downloads, permissões e apresentação de erros TLS | Testar câmera, microfone, localização, notificações, clipboard e certificados inválidos |
| 8 | Flatpak, CI e alpha interno | Instalar em outra máquina; validar runtime suportado, portais e permissões mínimas |

## Validação de desempenho

Comparar Kairo, Firefox e Chromium no mesmo hardware, sistema e conjunto de URLs,
usando perfis limpos e registrando versões. Medir PSS preferencialmente, RSS,
CPU em repouso, tempo até janela utilizável, carregamento e falhas para todos
os processos do navegador.

Cenários: janela vazia, um site simples, cinco sites, dez abas com nove inativas
e sessenta minutos de navegação. Nenhuma meta pública de RAM será fixada antes
dessas medições.

## Condições para o alpha

Manter o sandbox do motor e a validação TLS. Definir permissões explícitas,
exclusão de dados, política de navegação externa e destinos de download via
portal no Flatpak. Testar autenticação, upload, mídia, pop-ups e target=_blank.
Definir licença, identidade do aplicativo, política de privacidade, canal de
bugs e rotina de atualização de segurança antes de distribuir a terceiros.

Sincronização, extensões de terceiros, motor próprio, bloqueador complexo,
Windows e IA embutida ficam fora do MVP.
