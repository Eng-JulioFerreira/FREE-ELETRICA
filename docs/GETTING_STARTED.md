# Guia de Início Rápido

Você acabou de abrir o Free Elétrica e quer saber por onde começar. Este guia leva do desenho vazio até a prancha
documentada, em oito passos. Cada passo diz o botão da paleta e o comando que você pode digitar na linha de comando.

Uma regra vale para tudo: **lançar não calcula**. Inserir pontos, circuitos e eletrodutos não muda nenhum cabo ou
disjuntor. O cálculo só muda quando você manda dimensionar.

## Passo 1: Onde encontrar as ferramentas

À esquerda da tela fica o painel **Conjuntos de ferramentas**, com as abas **Desenho**, **Modelagem** e **Instalações**.
As ferramentas do Free Elétrica estão na aba **Instalações**.

- No topo da aba, o seletor **Elétrica | Telecom** troca as ferramentas: baixa tensão de um lado, cabeamento estruturado do
  outro.
- As ferramentas ficam em seções que recolhem pela seta: **Pontos e Famílias**, **Circuitos e Comandos**, **Quadros e
  Dimensionamento**, **Eletrodutos**, **Documentação e Tabelas** e **Projeto e Inspeção**.
- Nenhuma janela fica aberta sem ser pedida. Cada janela fecha no X do cabeçalho ou com **Esc**. Com um comando em
  andamento, Esc cancela o comando.

## Passo 2: Configurando o projeto (`ELECONFIG`)

Abra sua planta em DXF ou comece um desenho novo e clique em **Configurações...**, em **Projeto e Inspeção**.

- **Unidade:** trabalhe com **1 unidade = 1 metro** (escala 1:50 na impressão). Tabelas, tags e símbolos são feitos nessa
  medida.
- **Alturas:** pé-direito e as alturas baixa (tomadas, 0,30 m), média (interruptores, 1,20 m) e alta (2,10 m). Um ponto na
  altura padrão não escreve nada na planta; em outra altura, aparece como `h=1,45m`.
- **Critérios:** temperatura ambiente, quedas de tensão admissíveis, método de instalação padrão e critério do disjuntor.
- **Cabos e eletrodutos:** os catálogos de cabos elétricos e de telecom e os perfis de eletroduto.
- **Demanda / Concessionárias:** a tabela da sua concessionária e o tipo de edificação. De fábrica vem a da Equatorial
  (NT.00001.EQTL); para outra, use **+ Cadastrar / Criar Nova Tabela de Demanda**.

As camadas e os blocos base entram sozinhos na primeira ferramenta que você usar.

## Passo 3: Inserindo pontos e empilhando na parede (`ELEPONTOS`)

Clique em **Inserir pontos**. Uma mini-barra aparece sobre o desenho:

1. Escolha a **categoria** (tomadas, iluminação, interruptores, quadros...) e o **bloco**. O campo **Pesquisar ponto...**
   filtra a lista enquanto você digita; cada item mostra a potência e a altura padrão.
2. Ajuste a potência, a altura e o quadro, se precisar.
3. Clique na planta. Cada clique insere um ponto, alinhado à parede mais próxima. **Tab** gira, **Enter** termina.

**Empilhar módulos:** para ter uma tomada e um interruptor (ou duas tomadas) na mesma caixa, clique **em cima de um
ponto que já está na parede**. O módulo novo encosta no de baixo, sem outra caixa. Dá para empilhar 2, 3 ou mais módulos
numa caixa 4x2. Cada módulo tem o seu circuito e o seu comando, e o eletroduto que chega na caixa atende todos.

Para um símbolo próprio, use **Blocos e famílias...**: a família guarda categoria, altura, caixa, potência e classe de
demanda. Para mudar um ponto já inserido, selecione-o e abra o **Inspetor de atributos...**.

## Passo 4: Criando o quadro, os circuitos e os comandos (`ELECARGAS` / `ELECOMANDO`)

