# Especificação do Padrão de Blocos Elétricos (CADCraft / NBR 5410)

Este documento estabelece as regras e convenções de nomenclatura para os blocos CAD (`BLOCK` e `INSERT`) e seus atributos (`ATTRIB`) no ecossistema do **Free Elétrica / CADCraft**.

---

## 🎯 1. Filosofia de Modelagem

Para que o motor de cálculo da **NBR 5410** realize o dimensionamento e o roteamento de fiação de forma automatizada, cada elemento elétrico inserido no desenho deve carregar seus parâmetros essenciais como atributos vinculados.

Os atributos são case-insensitive (não diferenciam maiúsculas de minúsculas) e toleram sinônimos comuns.

---

## 📋 2. Estrutura Unificada de Atributos (Modelo Orientado a Objetos)

Seguindo o paradigma de **Programação Orientada a Objetos (POO)**, todo ponto elétrico deriva de uma mesma classe base (`PontoEletrico`). Portanto, **todos os blocos compartilham exatamente os mesmos atributos**:

| Atributo | Descrição | Exemplo Tomada / Iluminação | Exemplo Interruptor |
| :--- | :--- | :--- | :--- |
| `TIPO_PONTO` | Categoria do elemento | `TOMADA`, `TUG`, `TUE`, `ILUM` | `INTERRUPTOR`, `INT_SIMPLES` |
| `POTENCIA` | Potência nominal | `100W`, `600VA`, `5400W` | **`0W` (Sempre zerada: 0)** |
| `TENSAO` | Tensão nominal | `127V`, `220V` | `127V`, `220V` |
| `NUMERO_FASES` | Quantidade de fases | `1`, `2`, `3` | `1` |
| `CIRCUITO` | Circuito associado | `1`, `2` | `1`, `2` |
| `COMANDO` | Tecla / acionamento | `a` (em lâmpadas) ou vazio | **`a`, `b`, `c` (Obrigatório)** |
| `TAG` | Identificador individual | `TUG-01`, `LUM-01` | `INT-01` |
| `ALTURA` | Altura de instalação | `0,30`, `30cm`, `2,20 m` | `1,10` |
| `QUADRO` (opcional) | TAG do quadro que alimenta | `QD-01` | `QD-01` |
| `TIPO_CARGA` (opcional) | Tipo de carga no cálculo de demanda | `ILUM_TUG`, `AQUECIMENTO`, `AR` | (não se aplica) |
| `FP` (opcional) | Fator de potência | `0,92`, `1`, `85%` | (não se aplica) |

`TIPO_CARGA` aceita `ILUM_TUG`, `AQUECIMENTO`, `ELETRODOMESTICOS`, `AR`, `BOMBA`, `OUTROS` e
`RESERVA`, e também nomes comuns como "chuveiro", "split" ou "geladeira". Sem ele, a TUE conta
como aquecimento e o resto como iluminação e TUG. Sem `FP`, vale 1,00 em aquecimento e 0,92
no resto. O Quadro de cargas mostra o resultado por circuito: os tipos dos pontos e o FP do
circuito (W / VA).

### Blocos base

Na primeira ferramenta usada num desenho, entram sem perguntar as camadas e os blocos que faltam:


| Blocos | Símbolo | Altura |
| :--- | :--- | :--- |
| `TOM-SIM-BAIXA` | Triângulo sem hachura | 0,30 m |
| `TOM-SIM-MEDIA` | Meia hachura | 1,20 m |
| `TOM-SIM-ALTA` | Hachura cheia | 2,10 m |
| `INT-SIM` | Interruptor simples: círculo vazado | Ponto médio, 1,20 m |
| `INT-PAR` | Paralelo (three-way): círculo cheio | Ponto médio |
| `INT-INT` | Intermediário (four-way): meia hachura | Ponto médio |
| `INT-SIM-ALTO`, `INT-PAR-ALTO`, `INT-INT-ALTO` | Os mesmos | Ponto alto, 2,10 m |

Cada bloco é um módulo, com o seu `CIRCUITO` e o seu `COMANDO`. Tomada dupla ou interruptor de duas teclas são dois módulos
empilhados: o segundo, inserido no mesmo ponto (a até 10 cm) ou com `eletrica.ponto.empilhar`, usa o bloco `<nome>$EMP`, sem
a caixa 4x2 (as peças da caixa têm o XDATA `wall_box`), encosta no topo do último símbolo e grava `host_point_handle`.

Também entram `TOM-TUE`, `ILU-OCT`, `QDG`, `QDC`, `RJ45`, `RACK` e `CX-PASSAGEM`. Um bloco com o mesmo nome já no desenho
fica como está: é assim que se usa outro símbolo. Só o desenho padrão de uma versão anterior é trocado. Excluído na janela
de blocos, um bloco de fábrica não volta sozinho; **Configurações > Geral** o traz de volta. Interruptores têm
`TIPO_INTERRUPTOR` constante (`1S`, `2S`, `3S`, `PARALELO`, `PARALELO-2S`, `PARALELO-3S`, `INTERMEDIARIO`,
`INTERMEDIARIO-2S`, `INTERMEDIARIO-3S`), que diz quantas teclas têm.

