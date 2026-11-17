# Roboto

Fonte obtida do Google Fonts. O WOFF2 original está preservado nesta pasta;
a versão TTF foi gerada com fontTools 4.68.0, sem alterar desenhos ou cobertura.
Origem, conversão, tamanhos e SHA-256 estão em `source.json`; a licença SIL Open Font License 1.1 está em
`OFL.txt` e acompanha os recursos da aplicação.

Foi incluída somente a variante normal variável, pesos 100–900 e largura 100%,
do subconjunto latino (TTF de 80.500 bytes, cerca de 78,6 KiB). Os caracteres usados na
interface em português estão nesse subconjunto. Outros caracteres usam as
fontes alternativas do sistema. A variante itálica não foi embarcada.

O mesmo arquivo serve às duas interfaces: o GTK usa o registro de fontes de
aplicação do Fontconfig, a partir de um descritor Linux em memória; a página
inicial recebe uma URL `data:` por `@font-face`. Não há instalação global,
arquivo extraído para o perfil, CDN ou download de fonte ao executar o Kairo.
A cópia WOFF2 de origem não é embarcada no binário. As ferramentas de conversão
não são necessárias para compilar ou executar o Kairo.
Fontconfig já faz parte da pilha GTK/Pango no Linux. Não foram adicionados crates.

O tamanho do arquivo não representa a memória total usada pela fonte ou pelo
navegador. A fonte padrão dos sites visitados não é alterada.