1. **O quadro:** em **Inserir pontos**, escolha a categoria **Quadros/Racks** e o bloco **Quadro QDG**, e clique na planta.
2. **Os circuitos:** clique em **Criar circuito** (`ELECIRCUITO`), depois no quadro, informe o número e clique nos pontos do
   circuito. Enter termina. A mini-barra edita descrição, esquema e tensão.
3. **Os comandos:** clique em **Comandos e luminárias** (`ELECOMANDO`), no interruptor e nas luminárias que ele acende.
   Para paralelos (three-way) e intermediários (four-way), clique também nos outros interruptores do mesmo comando, na
   ordem do caminho.
4. **O quadro de cargas:** **Quadro de cargas...** (`ELECARGAS`) abre o quadro embaixo do desenho, com as fases, os grupos de
   DR e a ficha de cada circuito. O menu **...** de cada linha adiciona e remove pontos, localiza o circuito e o exclui.

Ligue **Inspecionar projeto** para ver o que falta: vermelho falta circuito ou comando, laranja sem caminho até o quadro,
amarelo acima do limite e verde válido.

## Passo 5: Traçando os eletrodutos (`ELEELETRODUTO`)

Clique em **Inserir eletroduto** e depois nos blocos, em sequência. Na mini-barra, escolha o **perfil** (teto, parede ou
piso, com o material e os diâmetros) e o **traçado**: **Reto**, **Ângulo 45°** ou **Arco**. Enter termina.

- Um clique no vazio vira um desvio, para contornar um pilar.
- Um clique no meio de um eletroduto insere uma caixa de passagem.
- Cruzamentos sem bloco **não conectam**: cada eletroduto liga exatamente dois blocos, como na obra.
- O tipo de linha mostra o caminho: contínua no teto, tracejada na parede, traço e ponto no piso.

## Passo 6: Dimensionando e gerando as tags (`Dimensionar projeto...` e `ELEFIACAO`)

Abra **Dimensionar projeto...** (`ELEDIMENSIONAR`) e escolha o quadro.

1. Em cada circuito, escolha o **esquema**, o **tipo de cabo**, o **método** de instalação e o **grupo DR**.
2. **Vários de uma vez:** clique no número de um circuito para selecioná-lo, **Ctrl+clique** para somar outros,
   **Shift+clique** para pegar um intervalo e **Ctrl+A** para todos. Com duas ou mais linhas selecionadas, a mudança
   feita numa delas vale para todas. O mesmo vale no quadro de cargas, no Quadro de Telecomunicações e nas listas de
   pontos e de eletrodutos.
3. Monte os grupos de DR com **+ Novo Grupo DR** e **+ Adicionar circuito...**, ou pelo botão **Adicionar ao DR** de cada
   circuito sem DR.
4. Em **Proteção geral, DPS e montagem do quadro**, confira o disjuntor geral, a seção do alimentador, o DPS e a montagem
   dos trilhos DIN. O cadeado mostra o que você travou à mão.
5. Clique em **Calcular Dimensionamento**, revise e clique em **Aplicar ao Quadro**.

**Tags de fiação** (`ELEFIACAO`): a mini-barra escolhe o modo. **Direto no eletroduto** é um clique no trecho.
**Com linha de chamada** são dois cliques: o ponto no eletroduto e onde a base da tag começa, no ângulo que você quiser.
Enter no começo põe uma tag no meio de todos os eletrodutos.

## Passo 7: Gerando a documentação na prancha

Na seção **Documentação e Tabelas**:

- **Tabela do quadro** (`ELEQUADRO_TABELA`): o quadro de cargas desenhado na planta.
- **Diagrama unifilar** (`ELEUNIFILAR`): o diagrama do quadro, com o quadro de simbologia embaixo.
- **Vista frontal do quadro** (`ELEQUADRO_FRONTAL`): o layout de montagem nos trilhos DIN, com os barramentos.
- **Legenda de símbolos** (`ELELEGENDA`): os símbolos elétricos usados no desenho.
- **Lista de materiais...** (`ELEMATERIAIS`): cabos, eletrodutos, caixas, disjuntores, DR, DPS, barramentos e quadros.
- **Atualizar tags e tabelas** (`ELEREGEN`): refaz tudo o que foi gerado, sem mudar as posições. Use depois de qualquer
  mudança.

