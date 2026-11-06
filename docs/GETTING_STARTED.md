# Primeiros passos

Um projeto segue a ordem da paleta **Elétrica**. Lançar pontos, circuitos e eletrodutos não calcula nada:
os cabos e disjuntores só mudam quando você manda dimensionar.

A paleta é a aba **Instalações** das **Ferramentas**, à esquerda, ao lado de **Desenho** e **Modelagem**. No topo
dela, o seletor **Elétrica | Telecom** troca as ferramentas: baixa tensão de um lado, cabeamento estruturado do outro.
As seções recolhem pela seta. Nada fica aberto sem ser pedido. As janelas fecham no X do cabeçalho ou com Esc, e Esc
fecha a última aberta. Com um comando em andamento, Esc cancela o comando.

## 1. Prepare o projeto

Abra sua planta em DXF ou comece um desenho novo. Em **Configurações...**, ajuste:

- **Critérios elétricos:** temperatura e limites de queda.
- **Alturas:** pé-direito e alturas baixa, média e alta do projeto.
- **Eletrodutos:** perfis com tipo de linha, cor, caminhamento e tabela de diâmetros.
- **Cabos:** os catálogos de cabos elétricos e de telecom.
- **Demanda / Concessionárias:** a tabela da sua concessionária e o tipo de edificação. Se ela não estiver na lista,
  use **+ Criar / Editar Tabela de Concessionária**.

As camadas e os blocos base entram sozinhos na primeira ferramenta que você usar.

## 2. Insira os pontos

Clique em **Inserir pontos**. Na mini-barra sobre o desenho, **Pesquisar ponto...** filtra os blocos pelo nome ou pela
legenda enquanto você digita, junto com a categoria; a lista mostra a potência e a altura padrão de cada um. Escolha
categoria, bloco, potência, altura e
quadro pai. Cada clique no desenho insere um ponto, que alinha à parede mais próxima. **Tab** gira e
**Enter** termina.

Comece pelo quadro: categoria **Quadros/Racks**, bloco **Quadro QDG**.

Cada quadro tem o seu **Sistema**, **Tensão Nominal**, **Tipo de Cabo Padrão** e **Alimentado por**. Selecione o
quadro e abra o **Inspetor de atributos...**, ou use o topo do **Quadro de cargas...**.

Um ponto na altura da legenda não escreve nada na planta. Em outra altura, aparece como `h=1,45m`.

Para um símbolo próprio, use **Blocos e famílias...** e **+ Criar novo bloco elétrico**. A família guarda a
**Classe de Demanda** e o **Fator de Demanda**. Para mudar valores de um bloco já inserido, selecione-o e abra o
**Inspetor de atributos...**: um ponto pode ter classe e fator próprios, que valem sobre os da família.

Na aba **Telecom** da paleta, **Inserir ponto de dados** e **Inserir rack** já começam com o bloco certo. Ligue os pontos ao
rack com eletrodutos (um cabo de telecom passa por outros pontos de dados e por caixas de passagem, nunca por uma caixa de
tomada) e use a seção **Cabeamento (NBR 14565)**:

- **Numerar e vincular ao rack:** liga cada ponto ao rack (com um rack só, automático) e numera `D-01`, `CFTV-01`, `AP-01`,
  `FO-01`, com o cabo (`UTP Cat 6` por padrão) e a quantidade de cabos.
- **Dimensionar telecom...:** um cabo inteiro de cada ponto até o rack, o comprimento 3D com as folgas e o aviso acima de 90 m,
  e o diâmetro de cada eletroduto com 40 % de ocupação. O resultado abre no **Quadro VDI / Rack**, embaixo do desenho.
- **Tags de cabeamento** e **Tabela do rack** desenham o resultado; **Atualizar tags e tabelas** refaz tudo.

## 3. Crie os circuitos e os comandos

- **Criar circuito:** clique no quadro, informe o número e clique nos pontos. Enter termina. A mini-barra edita
  descrição, esquema e tensão.
- **Comandos e luminárias:** clique no interruptor e nas luminárias dele. Enter termina. Para paralelos (three-way) e
  intermediários (four-way), clique também nos outros interruptores do mesmo comando, na ordem do caminho: o primeiro recebe
  a fase, os intermediários ficam no meio e do último sai o retorno às luminárias. A fiação passa dois retornos (`a'` e
  `a"`) entre eles e um (`a`) até as luminárias.

Ligue **Inspecionar projeto** para ver os halos: vermelho falta circuito ou comando, laranja sem caminho até o
quadro, amarelo acima do limite e verde válido.

