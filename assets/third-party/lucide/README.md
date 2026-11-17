# Lucide

Origem: https://github.com/lucide-icons/lucide

Commit: `55782beff3a0133df79c417eb68f96e8f19e2404`.

Os 13 SVGs de `icons/` estão preservados em `originals/` (4.170 bytes).
As versões em `assets/icons/scalable/actions/kairo-<nome>-symbolic.svg`
somam 7.950 bytes: cada forma recebeu as classes GTK `transparent-fill` e
`foreground-stroke`, largura 2 e extremidades/junções arredondadas explícitas.
O estilo `fill:none!important` preserva o contorno quando o ícone é renderizado
por librsvg. A geometria original permanece intacta.
`source.json` registra o SHA-256 de cada arquivo original. `LICENSE` preserva
os avisos ISC do Lucide e MIT dos desenhos derivados de Feather, e também é
incluído no GResource da aplicação.

O GTK carrega os desenhos como ícones simbólicos, ajustando cor e tamanho ao
controle. A tela inicial inclui os mesmos SVGs de busca, seta e edição no HTML,
marcados como decorativos; os controles têm nomes acessíveis. Não são usados
React, npm, CDN ou requisições externas. Os controles nativos da janela seguem
o GTK.

O tamanho desses arquivos não representa o consumo de RAM do navegador.
