# Primeiros passos

Um projeto segue a ordem da paleta **Elétrica**. Lançar pontos, circuitos e eletrodutos não calcula nada:
os cabos e disjuntores só mudam quando você manda dimensionar.

Nada fica aberto sem ser pedido. As janelas fecham no X ou com Esc, e Esc fecha a última aberta. Com um
comando em andamento, Esc cancela o comando.

## 1. Prepare o projeto

Abra sua planta em DXF ou comece um desenho novo. Em **Configurações...**, ajuste:

- **Critérios elétricos:** sistema, temperatura, tipo de cabo padrão, limites de queda e folga de cabos.
- **Alturas:** pé-direito e alturas baixa, média e alta do projeto.

As camadas e os blocos base entram sozinhos na primeira ferramenta que você usar.

## 2. Insira os pontos

Clique em **Inserir pontos**. Na mini-barra sobre o desenho, escolha categoria, bloco, potência, altura e
quadro pai. Cada clique no desenho insere um ponto, que alinha à parede mais próxima. **Tab** gira e
**Enter** termina.

Comece pelo quadro: categoria **Quadros/Racks**, bloco **Quadro QDG**.

Um ponto na altura da legenda não escreve nada na planta. Em outra altura, aparece como `h=1,45m`.

Para um símbolo próprio, use **Blocos e famílias...** e **+ Criar novo bloco elétrico**. Para mudar valores de
um bloco já inserido, selecione-o e abra o **Inspetor de atributos...**.

## 3. Crie os circuitos e os comandos

- **Criar circuito:** clique no quadro, informe o número e clique nos pontos. Enter termina. A mini-barra edita
  descrição, esquema e tensão.
- **Comandos e luminárias:** clique no interruptor e nas luminárias dele. Enter termina.

Ligue **Inspecionar projeto** para ver os halos: vermelho falta circuito ou comando, laranja sem caminho até o
quadro, amarelo acima do limite e verde válido.

## 4. Lance os eletrodutos

Clique em **Inserir eletroduto** e depois nos blocos, em sequência. Na mini-barra, escolha traçado (reto,
ortogonal ou arco), cota e tipo de eletroduto. Enter termina.

- Um clique no vazio vira um desvio do trajeto, para contornar pilares.
- Um clique no meio de um eletroduto insere uma caixa de passagem.
- Cruzamentos e derivações sem caixa **não conectam**: cada eletroduto liga exatamente dois blocos.

## 5. Dimensione

Abra **Dimensionar projeto...**, ajuste as premissas (PE compartilhado, temperatura, FCA) e clique em
**Processar cálculo**.

Em **Quadro de cargas...**:

- A aba **Quadro de cargas** mostra o resumo: fases, grupo DR, cabo e disjuntor calculados. Um circuito que
  mudou depois do cálculo aparece como **Desatualizado**.
- A aba **Ficha de engenharia** mostra cada circuito: tipo de cabo, FCT, FCA, correntes, queda de tensão e as
  travas de seção e disjuntor.

Depois de mudar algo na ficha, processe o cálculo de novo.

## 6. Documente

- **Tags de fiação:** põe a simbologia nos eletrodutos. Mova a tag para onde quiser; ela fica lá.
- **Tabela do quadro:** desenha o quadro de cargas num ponto da planta.
- **Atualizar tags e tabelas:** refaz o conteúdo sem mudar as posições. Processar o cálculo também atualiza.
- **Lista de materiais...:** cabos, eletrodutos, caixas, pontos e disjuntores.

## Antes de usar em projeto real

Os resultados são apoio ao projeto e precisam da conferência de um profissional habilitado. Confira também:

- os diâmetros internos de eletroduto, que são valores típicos de catálogo;
- os diâmetros externos de cabo, também típicos;
- as sugestões de DR e DPS;
- a tabela de demanda da sua concessionária.