### Famílias

Cada definição de bloco pode ter uma família, gravada no XDATA do `BLOCK_RECORD` (aplicativo
`FREE_ELETRICA`), editada na paleta enquanto o editor de blocos (`BEDIT`) está aberto ou pelo comando
`eletrica.familia.definir`:

| Chave | Valores |
| :--- | :--- |
| `domain` | `POWER`, `DATA_TELECOM`, `FIRE_SECURITY`, `INFRA` |
| `category` | `NONE`, `ELECTRICAL_FIXTURE`, `LIGHTING_FIXTURE`, `LIGHTING_DEVICE`, `DATA_DEVICE`, `FIRE_SECURITY_DEVICE`, `ELECTRICAL_EQUIPMENT`, `TELECOM_ENCLOSURE`, `JUNCTION_BOX` |
| `height_ref` | `LOW`, `MEDIUM`, `HIGH`, `CEILING`, `FLOOR`, `FIXED` (com `fixed_elevation_m`) |
| `physical_box` | `NONE`, `4X2`, `4X4`, `OCTAGONAL`, `PULL_15X15`, `PULL_20X20`, `PULL_30X30` |
| `power_va`, `power_factor`, `keys` | Potência, FP e teclas padrão |
| `legend_name`, `material_desc` | Nome na Legenda de Símbolos e descrição na Lista de Materiais |

Cada inserção herda os dois textos e pode ter os seus, nos atributos ou no XDATA `LEGENDA` e `DESC_MATERIAL`
(`eletrica.ponto.documentacao`, ou o Inspetor de atributos). Vazio volta ao texto da família.

A família manda sobre o nome do bloco e sobre `TIPO_PONTO`. `NONE` marca um bloco comum de CAD, que
nunca é ponto, mesmo com atributos elétricos. Dados, racks e dispositivos especiais ficam fora dos
circuitos e da inspeção de força.

### Altura na planta

As alturas padrão ficam nas Configurações. Uma inserção na altura da legenda deixa o atributo
`ALTURA` vazio e acompanha a legenda se ela mudar. Em outra altura, o valor exato vai para o XDATA
(`elevation_m`), que é o que o cálculo usa, e o atributo mostra `h=1,45m`. Blocos antigos com um número
em `ALTURA` continuam valendo.

### Blocos do template (opcional: `templates/FREE_ELETRICA_BLOCOS.dxf`)

| Bloco | Símbolo | Inserção | TIPO_PONTO (constante) | Padrões | Visíveis |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `INT-SIM` | Interruptor simples | meio da aresta na parede | `INTERRUPTOR` | 0W, 220V, 1 fase, 1,10 m | COMANDO |
| `TOM-SIM-BAIXA` | Tomada simples baixa | meio da aresta na parede | `TUG` | 100VA, 220V, 1 fase, 0,30 m | CIRCUITO |
| `QDG` | Quadro de distribuição | meio da aresta na parede | `QUADRO` | TAG QDG, 380/220V, 3 fases, 1,50 m | TAG |
| `ILU-OCT` | Ponto de luz, caixa octogonal | centro | `ILUM` | 100VA, 220V, 1 fase, no teto | POTENCIA, COMANDO, CIRCUITO |

`TOM-SIM-BAIXA` e `ILU-OCT` trazem também `TIPO_CARGA` = `ILUM_TUG` e `FP` = `0,92`,
invisíveis.

Camadas: `ELE-PONTOS`, `ELE-QUADROS`, `ELE-FIACAO`, `ELE-ELETRODUTO-TETO`, `ELE-ELETRODUTO-PISO`
e `ELE-ELETRODUTO-PAREDE`. Camadas antigas com `ELETRODUTO` no nome continuam valendo. Para limpar
blocos que não são mais usados num desenho, use o comando `PURGE` (ou `PU`).

### Tipo de cabo e tipo de eletroduto

Cada circuito tem o seu tipo de cabo, no XDATA do quadro (`CABO.<circuito>`): `FLEX_PVC_70`, `UNI_PVC_70`,
`HEPR_XLPE_90`, `LSZH_70` ou `LSZH_HEPR_90`. Sem ele, vale o cabo padrão das Configurações. A temperatura
da isolação (70 °C ou 90 °C) escolhe a capacidade de condução e o FCT do circuito; a tensão (450/750 V ou
0,6/1 kV) escolhe a coluna de diâmetro externo. Um `ISOL.<circuito>` de desenho anterior vira o cabo
equivalente.

