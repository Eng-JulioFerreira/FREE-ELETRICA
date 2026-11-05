# Primeiros passos

Um projeto segue a ordem da paleta **Elétrica**. Lançar pontos, circuitos e eletrodutos não calcula nada:
os cabos e disjuntores só mudam quando você manda dimensionar.

A paleta tem as abas **Elétrica** e **Telecom**, com seções que recolhem pela seta. Nada fica aberto sem ser
pedido. As janelas fecham no X do cabeçalho ou com Esc, e Esc fecha a última aberta. Com um
comando em andamento, Esc cancela o comando.

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

Clique em **Inserir pontos**. Na mini-barra sobre o desenho, escolha categoria, bloco, potência, altura e
quadro pai. Cada clique no desenho insere um ponto, que alinha à parede mais próxima. **Tab** gira e
**Enter** termina.

Comece pelo quadro: categoria **Quadros/Racks**, bloco **Quadro QDG**.

Cada quadro tem o seu **Sistema**, **Tensão Nominal**, **Tipo de Cabo Padrão** e **Alimentado por**. Selecione o
quadro e abra o **Inspetor de atributos...**, ou use o topo do **Quadro de cargas...**.

Um ponto na altura da legenda não escreve nada na planta. Em outra altura, aparece como `h=1,45m`.

Para um símbolo próprio, use **Blocos e famílias...** e **+ Criar novo bloco elétrico**. A família guarda a
**Classe de Demanda** e o **Fator de Demanda**. Para mudar valores de um bloco já inserido, selecione-o e abra o
**Inspetor de atributos...**: um ponto pode ter classe e fator próprios, que valem sobre os da família.

Na aba **Telecom** da paleta, **Inserir ponto de dados** e **Inserir rack** já começam com o bloco certo.

## 3. Crie os circuitos e os comandos

- **Criar circuito:** clique no quadro, informe o número e clique nos pontos. Enter termina. A mini-barra edita
  descrição, esquema e tensão.
- **Comandos e luminárias:** clique no interruptor e nas luminárias dele. Enter termina.

Ligue **Inspecionar projeto** para ver os halos: vermelho falta circuito ou comando, laranja sem caminho até o
quadro, amarelo acima do limite e verde válido.

## 4. Lance os eletrodutos

Clique em **Inserir eletroduto** e depois nos blocos, em sequência. Na mini-barra, escolha o perfil, que traz o tipo de
linha, a cor, o caminhamento e os diâmetros, e o traçado: **Reto**, **Ângulo 45°** ou **Arco**. Enter termina.

Para mudar a curvatura ou o lado do arco, digite `Flecha` ou `Inverter` na linha de comando.

- Um clique no vazio vira um desvio do trajeto, para contornar pilares.
- Um clique no meio de um eletroduto insere uma caixa de passagem.
- Um clique num bloco que está no caminho de um eletroduto divide o trecho nele: o próprio bloco já é a caixa.
- Cruzamentos sem bloco **não conectam**: cada eletroduto liga exatamente dois blocos.
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

Em **Quadro de cargas...**:

- A aba **Resumo** mostra fases, grupo DR, cabo e disjuntor calculados. Um circuito que mudou depois do cálculo
  aparece como **Desatualizado**.
- A aba **Ficha de engenharia** mostra cada circuito: tipo de cabo, FCT, FCA, correntes, queda de tensão e as
  travas de seção e disjuntor.

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
