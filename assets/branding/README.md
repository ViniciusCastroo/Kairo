# Marca do Kairo

`kairo-black.svg` e `kairo-white.svg` são cópias dos arquivos `1.svg` e `2.svg`
fornecidos pelo usuário. Os desenhos originais foram preservados.

As variantes `*-compact.svg` alteram apenas o viewport e o tamanho declarados
para remover as margens largas do documento na interface. A tela inicial usa a
variante branca, embarcada como imagem de dados, sem buscar arquivos externos.

O ícone em `../icons/scalable/apps/dev.example.Kairo.svg` usa os mesmos caminhos
da versão branca sobre um fundo escuro com cantos arredondados. Ele é compilado
em GResource por `build.rs` e disponibilizado ao GTK. A associação definitiva
com o lançador do sistema dependerá do arquivo desktop/Flatpak da etapa de
distribuição. A versão preta está disponível para um futuro tema claro.
