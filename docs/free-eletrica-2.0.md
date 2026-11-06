# Free Elétrica 2.0

Registro da especificação "Free Elétrica 2.0": o que estava pronto, o que mudou, as estruturas do
grafo, a divisão em fases e onde a implementação se afasta do texto. A versão anterior está na tag
`v1-antes-da-2.0` e no branch `versao-1`.

## Diagnóstico da versão 1

| Área | Estado na versão 1 | Na 2.0 |
| :--- | :--- | :--- |
| Motor sem interface, tudo por comando | pronto | mantido |
| Dados no desenho (atributos e XDATA) | pronto, AppID `FREEELETRICA` | AppID `FREE_ELETRICA`, `schema_ver = 2`; os nomes antigos continuam sendo lidos e migram na próxima gravação |
| Cálculo NBR 5410, demanda da concessionária | pronto | ganhou I'b, esquema por circuito, fases travadas e alimentador |
| Rede de eletrodutos | **refeita**: pontas emendadas por proximidade, derivação em T e bloco no meio da linha ligavam | grafo estrito |
| Fiação | **refeita**: marcas apagadas e redesenhadas, perdendo o ajuste manual | tags com deslocamento guardado |
| Interface | **refeita**: janelas flutuantes | paleta, barra de contexto, painel de baixo, sobreposição |
| Blocos | exigia abrir o template | auto-bootstrap |

## Fases

| Fase | Comandos | Conteúdo |
| :--- | :--- | :--- |
| 1. Pontos | `ELEPONTO`, `ELEPONTOS`, `eletrica.bootstrap`, `eletrica.ponto.modelo` | camadas e blocos sem perguntar, barra de inserção (tipo, altura, potência, FP), alinhamento à parede, Tab gira |
| 2. Circuitos e comandos | `ELECIRCUITO`, `ELEREMOVERPONTO`, `ELECOMANDO`, `eletrica.circuito.*` | esquema e tensão por circuito, próxima letra respeitando as teclas do interruptor, letra própria, `target_handles` |
| 3. Cargas | `ELECARGAS`, `eletrica.rotear`, `eletrica.sobrescrever` | I'b, fases R, S, T com trava, alimentador, DR e DPS, queda estimada (`~`) ou real |
| 4. Eletrodutos | `ELEELETRODUTO`, `eletrica.eletroduto.*`, `eletrica.eletrodutos` | grafo estrito, divisão com caixa, comprimento 3D, retornos paralelos, diâmetro por ocupação com trava |
| 5. Documentação | `ELEFIACAO`, `ELEREGEN`, `ELEQUADRO_TABELA` | tags não destrutivas, regeneração automática nas edições, tabela no desenho |
| Interface | `ELECONFERIR`, `ELEINSPECAO` | quatro zonas, inspeção visual |

As fases foram entregues juntas: mudar a rede quebra os comandos, e os comandos sustentam a
interface, então nenhuma delas compila e passa nos testes sozinha.

## O grafo estrito

`crates/free-eletrica/src/rota.rs`:

```rust
pub enum Instalacao { Teto, Piso, Parede }

/// Um nó: um bloco elétrico.
pub struct No { pub handle: Handle, pub tipo: TipoPonto, pub pos: [f64; 2], pub z: f64 }

/// Uma aresta: um eletroduto entre dois blocos.
pub struct Aresta {
    pub eletroduto: Handle,
    pub de: usize,
    pub para: usize,
    pub instalacao: Instalacao,
    pub comprimento_2d: f64,
    /// 2D + |cota - Z de A| + |cota - Z de B|.
    pub comprimento: f64,
    pub meio: [f64; 2],
    pub angulo: f64,
}

/// Um eletroduto que não liga dois blocos: fica fora do cálculo e é apontado.
pub struct Solto { pub eletroduto: Handle, pub motivo: String, pub meio: [f64; 2], pub angulo: f64 }

pub struct Rede { pub nos: Vec<No>, pub arestas: Vec<Aresta>, pub soltos: Vec<Solto>, /* adjacência */ }
```

Regras:

- Nós são só blocos elétricos. Arestas são linhas, polilinhas abertas, arcos e splines abertas nas
  camadas de eletroduto.
- Cada ponta vale pelo handle gravado (`from_handle`, `to_handle`) quando o bloco está dentro do
  raio de captura; senão, pelo bloco mais perto dentro do raio. Sem bloco, o eletroduto é `Solto`.