Cada eletroduto guarda o tipo no XDATA (`conduit_type`): `PVC_CORR_LIGHT` (PVC-CL), `PVC_CORR_HEAVY`
(PVC-CR), `PVC_RIGID` (PVC-R), `HDPE_CORR` (PEAD), `STEEL_ELECTRO` (AG-E) ou `STEEL_HOT_DIP` (AG-F). Sem
ele, é PVC rígido. Os diâmetros internos de cada tipo entram na ocupação. O PVC rígido usa a tabela das
Configurações; os outros usam valores típicos de catálogo no código, a conferir com o fabricante.

### Tags de fiação

`ELEFIACAO` tem dois modos, escolhidos na mini-barra ou por `eletrica.fiacao.modo`. Direto no eletroduto (`direct`): um clique, e
as marcas ficam sobre o trecho, perpendiculares a ele naquele ponto. Com linha de chamada (`leader`): o primeiro clique no
eletroduto e o segundo onde a base começa; a chamada liga os dois pontos no ângulo que der, e a base horizontal sai do segundo
clique. O XDATA guarda `tag_mode`, `leader_anchor_x/y` e `leader_tag_x/y`, e `ELEREGEN` refaz essa geometria. Em lote (Enter no começo, ou o JSON sem pontos), a tag fica perpendicular ao meio do trecho. A tag é um bloco
`ELE_TAG_FIACAO_<handle>` com uma marca por condutor, agrupadas por circuito:

- em cima, o número do circuito sobre a primeira marca e a letra sobre cada retorno (lado a
  lado leem `1b`);
- embaixo do grupo, a seção (`#1,5`), quando não omitida nas Configurações;
- na linha de baixo, centrado sob toda a simbologia, o diâmetro do eletroduto do trecho
  (`Ø20`).

As tags ficam na camada `ELE-FIACAO` e as marcas são blocos em cor 7 (preto no fundo claro, branco no
escuro):

| Bloco | Condutor | Desenho |
| :--- | :--- | :--- |
| `FIACAO-FASE` | Fase | traço que atravessa o eletroduto |
| `FIACAO-NEUTRO` | Neutro | traço que atravessa, com bandeira para a esquerda no topo |
| `FIACAO-RETORNO` | Retorno | traço curto só acima do eletroduto |
| `FIACAO-PROTECAO` | Terra (proteção) | traço que atravessa, com barra no topo (T) |

Se o desenho não tem esses blocos, eles são criados com esse desenho padrão. Blocos padrão de
versões anteriores do Free Elétrica são trocados pelo atual. Para usar outro símbolo, redesenhe o
bloco no seu template: um bloco redesenhado fica como está.

A tag é ligada ao eletroduto pelo XDATA (`parent_conduit_handle`) e guarda onde você a deixou. Mover
a tag no desenho é um ajuste permanente: a regeneração atualiza o conteúdo e mantém o deslocamento, e
uma tag afastada ganha uma linha de chamada até o eletroduto. Tag sem dados mostra `?`. A tag de um
eletroduto apagado some junto. Edições feitas pelas ferramentas elétricas regeneram sozinhas; depois
de mover objetos com os comandos do CAD, use `ELEREGEN`.

O diâmetro do eletroduto, se não foi travado à mão na aba Eletrodutos, é o menor tamanho, a partir do
mínimo das Configurações, cuja área interna comporta os condutores do trecho: ocupação máxima de 53 % com um condutor, 31 % com
dois e 40 % com três ou mais (NBR 5410, 6.2.11.1.6). Os diâmetros externos dos condutores
(por isolação) e os internos dos eletrodutos ficam nas abas Condutores e Eletrodutos das
Configurações. Os cabos de telecom têm diâmetro externo obrigatório no cadastro.

