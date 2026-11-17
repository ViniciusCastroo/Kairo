# Kairo

Navegador Linux em Rust com interface GTK4 e renderização WebKitGTK 6.0.
A prioridade é medir e reduzir o consumo de memória sem enfraquecer as proteções do motor.

## Estado do projeto

Protótipo das quatro primeiras etapas do guia de arquitetura de 13/11/2026,
com instrumentação local de desempenho e interface escura inspirada na referência
visual fornecida pelo usuário: abas no topo, endereço arredondado e barra compacta.
O código cria uma janela de 1100 × 740 e restaura as abas salvas. Sem sessão,
abre a página inicial configurada, por padrão a tela local do Kairo.
Uma nova ativação apresenta a janela existente.
A barra de endereço permite navegar e pesquisar; há botões para voltar,
avançar, recarregar e parar, além de indicador de carregamento e título da página.
Há abas com páginas e históricos independentes, título e botão de fechamento.
Links que pedem outra janela abrem em uma aba, quando acionados pelo usuário.
Pop-ups sem interação direta são recusados.
Histórico, favoritos, preferências e sessão são persistidos em SQLite. Há
gerenciador de downloads, confirmação de permissões e aviso explícito de erros
TLS. Há janelas privadas e limpeza dos dados de navegação. Descarte de abas
e comparação de memória em release ainda estão pendentes.
As versões SVG da marca fornecida pelo usuário estão preservadas em
`assets/branding`; logo e ícone são embarcados na aplicação.
Os controles usam 13 SVGs do [Lucide](https://lucide.dev), incluídos localmente
como ícones simbólicos do GTK e SVGs na tela inicial. Os SVGs adaptados ao GTK somam 7.950
bytes; não há React, npm, CDN ou dependência adicional para ícones. Isso descreve
o tamanho dos arquivos, não uma medida de RAM. Origem, versão e licença estão em
`assets/third-party/lucide`.
Roboto é a fonte principal da interface GTK e da tela inicial. Um único TTF
latino variável de 80.500 bytes (cerca de 78,6 KiB) está embarcado, sem importar
CSS do Google ou instalar a fonte no sistema. Pesos 100–900 estão disponíveis;
caracteres fora do subconjunto usam fontes alternativas. A tipografia dos sites
visitados segue as escolhas de cada site. Licença e origem da fonte ficam em
`assets/third-party/roboto`. O carregamento nativo usa Fontconfig, já integrante
da pilha GTK/Pango no Linux; não há novos crates.

As dependências de desenvolvimento estão instaladas e o `Cargo.lock` foi gerado.
Compilação, formatação e Clippy validados. O aplicativo foi iniciado na sessão
gráfica local, sem erros no terminal. O usuário confirmou a exibição do conteúdo
de example.org na janela do Kairo. O usuário também confirmou navegação no site
Rust, voltar/avançar, recarregamento e pesquisa.
O usuário confirmou também o funcionamento da versão com abas. A confirmação
geral não substitui os casos específicos de pop-ups e confirmação de saída
descritos no aceite manual abaixo.
Há ensaios GTK de integração; o benchmark comparativo release e qualquer
promessa de consumo de RAM continuam pendentes.

## Preparar o ambiente

No Ubuntu 26.04, os pacotes abaixo estão disponíveis nos repositórios consultados:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config rustc cargo rustfmt rust-clippy libgtk-4-dev libwebkitgtk-6.0-dev libsqlite3-dev libglib2.0-dev-bin
```

Os pacotes Rust da distribuição permitem iniciar sem um segundo gerenciador de
toolchains. Se já usa [rustup](https://rustup.rs), mantenha essa instalação e
instale os componentes `rustfmt` e `clippy` com `rustup component add rustfmt clippy`;
nesse caso, omita os quatro pacotes Rust do comando apt.

Verifique as versões efetivamente instaladas:

```bash
bash scripts/check-environment.sh
```

| Componente | Versão instalada e verificada em 13/11/2026 |
| --- | --- |
| Sistema | Ubuntu 26.04.1 LTS, amd64 |
| Rust / Cargo | 1.93.1 |
| GTK4 (desenvolvimento) | 4.22.4 |
| WebKitGTK 6.0 (desenvolvimento) | 2.52.6 |
| SQLite (desenvolvimento) | 3.46.1 |

Use GTK4 e WebKitGTK **6.0**. Bibliotecas WebKitGTK 4.0/4.1 para GTK3 não
substituem essa dependência. Se `pkg-config` não encontrar `webkitgtk-6.0`,
corrija a instalação antes de prosseguir. O manifesto habilita a feature
`v4_10` do GTK4, necessária para os tipos de acessibilidade usados por webkit6;
a biblioteca nativa GTK deve ser 4.10 ou superior.
Os bindings do WebKit habilitam `v2_46` para as APIs de permissões; a biblioteca
nativa WebKitGTK deve ser 2.46 ou superior. Os recursos SVG são compilados por
`glib-compile-resources`, sem depender da pasta de downloads em execução.

## Compilar e executar

```bash
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked
```

Execute em uma sessão gráfica Linux com Wayland ou X11 e acesso HTTPS.
O primeiro build baixa as crates e pode demorar. O `Cargo.lock` gerado pelo
Cargo está incluído no projeto e deve ser versionado junto ao código.
`cargo test` cobre a interpretação de endereços, a codificação das pesquisas,
o bloqueio de esquemas, a rejeição de URLs inválidas ou com credenciais e
o modelo de abertura, seleção e fechamento de abas.
Também verifica persistência ao reabrir o banco, preferências, limpeza do
histórico, validação de URLs do perfil, versão do esquema e rollback da sessão
quando uma gravação falha.

Validação local em 13/11/2026: `cargo fmt --check`, `cargo check --locked --offline`,
`cargo clippy --locked --offline --all-targets -- -D warnings`,
`cargo test --locked --offline` (trinta testes de lógica) e `cargo build --locked --offline`
concluídos com sucesso. A primeira execução do bootstrap com `cargo run --locked --offline`
encerrou com código 0 e sem erros no terminal. A renderização visual de example.org
foi confirmada pelo usuário. A segunda ativação com a janela ainda aberta e o
fechamento manual precisam ser confirmados.

O teste gráfico adicional passou na sessão local: abriu dez WebViews, aguardou
o carregamento, verificou alternância e isolamento das mensagens de erro,
fechou uma aba via WebKit e removeu as demais. Não acessa sites externos.
Também usou um servidor HTTP em localhost e banco em memória para verificar
histórico, restauração de duas abas na ordem e seleção originais, limpeza de
histórico sem perder a sessão e desativação da restauração.
Ele usa um executável próprio porque GTK/WebKit precisam inicializar na thread
principal; sem a opção abaixo, o `cargo test` padrão apenas informa como executá-lo.
O teste GTK completo também exige Python 3 e OpenSSL para o servidor TLS local.
Verificou downloads de arquivos reais em localhost, destino com espaços e
acentos, recusa de substituição sem autorização, cancelamento, transferência
truncada, proteção ao fechar durante download, recusa de localização,
cancelamento por navegação, pedidos de abas em segundo plano e certificado
autoassinado rejeitado. Os testes escolhem destinos temporários automaticamente;
o seletor de arquivos/portal interativo ainda precisa de aceite manual.

```bash
cargo test --locked --offline --test gtk-tabs -- --run-gtk
```

### Aceite manual da primeira entrega

- A janela Kairo abre e renderiza a página HTTPS de example.org.
- Uma segunda execução apresenta a janela existente.
- Fechar a janela encerra a aplicação, sem falhas recorrentes no terminal.
- Registrar aqui as versões instaladas e o resultado antes de concluir a etapa.

O identificador `dev.example.Kairo` é provisório para desenvolvimento.
O protótipo mantém as configurações padrão de sandbox e TLS do WebKitGTK.

## Estrutura

```text
src/main.rs       entrada e código de saída do processo
src/app.rs        ciclo de vida do GTK Application
src/browser/      validação de URLs e modelo de abas independente de GTK
src/storage/      banco SQLite, histórico, preferências e sessão
src/ui/mod.rs     janela, barra, ações e uma WebView por aba viva
src/ui/data.rs    salvamento de sessão e telas de histórico e preferências
src/ui/tests.rs   verificação gráfica da integração entre abas e controles
tests/           executor GTK e página local para testes manuais
scripts/         diagnóstico das dependências locais
docs/            plano incremental e critérios de aceite
```

As próximas etapas de otimização e distribuição estão no [roteiro](docs/roadmap.md).

## Usar a navegação

- `Ctrl+L`: selecionar o endereço; Enter: abrir ou pesquisar.
- `Alt+←` / `Alt+→`: voltar e avançar.
- `Ctrl+R` ou `F5`: recarregar; `Esc`: parar o carregamento.
- `Alt+Home`: abrir a página inicial configurada.
- `Ctrl+H` ou menu → Histórico: consultar as 200 páginas distintas mais recentes,
  abrir uma delas em nova aba ou limpar o histórico.
- `Ctrl+J` ou menu → Downloads: acompanhar progresso, cancelar transferências,
  mostrar a pasta de um arquivo concluído e limpar os registros terminados.
- `Ctrl+Shift+N` ou menu → Nova janela privada: abrir uma janela com dados
  temporários, isolada do perfil normal e de outras janelas privadas.
- Menu → Limpar dados de navegação: revisar a confirmação e fechar as abas
  normais para apagar histórico, sessão salva e dados dos sites.
- `Ctrl+D` ou estrela na barra: salvar a página atual como favorito. Se ela já
  estiver salva, o editor abre o favorito existente.
- `Ctrl+Shift+O`, menu → Favoritos ou **Gerenciar favoritos** na tela inicial:
  adicionar, editar nome/endereço, abrir, remover e ordenar com as setas.
  Os 12 primeiros favoritos aparecem na tela inicial, com prioridade sobre
  os sites recentes. Ao remover todos, os atalhos recentes voltam a aparecer.
- No histórico, use **Voltar à navegação**, o botão de fechar ou `Esc` para sair.
  As preferências também podem ser fechadas com **Cancelar** ou `Esc`.
- Menu → Preferências: escolher a página inicial e ativar/desativar a
  restauração de abas. Use o botão Salvar para persistir as escolhas.
- Em Preferências → Buscador padrão, escolha **DuckDuckGo** ou **Google** e
  salve. A escolha vale para a barra e para a busca central, é mantida após
  reiniciar e atualiza telas iniciais já abertas. DuckDuckGo é o padrão inicial.
- `Ctrl+T` abre a tela inicial local, com relógio, busca no provedor escolhido e até seis
  atalhos de sites recentes, um por domínio. Sem histórico, aparecem acessos
  iniciais ao buscador escolhido, Wikipedia e GitHub.
- Menu → Tela inicial do Kairo abre essa tela na aba atual. Para usá-la também
  ao iniciar sem sessão, vá a Preferências → Usar tela inicial do Kairo → Salvar.
  Páginas iniciais personalizadas e abas já salvas são preservadas.
- Menu → Desempenho: medir memória e CPU do conjunto de processos, consultar
  carregamentos/falhas e salvar um relatório local. Os detalhes, o coletor CSV
  e o protocolo de cinco execuções estão em [benchmarks](benchmarks/README.md).
- `Ctrl+T` ou botão `+`: abrir a tela inicial, com foco na barra de endereço.
- `Ctrl+W` ou botão de fechar na aba: fechar a aba selecionada. Fechar a última
  encerra a janela. O fechamento de uma aba consulta o WebKit para respeitar
  páginas com confirmação de saída.
- `Ctrl+Tab` / `Ctrl+Shift+Tab`: próxima/aba anterior; também é possível usar
  `Ctrl+PageDown` / `Ctrl+PageUp` ou clicar na aba.
- Um domínio sem protocolo, como `example.org`, usa HTTPS. Para servidores
  locais sem TLS, digite explicitamente `http://localhost:3000`.
- Textos como `rust gtk tutorial` são enviados ao buscador escolhido. A pesquisa só
  é enviada ao pressionar Enter; não há sugestões remotas enquanto digita.
- A barra aceita somente HTTP/HTTPS e rejeita credenciais na URL. Entradas
  inválidas mostram uma mensagem sem substituir a página atual.

### Aceite manual da navegação

Abra dois endereços na mesma janela e confira voltar/avançar, inclusive seus
estados desabilitados. Teste os atalhos, a pesquisa com acentos, o título e
a atualização da barra após redirecionamentos. Digite `javascript:alert(1)`
e confirme que a barra bloqueia a entrada. Teste erro de conexão e certificado
inválido, sem contornar a validação TLS. O usuário confirmou abertura do site
Rust, voltar/avançar, recarregamento e pesquisa; os demais casos seguem pendentes.

### Aceite manual de abas e pop-ups

Reinicie o aplicativo para carregar a nova versão. Abra dez abas, navegue em
duas delas e confirme que voltar/avançar afetam apenas a selecionada. Feche uma
aba em segundo plano e a última aba. Para testar pop-ups sem depender de sites:

```bash
python3 -m http.server 8765 --bind 127.0.0.1 --directory tests/fixtures
```

No Kairo, abra `http://127.0.0.1:8765/tabs.html`. O link `target=_blank` e o
botão de pop-up devem criar novas abas. O pop-up automático após um segundo
deve continuar bloqueado. Na aba aberta por script, teste o botão de fechar.
Marque a confirmação de saída, digite no campo e feche a aba para verificar
o diálogo do motor. Esses casos de pop-up e confirmação ainda dependem de
validação manual. Encerre o servidor com `Ctrl+C` após o teste.

## Downloads e permissões

Ao baixar um arquivo, o Kairo abre o seletor nativo de destino. O nome sugerido
pelo servidor é reduzido a um nome de arquivo, sem diretórios ou caracteres de
controle. A transferência espera a seleção e é cancelada se o seletor for
fechado. A substituição de um arquivo existente depende da escolha pelo seletor.
O aplicativo não abre nem executa automaticamente os arquivos recebidos.

O progresso e os estados de erro/cancelamento ficam disponíveis nesta sessão.
Downloads incompletos com tamanho conhecido não são marcados como concluídos;
respostas comprimidas ou sem tamanho usam os dados disponíveis do motor.
Arquivos parciais podem permanecer no destino após falha/cancelamento. Ainda
não há retomada de transferências nem histórico de downloads após reiniciar.
Fechar a janela com downloads ativos permite continuar ou cancelar e sair;
fechar a última aba mantém uma nova aba aberta enquanto houver transferência.

Câmera, microfone, localização, notificações e área de transferência passam por
um diálogo de confirmação. A página selecionada precisa usar HTTPS ou HTTP em
localhost/loopback. O diálogo mostra a origem da página principal e informa que
o pedido pode vir de conteúdo incorporado. Não há concessões salvas no perfil;
pedidos desconhecidos, de abas em segundo plano ou de páginas inseguras são
negados. Navegar, fechar a aba ou dispensar o diálogo nega o pedido pendente.
Notificações básicas são encaminhadas ao desktop; ações de clique e recursos
como captura de tela, DRM e bloqueio do ponteiro ainda não têm fluxo próprio.

Erros de certificado bloqueiam a conexão e apresentam mensagem explícita.
Não existe botão de exceção e não é alterado o repositório de certificados.

### Aceite manual desta entrega

Teste um download real com o seletor nativo, escolha outro diretório e confira
progresso, cancelamento e fechamento da janela durante uma transferência.
Teste câmera/microfone, notificações do desktop e clipboard no seu hardware,
incluindo recusa e troca de página antes de responder. A entrega física desses
recursos e o portal no futuro Flatpak ainda dependem dessa validação.

## Perfil e restauração

### Privacidade

Uma janela privada usa `NetworkSession::new_ephemeral()` do WebKit, sem abrir
o banco do perfil. Não lê nem grava histórico, favoritos ou sessão salva. Usa
o buscador escolhido na janela de origem e inicia na tela local do Kairo.
As abas e pop-ups relacionados compartilham a sessão temporária da janela;
outra janela privada começa com uma sessão independente. Ao fechar a janela,
essa sessão é liberada. Histórico, preferências e edição de favoritos ficam
desativados nesse modo. Os arquivos que você baixar continuam no destino escolhido.
Esse modo não oculta o endereço IP dos sites ou do provedor de internet.

**Limpar dados de navegação** exige uma confirmação na interface. A operação
fecha as abas normais e as telas auxiliares dessa janela, apaga histórico e
sessão salva em uma transação SQLite e solicita ao WebKit a remoção de todos
os tipos de dados de sites, de todo o período: cookies, caches, armazenamento
local/de sessão, IndexedDB e registros de service workers, entre outros.
Favoritos, preferências e arquivos baixados são preservados. Janelas privadas
não são alteradas. Downloads ativos precisam ser concluídos ou cancelados antes.
Durante a limpeza a janela fica indisponível; ao terminar abre uma nova tela
local e informa o resultado. Se o WebKit falhar, a mensagem informa que a
remoção foi parcial e permite tentar novamente. Não é uma ferramenta de
apagamento forense do disco.

O teste GTK usa perfis e diretórios temporários, nunca os cookies do usuário.
Ele verifica isolamento, compartilhamento entre abas privadas, liberação da
sessão, cancelamento da confirmação e remoção de cookies, localStorage,
sessionStorage, IndexedDB e CacheStorage. O teste SQLite cobre preservação dos
favoritos/preferências e rollback em caso de falha. Para executar só esse fluxo:

```bash
cargo test --locked --offline --test gtk-tabs -- --run-gtk --only-privacy
```

A arquitetura segue a [migração oficial para WebKitGTK 6.0](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/gtk/migrating-to-webkitgtk-6.0.md):
sessões de rede compartilhadas entre WebViews e sessões efêmeras para dados
temporários. Não foram adicionadas dependências.

### Dados persistentes

O banco fica em `$XDG_DATA_HOME/kairo/kairo.sqlite3`; sem configuração XDG,
normalmente `~/.local/share/kairo/kairo.sqlite3`. Novos diretórios e arquivos
do perfil são criados com acesso restrito ao usuário. O banco usa esquema
versionado e grava sessões em transações. Falhas de leitura ou escrita aparecem
na janela; um perfil que não pode ser lido não é sobrescrito com valores padrão.
O esquema 2 acrescenta a preferência de busca aos perfis antigos, preservando
página inicial, restauração, histórico e abas. Perfis existentes continuam com
DuckDuckGo até a escolha explícita de outro buscador.
O esquema 3 acrescenta os favoritos, preservando os dados anteriores. Eles são
salvos separadamente do histórico; limpar o histórico não remove favoritos.

A restauração vem ativada. URLs, ordem e aba selecionada são salvas após
250 ms sem alterações e ao fechar a janela. Feche pelo botão da **janela** para
manter as abas na sessão. Fechar abas individualmente remove-as da sessão;
fechar a última faz a próxima execução abrir a página inicial.

São restauradas URLs HTTP/HTTPS sem credenciais, a tela inicial do Kairo e abas
vazias. Outras páginas internas ou esquemas incompatíveis viram abas vazias. Esta
etapa não restaura formulários, rolagem nem a pilha voltar/avançar de cada aba.

O histórico registra páginas HTTP/HTTPS cujo carregamento foi concluído sem
falha, com URL completa, título e horário da visita mais recente. Não registra
abas vazias nem a própria tela inicial; visitas repetidas atualizam a mesma entrada.
Limpar histórico
remove essas entradas, preservando sessão, preferências e dados do motor.
Desativar a restauração apaga a sessão salva e mantém o histórico.

Os testes do banco usam perfis temporários, e o teste GTK usa banco em memória.
As operações SQLite ainda são síncronas; métricas, retenção do histórico e
eventual trabalho em segundo plano serão avaliados na etapa de desempenho.

A tela inicial é servida pelo próprio aplicativo em `kairo://start/`, com
HTML, CSS e JavaScript embarcados. Não baixa imagens, fontes ou relógios da rede;
somente uma pesquisa enviada ou um atalho acionado inicia navegação externa.
Os atalhos são reconstruídos a cada carregamento a partir do histórico local,
com títulos escapados como texto. A tela usa política de conteúdo restrita e
um esquema isolado, sem acesso privilegiado ao banco por JavaScript.
O botão de gerenciar favoritos apenas solicita a abertura do gerenciador nativo;
o aplicativo aceita essa solicitação somente quando o documento atual é a tela
inicial. Páginas externas não podem usá-la para abrir controles internos.

### Aceite manual da persistência

Reinicie para carregar a nova versão, abra duas páginas em abas diferentes,
selecione a primeira e feche a janela. Execute `cargo run --locked` novamente:
as duas abas e a seleção devem voltar. Confira `Ctrl+H`, abra um item do histórico
e teste sua limpeza. Em Preferências, altere a página inicial, salve e teste
`Alt+Home`. Desative a restauração, reinicie e confira que só a página inicial
abre. Esses casos ainda aguardam confirmação manual do usuário.

## Referências técnicas

- [Configuração de projetos GTK4 em Rust](https://gtk-rs.org/gtk4-rs/stable/latest/book/project_setup.html)
- [Manifesto webkit6 0.6.1](https://docs.rs/crate/webkit6/0.6.1/source/Cargo.toml): usa GTK4 0.11, compatível com a série escolhida neste projeto.
- [API WebView](https://docs.rs/webkit6/0.6.1/webkit6/struct.WebView.html)

Distribuição pública, Flatpak, licença e app ID definitivo serão tratados
antes do alpha. A escolha do motor será reavaliada com medições reproduzíveis.