- Cruzamento em X, derivação em T e bloco encostado no meio da linha **não** conectam. Duas linhas
  com pontas coincidentes sem bloco também não.
- Cota: teto = pé-direito, piso = 0, parede = Z do bloco A.
- O caminho até o quadro é o menor em comprimento 3D. Sem caminho, o comprimento é estimado por
  Manhattan 3D (ΔX + ΔY + ΔZ) e aparece com `~`.

## Esquema do XDATA

AppID `FREE_ELETRICA`, em grupos `chave=valor`:

| Onde | Chaves |
| :--- | :--- |
| Eletroduto | `schema_ver`, `from_handle`, `to_handle`, `install_elevation` (CEILING, FLOOR, WALL), `nominal_diameter_mm`, `manual_diameter_lock`, `passing_wires.<n>` (`circuito:funções:seção`), `tag_handle` |
| Tag de fiação | `schema_ver`, `parent_conduit_handle`, `anchor_x`, `anchor_y`, `user_offset_x`, `user_offset_y` |
| Interruptor | `target_handles.<letra>` |
| Quadro | `SECAO.`, `DISJ.`, `DESC.`, `ISOL.`, `ESQUEMA.`, `TENSAO.`, `FASE.` + circuito; `ISOLACAO` |

O bloco da tag se chama `ELE_TAG_FIACAO_<handle do eletroduto>`, e o da tabela,
`QUADRO_TABELA_<handle do quadro>`.

## Onde a implementação se afasta do texto

- **Dados dos pontos em atributos.** A especificação pede `power_va`, `power_factor` etc. no XDATA
  dos pontos. Eles ficam nos atributos do bloco, que o projetista vê e edita (regra 6 do
  `CLAUDE.md` e pedido anterior sobre FP e TIPO_CARGA). O XDATA é o plano B quando o bloco não tem o
  atributo. Duplicar criaria duas fontes para o mesmo valor.
- **Eletroduto em spline.** Continua existindo o arco pedido antes; flecha 0 faz uma linha.
- **Espaço não gira.** Tab gira o ponto, Espaço continua sendo Enter, como em todo o CAD.
- **Parede como cota.** A especificação cita teto, piso e parede. A cota de parede é a do bloco A.
- **Painel só embaixo do desenho.** Ele ocupa a largura da área de desenho, não a da janela.
- **`elevation_m`.** É o atributo `ALTURA`; o Z da inserção do bloco fica em 0.

## O que muda em desenhos existentes

- Eletrodutos emendados sem bloco, derivações em T e pontos no meio de uma linha deixam de ligar. A
  inspeção aponta cada um, e o comprimento vira estimativa (`~`) até pôr as caixas.
- As marcas de fiação da versão 1 são apagadas na primeira regeneração. Use Tags de fiação para
  criar as novas.
- As fases passam a se chamar R, S, T.

## Testes reescritos

Os testes abaixo fixavam comportamento que a especificação pediu para mudar:

| Teste antigo | Por que mudou |
| :--- | :--- |
| Rede e roteamento com a planta de linhas contínuas | a planta agora liga bloco a bloco, porque um bloco no meio da linha não conecta mais |
| Passagem do teto ao piso | passa por uma caixa, e o comprimento segue a fórmula 3D |
| Sem eletrodutos estima e avisa | a estimativa agora é Manhattan 3D, não "em L com as pernas" |
| Ajustar fiação desenha e refaz | as marcas viraram tags; há um teste novo de posição preservada |
| Eletroduto por coordenadas | as pontas precisam ser blocos |
| Fases A, B, C | agora R, S, T |

## O que ficou parcial

- Motores, outras concessionárias além da Equatorial e o roteamento de telecom não entraram.
- DR e DPS são sugestões a conferir: DR 30 mA, DPS classe II com In ≥ 5 kA e Uc ≥ 1,1 Uo.
- O Ø16 dos tamanhos de eletroduto é valor típico de catálogo: confira com o fabricante.
- Mover um bloco ou eletroduto com os comandos do CAD não dispara a regeneração: use Atualizar tags
  e tabelas (`ELEREGEN`). Só as edições feitas pelas ferramentas elétricas regeneram sozinhas.
- O template `templates/FREE_ELETRICA_BLOCOS.dxf` é opcional e não foi refeito. Os blocos dele têm
  prioridade sobre os base, e as camadas antigas continuam valendo como eletroduto.

## Reestruturação em 4 etapas

Segunda especificação, com famílias de bloco, separação entre lançar e dimensionar e vários sistemas.
O que já existe da 2.0 fica; cada etapa acrescenta ou ajusta.