Condutores por trecho: fase do quadro às tomadas, interruptores e luminárias sem interruptor;
neutro e proteção do quadro às cargas, conforme o esquema do circuito; retorno de cada interruptor
às luminárias com a letra dele; dois retornos paralelos (`a'` e `a`") entre interruptores paralelos
ou intermediários da mesma letra.

`ALTURA`, junto com o pé-direito das Configurações, dá o comprimento real do cabo: os
eletrodutos correm no teto, no piso ou na parede, e cada ponta desce ou sobe até a altura do bloco:
comprimento = 2D + |cota - Z de A| + |cota - Z de B|. Sem `ALTURA`, valem: TUG/TUE
0,30 m, interruptor 1,10 m, quadro 1,50 m, iluminação no teto (sem aviso). Um número sem unidade acima de
10 é lido em centímetros (`30` = 0,30 m).

`CIRCUITO`, `COMANDO` e `QUADRO` não precisam existir no bloco: as ferramentas **Criar
circuito** e **Comandos e luminárias** gravam no atributo quando ele existe e, senão, no XDATA
do bloco (aplicativo `FREE_ELETRICA`), que viaja com o DWG/DXF e é copiado junto com o bloco. Com um
só quadro no desenho, todo ponto é dele sem precisar de `QUADRO`.

> [!IMPORTANT]
> **Regra do Interruptor em POO**:
> O interruptor possui **todos** os atributos da família de pontos elétricos. A sua potência é **sempre 0W** (zero), e a única diferença operacional é que ele possui o atributo `COMANDO` preenchido indicando a tecla de acionamento da lâmpada.


---

### C. Quadros de Distribuição (`QD`)
Ponto de origem e alimentação dos circuitos terminais. Ficam no XDATA do bloco do quadro
(aplicativo `FREE_ELETRICA`; o nome antigo `FREEELETRICA` continua sendo lido):

- as escolhas travadas no Quadro de cargas (`SECAO.<circuito>` e
  `DISJ.<circuito>`);
- as descrições do Quadro de cargas (`DESC.<circuito>`), que também guardam os circuitos
  criados ainda sem pontos;
- a isolação dos condutores do quadro (`ISOLACAO` = `PVC`, `EPR` ou `XLPE`); sem ela, vale a
  isolação padrão das Configurações;
- a isolação de um circuito (`ISOL.<circuito>`), que vale sobre a do quadro;
- o esquema (`ESQUEMA.<circuito>`), a tensão (`TENSAO.<circuito>`) e as fases travadas
  (`FASE.<circuito>`) de um circuito.

`TENSAO` do quadro no formato `220/127V` dá as tensões fase-fase e fase-neutro do quadro e dos
circuitos sem `TENSAO`; sem ela, valem as do sistema das Configurações.

| Atributo | Descrição | Exemplos Válidos |
| :--- | :--- | :--- |
| `TAG` | Nome do quadro | `QD-01`, `QGBT`, `QDF` |
| `TENSAO` | Tensão de fornecimento | `220/127V`, `380/220V` |
| `FASES` | Fases de entrada do quadro | `2` (Bifásico), `3` (Trifásico) |
| `DISJUNTOR_GERAL` | Proteção geral pré-definida (opcional) | `50A`, `63A`, `AUTO` |

---

## 🔎 3. Normalização e Validação no Adaptador

O adaptador do Free Elétrica realiza:
1. Normalização automática de strings: remove espaços em excesso e converte para maiúsculas internamente.
2. Extração de valores numéricos de potência (ex: extrai `600` de `'600VA'` ou `'600 W'`).
3. Validação de consistência: emite alerta caso um interruptor contenha potência indevida ou uma tomada não contenha tensão/circuito especificado.

---

## 🧭 4. Como o painel Elétrica reconhece os blocos

Um bloco só entra na análise se tiver **pelo menos um** destes atributos: `TIPO_PONTO`,
`POTENCIA`, `TENSAO`, `NUMERO_FASES`, `CIRCUITO`, `COMANDO`, `ALTURA` ou `QUADRO` (no
atributo ou no XDATA do Free Elétrica). Blocos de arquitetura sem
atributos elétricos são ignorados, mesmo com nomes parecidos.

O tipo vem do valor de `TIPO_PONTO`. Se ele faltar, vem do nome do bloco. Maiúsculas e
acentos não importam.

| Tipo reconhecido | Valores ou nomes aceitos (exemplos) | É carga? |
| :--- | :--- | :--- |
| Quadro | `QUADRO`, `QD`, `QD-01`, `QGBT` | não |
| Interruptor | `INTERRUPTOR`, `INT`, `INT_SIMPLES`, `PULSADOR`, `DIMMER` | não (0 W) |
| TUE | `TUE`, `TOMADA_TUE`, `CHUVEIRO`, `ESPECIFICA` | sim |
| TUG | `TUG`, `TOMADA`, `TOM` | sim |
| Iluminação | `ILUM`, `LUZ`, `CAIXA DE LUZ`, `LUMINARIA`, `LAMPADA`, `PLAFON`, `SPOT` | sim |
| Caixa de passagem | `CAIXA`, `CAIXA DE PASSAGEM`, `CP` | não |

> [!NOTE]
> Ter o atributo `COMANDO` **não** transforma um ponto em interruptor. Uma luminária com
> `COMANDO = a` continua sendo carga de iluminação.

Valores aceitos nos demais atributos:

- `POTENCIA`: `100VA`, `600 W`, `1,5kW`, `5400W`, `1.500 W`. W é tratado como VA por enquanto.
- `TENSAO`: `127V`, `220V`. No quadro, `220/127V` indica bifásico se faltar `NUMERO_FASES`.
- `NUMERO_FASES`: `1`, `2`, `3`, ou `MONO`, `BI`, `TRI`.
- Atributos **constantes** da definição do bloco também são lidos.
