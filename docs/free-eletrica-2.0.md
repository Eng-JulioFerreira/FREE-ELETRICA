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