### Etapa 1: Setup, famílias e pontos (feita)

- `SystemDomain`, `FamilyCategory`, `RefAltura` e `CaixaFisica` em `crates/free-eletrica/src/familia.rs`.
- Família no XDATA do `BLOCK_RECORD`, painel no editor de blocos, geometria base, ponto de conexão e
  atributos padrão.
- Configurações com as abas Critérios elétricos e Alturas, camada `ELE-DADOS`, blocos `RJ45` e `RACK`
  e o botão Inicializar projeto.
- `ELEPONTOS` com filtros por categoria e campo de altura, e a regra do `h=X,XXm`.

### Etapa 2: Circuitos e quadro de cargas em 2 abas

- Separar lançar de dimensionar: nos pontos e circuitos só somar VA, W e Ib; FCA, queda e a escolha de
  cabos e disjuntores só no botão Dimensionar.
- Aba 1, enxuta: circuito, descrição, esquema, pontos, potência, Ib, fases, grupo DR, resumo e status;
  Equilibrar fases e Gerenciar módulos IDR.
- Aba 2, mestre-detalhe: método de instalação, FCT, FCA, I'b, distância, comparativo das três seções
  mínimas, travas de seção, disjuntor com curva, polos e módulos DIN.
- Limite de queda do alimentador no cálculo.

### Etapa 3: Grafo com domínios

- `domain` nas arestas (`POWER`, `DATA_TELECOM`) e cabos UTP e fibra nos condutores passantes.
- Perguntar antes de inserir a caixa ao terminar um eletroduto no meio de outro.
- Rack como origem da rede de dados; inspeção por domínio.

### Etapa 4: Documentação e materiais

- `ELEMATERIAIS`: caixas por tipo (a caixa física das famílias), pontos, disjuntores, metros de
  eletroduto em 3D e metros de cabo por seção.
- Tabela do quadro seguindo as colunas novas; tags de dados.

### O que a Etapa 1 deixou de fora ou decidiu diferente

- **Medidas da geometria base:** são proporcionais aos símbolos base do projeto, não medidas conferidas
  com a NBR 5444, que a especificação cita. Confira antes de tratá-las como oficiais.
- **Painel, não janela:** a especificação fala em janela `ELEPONTOS`; ela segue como aba do painel de
  baixo, sem bloquear o desenho. Escolher o bloco já começa a inserir.
- **Filtro de especiais:** dispositivos especiais e de incêndio só aparecem em Todos, porque a lista de
  filtros pedida não tem um botão para eles.
- **Queda do alimentador:** o valor já fica no Setup, mas só entra no cálculo na Etapa 2.
- **Marcador de conexão:** fica na camada `Defpoints`, que por convenção de CAD não plota; ele aparece na
  tela dentro de cada inserção.
- **Barra sobre o desenho:** continua existindo durante o `ELEPONTO`, junto com a lista.

## Correções depois do teste prático

Quatro pedidos depois de usar a versão no CADCraft, que mudam parte do que está acima:

- **`ELEPONTOS` voltou à mini-barra:** categoria, bloco, potência, altura e quadro pai. O comando já começa
  com um bloco escolhido, então o primeiro clique insere. A janela de pontos só lista e edita.
- **Painel de baixo removido:** tudo virou janela independente, aberta só quando pedida, com X e Esc. Esc
  fecha a última aberta, e não age com um comando em andamento, que usa Esc para cancelar.
- **Tipo de cabo por circuito**, com cinco tipos e a tabela de 70 °C ou 90 °C de cada um. Na lista de
  materiais, os cabos saem por seção e tipo.
- **Tipo de eletroduto**, com seis tipos, gravado em cada trecho, usado na ocupação e separado por tipo e
  diâmetro na lista de materiais.

Limites:

- **Dimensionar** refaz o cálculo do quadro e atualiza tags e tabelas. Os valores continuam visíveis antes
  do botão: a separação completa entre lançar e dimensionar segue para a Etapa 2.
- **Equilibrar fases** destrava as fases de todos os circuitos; o balanceamento em si é o automático que já
  existia.
- **Diâmetros internos** dos tipos que não são PVC rígido são valores típicos, sem conferência em catálogo,
  e não são editáveis nas Configurações.
- **Legenda:** a metragem por tipo e diâmetro está na lista de materiais. Ainda não existe uma legenda
  desenhada na planta.
- **Cabos de 0,6/1 kV:** usam a mesma coluna de diâmetro; LSZH segue a coluna da tensão dele.