## Passo 8: Fluxo rápido de telecomunicações (`Telecom`)

No seletor do topo, escolha **Telecom**.

1. **Inserir rack** e **Inserir ponto de dados** já começam com o bloco certo. Para CFTV, acesso sem fio ou fibra, use uma
   família de dados com esse nome ou escolha o cabo do ponto.
2. Ligue os pontos ao rack com **Inserir eletroduto**. O cabo de telecom passa por outros pontos de dados e por caixas de
   passagem, nunca por uma caixa de tomada.
3. **Numerar e vincular ao rack** (`TELEVINCULAR`): liga cada ponto ao rack e dá a ele o número da porta (1, 2, 3...), com o
   cabo e a quantidade de cabos.
4. **Dimensionar telecom...** (`TELEROTEAR`): um cabo inteiro de cada ponto até o rack (topologia em estrela), o comprimento
   3D com alerta acima de 90 m e o diâmetro de cada eletroduto com 40 % de ocupação. O resultado abre no **Quadro de
   Telecomunicações**, embaixo do desenho, onde você troca a descrição, o cabo e a quantidade de cada porta.
5. **Tags de cabeamento** (`TELEFIACAO`): a quantidade e o tipo de cabo em cima, o diâmetro embaixo, nos mesmos dois modos
   da elétrica. **Tabela do rack** (`TELETABELA`) e **Legenda de símbolos de telecom** (`TELELEGENDA`) completam a prancha.

## Referência rápida dos comandos

| Botão | Comando | Para que serve |
| :--- | :--- | :--- |
| Configurações... | `ELECONFIG` | Unidade, alturas, critérios, catálogos e demanda |
| Inserir pontos | `ELEPONTOS` | Lançar e empilhar pontos com a mini-barra |
| Blocos e famílias... | `ELEBLOCOS` | Criar e editar símbolos próprios |
| Criar circuito | `ELECIRCUITO` | Agrupar pontos num circuito do quadro |
| Comandos e luminárias | `ELECOMANDO` | Ligar interruptores às luminárias |
| Quadro de cargas... | `ELECARGAS` | Fases, DR e ficha de cada circuito |
| Inserir eletroduto | `ELEELETRODUTO` | Ligar os blocos pelo teto, parede ou piso |
| Dimensionar projeto... | `ELEDIMENSIONAR` | Cabos, disjuntores, DR, DPS; calcular e aplicar |
| Tags de fiação | `ELEFIACAO` | Simbologia de fiação nos eletrodutos |
| Tabela do quadro | `ELEQUADRO_TABELA` | O quadro de cargas na prancha |
| Diagrama unifilar | `ELEUNIFILAR` | O unifilar com o quadro de simbologia |
| Vista frontal do quadro | `ELEQUADRO_FRONTAL` | A montagem nos trilhos DIN |
| Legenda de símbolos | `ELELEGENDA` | A legenda dos símbolos elétricos |
| Atualizar tags e tabelas | `ELEREGEN` | Refazer tudo o que foi gerado |
| Lista de materiais... | `ELEMATERIAIS` | O levantamento de materiais |
| Inspecionar projeto | `ELEINSPECAO` | Os halos do que falta no projeto |
| Numerar e vincular ao rack | `TELEVINCULAR` | Porta e rack de cada ponto de telecom |
| Dimensionar telecom... | `TELEROTEAR` | Cabos em estrela e eletrodutos de telecom |
| Tags de cabeamento | `TELEFIACAO` | Cabos e diâmetro nos eletrodutos de telecom |
| Tabela do rack | `TELETABELA` | As portas do rack na prancha |
| Legenda de símbolos de telecom | `TELELEGENDA` | A legenda dos símbolos de telecom |

## Antes de usar em projeto real

Os resultados apoiam o projeto e precisam da conferência de um profissional habilitado. Confira em especial os diâmetros
de cabos e eletrodutos (valores típicos de catálogo), as tabelas de capacidade de condução na norma vigente, as sugestões
de DR e DPS e a tabela de demanda da sua concessionária.