Duas tomadas no mesmo ponto, ou duas teclas: insira um módulo sobre o outro. Cada um tem o seu circuito e o seu
comando; a planta mostra uma caixa 4x2 só, com os símbolos empilhados, e o eletroduto da caixa atende todos.

## 4. Lance os eletrodutos

Clique em **Inserir eletroduto** e depois nos blocos, em sequência. Na mini-barra, escolha o perfil, que traz o tipo de
linha, a cor, o caminhamento e os diâmetros, e o traçado: **Reto**, **Ângulo 45°** ou **Arco**. Enter termina.

Para mudar a curvatura ou o lado do arco, digite `Flecha` ou `Inverter` na linha de comando.

- Um clique no vazio vira um desvio do trajeto, para contornar pilares.
- Um clique no meio de um eletroduto insere uma caixa de passagem.
- Um clique num bloco que está no caminho de um eletroduto divide o trecho nele: o próprio bloco já é a caixa.
- Cruzamentos sem bloco **não conectam**: cada eletroduto liga exatamente dois blocos.
- Cada trecho é uma linha, um arco ou uma polilinha a 45°. Um trecho novo nunca mexe nos que já existem.
- A cor vem do material e o tipo de linha vem do caminhamento: contínua no teto, tracejada ou pontilhada na parede, traço e ponto no piso.

## 5. Dimensione

Abra **Dimensionar projeto...** e escolha o quadro:

1. Em cada circuito, escolha o esquema (`F+N`, `F+N+T`, `2F+T`, `2F+N+T`, `3F+T`, `3F+N+T`) e o tipo de cabo.
2. Monte os grupos de DR: marque os circuitos à esquerda, crie o grupo com **+ Novo Grupo DR** e use
   **Mover selecionados**. Pode haver quantos grupos o projeto pedir.
3. Marque, se quiser, **Compartilhar condutor de proteção (Terra / PE)** e **Gerar / Atualizar Diagrama Unifilar**.
4. Clique em **Calcular Dimensionamento**. Os cabos, os disjuntores e as curvas aparecem por circuito, sem gravar
   nada, e você pode trocar cada valor.
5. Clique em **Aplicar ao Quadro**.

**Quadro de cargas...** abre um painel embaixo do desenho, entre as Ferramentas e as Camadas, sem cobrir nada. A borda
de cima muda a altura; o X do canto ou Esc fecha, e o desenho volta a ocupar a tela toda. Nele:

- A tabela mostra fases, grupo DR, cabo e disjuntor calculados. Um circuito que mudou depois do cálculo aparece
  como **Desatualizado**.
- O menu **...** de cada linha adiciona e remove pontos, localiza o circuito, abre a ficha de engenharia (tipo de
  cabo, FCT, FCA, correntes, queda de tensão e as travas de seção e disjuntor) e exclui o circuito.
- **Configurações e dados do quadro** abre sistema, tensão, cabo padrão, alimentação, montagem, módulos, reservas e
  observações.

Depois de mudar algo, calcule e aplique de novo.

## 6. Documente

- **Tags de fiação:** a mini-barra escolhe o modo. **Direto no eletroduto:** um clique no trecho, e as marcas ficam sobre ele.
  **Com linha de chamada:** clique no eletroduto e depois onde a base da tag começa; a chamada liga os dois pontos no ângulo que
  você der. Enter no começo põe uma tag no meio de todos os eletrodutos. Mova a tag para onde quiser; ela fica lá.
- **Tabela do quadro:** desenha o quadro de cargas, todo dentro de uma moldura. Montagem, módulos, reservas e observações ficam
  no Inspetor de atributos do quadro.
- **Diagrama unifilar:** desenha o diagrama do quadro num ponto da planta, com os grupos de DR e as reservas.
- **Legenda de símbolos:** desenha o símbolo e o **Nome na Legenda** de cada tipo de ponto usado.
- **Atualizar tags e tabelas:** refaz o conteúdo das tags, tabelas e diagramas sem mudar as posições. Aplicar ao
  quadro também atualiza.
- **Lista de materiais...:** cabos, eletrodutos, caixas, pontos e disjuntores. A **Folga de Cabos** fica no topo da janela.

## Antes de usar em projeto real

Os resultados são apoio ao projeto e precisam da conferência de um profissional habilitado. Confira também:

- os diâmetros internos de eletroduto, que são valores típicos de catálogo;
- os diâmetros externos de cabo, também típicos;
- as sugestões de DR e DPS e a curva sugerida para cada disjuntor;
- a tabela de demanda da sua concessionária, na revisão em vigor.