## Especificação atualizada: traçados, dimensionamento, blocos e pacote

- **Traçado do eletroduto:** reto, ortogonal e arco, na mini-barra. Um clique no vazio vira um desvio do
  trajeto. O comprimento real e a ocupação valem para todos.
- **`ELEDIMENSIONAR`:** janela com as premissas e **Processar cálculo**, que grava em cada quadro o resultado
  de cada circuito (`DIM.<circuito>`) e atualiza tags e tabelas. O resumo do quadro de cargas mostra o que foi
  gravado e acusa **Desatualizado** quando o projeto muda depois. As edições não regeneram mais as tags
  sozinhas: isso substitui o gatilho automático pedido na primeira especificação.
- **Quadro de cargas em duas abas:** resumo e ficha de engenharia, que substituem a janela de detalhes.
- **PE compartilhado:** num trecho com vários circuitos, fica o PE de maior seção; a marca, a ocupação e a compra
  de cabo acompanham.
- **Blocos:** janela com **+ Criar novo bloco elétrico** e o **Inspetor de atributos** das inserções.
- **Pacote:** `cargo xtask dist` gera a pasta e o `.zip` em `dist/`, com executável, template, manuais e licenças.

Limites:

- **Ficha e inspeção:** mostram o cálculo de agora como prévia; a ficha avisa quando ele difere do gravado. A conta
  só roda com a janela ou a inspeção aberta, nunca ao clicar na planta.
- **Premissas:** PE compartilhado, FCA fixo, temperatura e os outros valores do Setup ficam nas preferências do
  programa, não no DXF. O resultado do cálculo fica no DXF.
- **Inspetor:** edita atributos de blocos inseridos. Os da definição do bloco se editam no editor de blocos.
- **Grupo DR:** organiza os circuitos e entra na lista de materiais, sem dimensionar o DR de cada grupo.
- **Método de instalação:** segue fixo em B1.
- **Pacote:** o executável não é assinado, e o Windows pode avisar na primeira vez.

## Paleta nativa, decisões por quadro e janela de dimensionamento

- **Paleta:** abas **Elétrica** e **Telecom**, seções recolhíveis (o estado fica junto das Tool Sets) e ferramentas planas com
  ícones desenhados. As janelas não têm barra de título: o cabeçalho é interno, e fecham no X dele ou com Esc.
- **Demanda:** classe e fator na família (`demand_class`, `demand_factor`) e no ponto (`TIPO_CARGA`, `FATOR_DEMANDA`). Um ponto
  com fator próprio sai da contagem da tabela. As formas com hachura saíram do editor de famílias.
- **Quadro:** sistema, tensão nominal, cabo padrão e alimentação na inserção do quadro. A tensão nominal escolhida vale sobre a
  dos pontos. A folga de cabos foi para a lista de materiais.
- **Tabelas de demanda:** de fábrica, só a tabela oficial da Equatorial (NT.00001.EQTL rev. 07). As outras concessionárias o
  projetista cadastra em **+ Cadastrar / Criar Nova Tabela de Demanda**, com faixas acumuladas ou com o fator da faixa sobre o total.
- **Cabos e eletrodutos:** catálogos com editar e excluir, inclusive os de fábrica. Perfis de eletroduto com tipo de linha, cor,
  caminhamento e diâmetros. O traçado em 45° substituiu o ortogonal.
- **Dimensionar projeto:** esquema, incluindo `2F+N+T` e `3F+T`, cabo e curva por circuito, grupos de DR à vontade em duas colunas,
  **Calcular Dimensionamento** sem gravar e **Aplicar ao Quadro**. O diagrama unifilar entra como bloco e acompanha o ELEREGEN.

Limites:

- **Presets:** só a Equatorial vem de fábrica; os presets da CEMIG e da Enel Goiás saíram. Um projeto que usava um deles passa
  para a Equatorial até o projetista cadastrar a tabela da concessionária dele. Sem tabela de motores, motores entram com 100 %.
- **Alimentado por:** é gravado, validado contra ciclos e mostrado no unifilar. A demanda de um quadro ainda não soma a dos quadros
  que ele alimenta.
- **Telecom:** o cabeamento é horizontal, do rack aos pontos; o backbone entre racks ainda não é calculado. As folgas de
  terminação (3 m no rack, 0,30 m no ponto) são prática de instalação, não números da norma.
- **Quadro VDI acoplado:** usa o lugar do quadro de cargas, e a aba dele continua com o nome do painel do CADCraft.
- **Curva:** a sugestão (B em carga só resistiva, C nas outras) é prática de projeto, não regra da NBR 5410. Fica a critério do projetista.
- **DR:** a corrente de cada grupo é uma sugestão pela soma de Ib, sem aplicar demanda.
- **Perfis:** mudar a linha ou a cor de um perfil vale para os próximos trechos; os desenhados guardam as suas. A tabela de diâmetros
  vale para todos os trechos do perfil no próximo cálculo.
- **Diagrama unifilar:** é um esquema simples do quadro, sem barras por fase e sem DPS desenhado.
- **Padrões internos:** `sistema` e `caboPadrao` continuam no Setup, só por comando, para os quadros sem escolha e os desenhos antigos.
- **`.AppImage`:** a regra de empacotar para Linux está em `CLAUDE.md`, mas o empacotamento ainda não existe.

## Correções e padronização dos desenhos

- **Destaque do circuito:** só existe com o Quadro de cargas aberto e usa um quadrado. Círculos e halos sobre o desenho são da inspeção.
- **Bloco em linha:** um bloco no caminho do eletroduto corta o trecho, sem caixa nova nem aviso de erro.
- **Excluir:** em blocos e famílias, pontos, circuitos, grupos DR, eletrodutos, perfis, cabos e tabelas de demanda.
- **Legenda e materiais:** `LEGENDA` e `DESC_MATERIAL` em toda família e todo ponto; `ELELEGENDA` desenha a legenda da prancha.
- **Famílias de fábrica:** tomadas e interruptores de um módulo cada, com o preenchimento dizendo a altura ou a função.
  Duplas e várias teclas são módulos empilhados numa caixa 4x2 só; nos materiais, uma caixa por altura.
- **Eletrodutos:** `LINE`, `ARC` ou `LWPOLYLINE`, nunca `SPLINE`, e um trecho novo não mexe nos outros.
- **Unifilar:** marcas de fiação iguais às da prancha e disjuntor com terminais, arco e traço de disparo.
- **Eletrodutos:** a cor é do material e o tipo de linha é do caminhamento.
- **Tag de fiação:** direto no eletroduto (um clique) ou com linha de chamada em ângulo livre (dois cliques), guardada no XDATA.
- **Tabela do quadro:** tudo dentro de uma moldura, com cabeçalho, grade, total e rodapé dividido.
- **Unifilar:** entrada com fase, neutro e terra, disjuntores curvos, barra com nós, DR com sub-barra e reservas.

Limites desta etapa:

- **Tolerância em linha:** sem clique, o bloco precisa estar a até 5 cm do traçado. Numa planta, um eletroduto do teto que passe
  exatamente sobre uma tomada baixa também é cortado nela: confira esses cruzamentos.
- **`TOM-TUE`:** continua com o triângulo cheio, o mesmo da tomada alta. Diferencie pela legenda ou redesenhe o bloco.
- **Módulos:** a estimativa não conta o DPS.
- **Blocos excluídos:** a marca vive nos dados do desenho e não foi testada depois de salvar e reabrir o DXF.
- **Template:** `templates/FREE_ELETRICA_BLOCOS.dxf` ainda traz os símbolos antigos; só os blocos gerados pelo motor seguem o novo padrão.

Limites dos módulos empilhados:

- **Comprimento:** o módulo empilhado usa o caminho até a caixa base; a diferença de altura entre os dois (0,30 m e 1,20 m, por
  exemplo) não entra no comprimento do cabo.
- **Base excluída:** os módulos que estavam nela voltam a ser pontos soltos, sem a caixa desenhada. Empilhe de novo ou reinsira.
- **Blocos antigos:** `TOM-DUP-*` e os interruptores de 2 e 3 seções saem do desenho na primeira ferramenta usada nele. Cada
  inserção vira os módulos de agora, empilhados no mesmo lugar: o primeiro fica com os atributos, o XDATA e a camada; os outros,
  com o mesmo circuito, uma letra de comando cada e a potência da família.

Ajustes de interface e de prancha:

- **Quadro de cargas:** sem abas; as ações de cada circuito ficam no menu "..." da linha, e os dados do quadro num grupo recolhível.
- **Tabela no desenho:** o sub-cabeçalho é uma caixa só, sem linhas internas, com os dados em 3 colunas.
- **Unifilar:** tudo na cor do diagrama, um risco por polo no disjuntor, e a moldura tracejada do quadro ligada ao terra (PE).

## Ferramentas, quadro acoplado e pontos de luz da NBR 5444

- **Paleta nas Tool Sets:** a coluna avulsa da direita saiu. A paleta é a aba **Installations** (Instalações) das Tool Sets, ao
  lado de Drafting e Modeling, com o seletor **Elétrica | Telecom** no topo. `ELETRICA` abre essa aba e alarga as Tool Sets para
  290 px uma vez; um workspace salvo com a coluna antiga a perde ao abrir.
- **Quadro de cargas acoplado:** aberto, ele entra no layout embaixo do desenho, entre as Tool Sets e as Camadas, com 330 px e a
  borda redimensionável; o X do canto, Esc ou o menu do painel o fecham e ele sai do layout. Da segunda vez, volta no tamanho em
  que ficou.
- **Ponto de luz no teto (`ILU-OCT`):** círculo de traço normal com diâmetro horizontal e o raio que divide a metade de baixo; a
  potência em cima, o comando e o circuito (`-1-`) embaixo. O anel cheio das versões anteriores é trocado ao preparar o desenho.
- **Arandela (`ILU-PAR`):** o D com a face reta na parede e a divisória até a borda; comando e circuito dentro, potência nos
  dados do ponto; ponto alto (2,10 m).
- **Textos que cabem:** cada texto do ponto de luz fica centralizado na sua divisão, com a altura que cabe (até 3 cm). Inserir,
  editar um atributo e `ELEREGEN` recolocam os textos.

## Famílias próprias empilhadas, borda dos blocos e esquemas paralelos

- **Empilhar qualquer família:** a caixa e a haste marcadas com `ROLE = BASE_BOX_CONNECTOR` somem no módulo empilhado, em
  famílias de fábrica ou próprias. O editor de blocos insere **Caixa 4x2**, **Caixa 4x4** e **Haste / conector padrão** já
  marcadas. O bloco empilhado é refeito a cada empilhamento, então redesenhar a família vale para os próximos módulos.
- **Caixa 4x4 em planta:** a mesma profundidade da 4x2 e o dobro da largura.
- **Busca no Inserir pontos:** **Pesquisar ponto...** filtra a lista pelo nome ou pela legenda, sem acentos; a lista mostra
  potência e altura padrão. Uma busca que deixa um bloco só já o escolhe.
- **Ponto de luz:** o circuito aparece só com o número (`1`); um `-1-` antigo volta ao número ao regenerar.
- **`CX-OCT`:** caixa de passagem octogonal de teto, sem potência nem comando.
- **Paralelos e intermediários:** `elecomando` aceita `interruptores: [...]`, e o interativo aceita clicar nos outros
  interruptores do comando. Cada um grava `linked_switch_handles.<letra>` e `switch_role.<letra>` (`ParallelStart`,
  `Intermediate`, `ParallelEnd`, ou `Simple` quando é um só); as luminárias gravam a lista. A fiação segue essa ordem: fase
  até o primeiro, `a'` e `a"` entre eles, `a` do último às luminárias.
- **Eletroduto na borda:** a linha, o arco ou a polilinha param no contorno do bloco (o círculo de um símbolo redondo, o
  retângulo dos outros), na inserção, ao dividir, ao encaixar e no `ELEREGEN`. O vínculo segue no XDATA, o grafo aceita a ponta
  a até o raio de captura além do contorno do bloco gravado, e o comprimento soma o pedaço até o centro de cada bloco. Um
  bloco antigo trocado por módulos passa os eletrodutos gravados nele ao primeiro módulo.

## Escala 1:50, unifilar com simbologia e corte só nas luminárias

- **Tabelas em 1 un = 1 m:** a tabela do quadro, a legenda e o unifilar usam texto de 9 cm (8 cm nos menores), títulos de 15 cm e
  linhas de 26 cm (28 cm na legenda).
- **Legenda:** cada símbolo fica com o centro do desenho dele no meio da célula da esquerda, reduzido se não couber. Os módulos
  empilhados (`<bloco>$EMP`) entram com o bloco e a família de origem, na legenda e na lista de materiais.
- **Tipos de linha dos eletrodutos:** `ELE-TRACEJADA` (8 cm de traço, 4 cm de espaço), `ELE-TRACO-PONTO` (8 cm, 3 cm, 1,5 cm, 3 cm)
  e `ELE-PONTILHADA` (pontos a cada 4 cm), em metros. O `ELEREGEN` troca os `DASHED`, `DASHDOT` e `DOT` das versões anteriores,
  nos eletrodutos e nas camadas.
- **Unifilar:** os riscos de polo do disjuntor são verticais, e o diagrama traz embaixo o quadro **Simbologia do diagrama
  unifilar**: disjuntores 1P, 2P e 3P, IDR, DPS, condutores fase, neutro e proteção, barramento e aterramento.
- **Corte na borda:** só nos pontos de luz (luminárias de teto e arandelas). Nos outros blocos o eletroduto vai até o ponto de
  inserção, e o `ELEREGEN` devolve a ele as pontas que a versão anterior cortou.

## Vista frontal do quadro

- **`ELEQUADRO_FRONTAL`** (`eletrica.quadro.frontal`), em **Documentação e Tabelas**, no **Dimensionar projeto** e no quadro de
  cargas: desenha o layout de montagem num bloco `QUADRO_FRONTAL_<handle>`, refeito pelo `ELEREGEN`.
- **Montagem:** caixa e chassi em borda dupla, título `VISTA FRONTAL / LAYOUT DE MONTAGEM - <quadro>`, barramento de neutro
  (azul) em cima e de terra (verde) embaixo, com um borne por circuito mais o do alimentador.
- **Trilhos DIN:** o geral com os polos do quadro, um DPS classe II por fase, cada DR (2 módulos, ou 4 com circuito multipolar)
  seguido dos circuitos dele, os circuitos sem DR e as reservas tracejadas. Até 12 módulos, um trilho de 12; acima disso, trilhos
  de 18, sem partir um dispositivo.
- **Escala:** é um detalhe ampliado: um módulo DIN vale 25 cm no desenho, para os textos de 8 cm caberem nos dispositivos.
- **Unifilar:** o DPS aparece na entrada, depois do geral, ligado ao terra.

## Telecom (NBR 14565) e disciplinas

- **Vincular e numerar (`TELEVINCULAR`):** `RACK_PAI`, `ID_PONTO` (`D-01`, `CFTV-01`, `AP-01`, `FO-01`, `TV-01`, pelo nome do bloco,
  da legenda ou do cabo), `TIPO_CABO` e `QTD_CABOS`. Com um rack só, todos vão para ele; com mais, o rack escolhido ou o mais perto
  pela rede. A numeração segue a distância até o rack.
- **Estrela (`TELEROTEAR`):** o menor caminho de cada ponto até o rack, passando só por pontos de dados e caixas de passagem; um
  módulo empilhado sai da caixa base. Cada ponto leva os cabos dele inteiros: um trecho por onde passam três pontos leva três cabos.
- **Comprimento 3D:** o caminho em planta, as subidas e descidas até os blocos, o desnível do módulo empilhado e as folgas. Aviso
  acima de 90 m num cabo UTP.
- **Eletroduto:** o menor do tipo ou do perfil com até 40 % de ocupação; um diâmetro travado só tem a ocupação conferida. Um
  eletroduto com circuitos de energia e cabos de telecom gera aviso.
- **Tags (`TELEFIACAO`):** direto no eletroduto ou com linha de chamada, como as de fiação: os pontos em cima (`D-01..D-04`), a
  quantidade, o cabo e o diâmetro embaixo (`4x Cat6 Ø25`).
- **Quadro VDI / Rack:** porta, ponto, descrição, cabo, comprimento 3D e situação, acoplado embaixo do desenho; **Tabela do rack**
  (`TELETABELA`) no desenho; cabos de telecom, terminações e portas na lista de materiais. O `ELEREGEN` refaz tags e tabelas.
- **Disciplinas (`disciplina.rs`):** Elétrica (radial), Telecom (estrela) e Alarme de incêndio (NBR 17240, laço classe A ou B)
  dizem qual é a central, quais pontos são de cada uma e por quais blocos um cabo passa. O alarme fica preparado, sem blocos nem
  interface ainda.

## Auditoria técnica: proteção geral, métodos de instalação, DR, quadro DIN, disciplinas e materiais

- **Proteção geral e DPS por quadro:** disjuntor geral e seção do alimentador travados à mão (`DISJ_GERAL`, `SECAO_GERAL`), conferidos
  contra Ib ≤ In ≤ Iz sem impedir; DPS opcional (`USAR_DPS`), classe I, II ou I+II (`CLASSE_DPS`) e disjuntor exclusivo antes dele
  (`DISJ_DPS`). O unifilar e a vista frontal seguem: sem DPS, nenhum; com o disjuntor dele, em série antes.
- **Critério do disjuntor:** o menor acima de Ib (padrão) ou o maior até o Iz do cabo escolhido, nas Configurações e no
  Dimensionar projeto.
- **Métodos de instalação:** A1, A2, B1, B2, C e D (tabelas 36 e 37) e E e F (tabelas 38 e 39), no quadro (`METODO_QUADRO`), por
  circuito (`METODO.<circuito>`) e nas Configurações. No F as capacidades começam em 25 mm²; no D o FCT do solo e o FCA da tabela 44
  ainda não são aplicados (aviso no quadro).
- **Neutro e PE:** PE pela tabela 58 e neutro reduzido pela tabela 48 (só em 3F+N+T), na tag (`#50 N25 T25`), no diâmetro do
  eletroduto, no PE compartilhado e nos materiais.
- **Grupos de DR:** **+ Adicionar circuito...** em cada grupo, **Adicionar ao DR ▾** em cada circuito sem DR e a coluna **Grupo DR**
  na tabela do Dimensionar projeto.
- **Vista frontal:** trilho de 12, 16, 18 ou 24 módulos (`MODULOS_TRILHO`), barramento pente ou espinha de peixe
  (`BARRAMENTO_FASE`), neutro e terra em cima e embaixo ou nas laterais (`BARRAMENTO_NT`).
- **Disciplinas:** legenda elétrica (`ELELEGENDA`, `LEGENDA_SIMBOLOS_ELETRICA`) e de telecom (`TELELEGENDA`,
  `LEGENDA_SIMBOLOS_TELECOM`); `TELEFIACAO` com a mini-barra das tags; cabo e quantidade de cada ponto no quadro VDI
  (`eletrica.telecom.ponto`).
- **Materiais:** disjuntores por polos, corrente e curva (inclusive o geral e o do DPS); DR, DPS, barramentos de neutro, terra e
  das fases, e a caixa com a capacidade em módulos DIN.

## Telecom no padrão do quadro de cargas e seleção múltipla

- **Tags de cabeamento:** só a quantidade e o tipo de cabo em cima (`3x UTP Cat 6`, `2x UTP Cat 6 + 1x Fibra N2`) e o
  diâmetro embaixo (`Ø25`), sem os números dos pontos; os dois modos da mini-barra, como na elétrica.
- **Quadro de Telecomunicações:** é o título da aba e do painel acoplado com o rack aberto. A coluna **Porta / Ponto** junta a
  porta e o número do ponto: `TELEVINCULAR` numera os pontos pela porta (1, 2, 3...; um ponto de dois cabos ocupa duas portas e
  leva o número da primeira). No painel: o rack em **Rack / Quadro:**, **+ Nova porta / ponto**, a descrição editável na linha
  (`DESC_PORTA`), o cabo e a quantidade, e o menu **...** com o número do ponto, vincular, desvincular (`RACK_PAI = -`), localizar
  e excluir. Os pontos sem rack aparecem no fim, para vincular.
- **Seleção múltipla:** clique, Ctrl+clique, Shift+clique e Ctrl+A no Dimensionar projeto, no quadro de cargas, no Quadro de
  Telecomunicações e nas listas de pontos e de eletrodutos. Com duas ou mais linhas selecionadas, uma mudança num campo de uma
  delas (esquema, cabo, método, grupo DR, curva, fases, seção e disjuntor, cabo e quantidade de telecom, altura, tipo, cota e
  diâmetro de eletroduto) vale para todas.
- Identificações de versões anteriores (`D-01`) continuam valendo; **Numerar e vincular** com renumerar passa todas para porta.

## Tensão nos blocos e atributos padrão das famílias

- **Atributos padrão:** toda família nova nasce com `CIRCUITO`, `POTENCIA`, `TENSAO`, `FP`, `COMANDO`, `ALTURA`, `LEGENDA` e
  `DESC_MATERIAL` conforme a categoria. **Gerar Atributos Padrão** só acrescenta os que faltam, sem duplicar.
- **Tensão Padrão (V)** no painel da família: 127, 220, 380 ou **Definida pelo Quadro** (`voltage_v` no `BLOCK_RECORD` e o
  valor padrão do ATTDEF `TENSAO`). Cada ponto inserido herda a tensão da família.
- **Tensão** no inspetor de um ponto selecionado grava o `TENSAO` dele (`eletrica.ponto.tensao`); vazio volta à família ou ao quadro.
- **Sem travar:** um `TENSAO` vazio ou ilegível cai na tensão da família e, sem ela, na do quadro (ou do Setup), com aviso.
