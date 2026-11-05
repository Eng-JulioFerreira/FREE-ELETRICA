# Documentação técnica do Free Elétrica

A arquitetura implementada. Para desenvolvedores e para quem automatiza o programa. O pedido original está em
[ESPECIFICACAO.md](ESPECIFICACAO.md), e o histórico de decisões e limites em [free-eletrica-2.0.md](free-eletrica-2.0.md).

## Camadas

| Onde | Camada | O quê |
| :--- | :--- | :--- |
| `cadcraft/crates/free-eletrica` | L2 | O motor: pontos, famílias, circuitos, grafo de eletrodutos, NBR 5410, tags, tabela, materiais. Depende só de `cadcraft-doc`. |
| `cadcraft/crates/engine/src/cmd/eletrica.rs` | L4 | Os comandos, com forma JSON e prompt interativo. |
| `cadcraft/crates/ui-egui/src/eletrica.rs` | L5 | A interface: só lê o estado e chama comandos. |

Os pontos de registro no código do CADCraft estão em [arquitetura.md](arquitetura.md).

## Módulos do motor

| Módulo | Responsabilidade |
| :--- | :--- |
| `ponto` | Lê blocos inseridos como pontos elétricos, com os atributos e o XDATA |
| `familia` | `SystemDomain`, `FamilyCategory`, `RefAltura`, `CaixaFisica`, gravados no `BLOCK_RECORD` |
| `biblioteca` | Blocos base, inserção, modelo do ELEPONTO, alinhamento à parede, utilitários do editor de blocos |
| `circuito`, `editar` | Agrupamento em circuitos e todas as escritas no desenho, incluindo `processar_calculo` |
| `rota` | O grafo estrito 3D, `TipoEletroduto`, Dijkstra e a estimativa por Manhattan 3D |
| `eletroduto` | Traçados, caixa de passagem ao dividir um trecho, modelo do ELEELETRODUTO |
| `nbr5410`, `quadro`, `demanda` | Dimensionamento, `TipoCabo`, `Esquema`, `Curva`, configuração de cada quadro, grupos de DR, alimentador, presets e tabelas de demanda |
| `fiacao`, `tabela`, `unifilar`, `materiais`, `inspecao` | Tags, tabela e diagrama unifilar no desenho, lista de materiais e inspeção visual |
| `setup` | As configurações, nas preferências do programa: catálogos de cabos, perfis de eletroduto e tabelas de demanda próprias |

## O grafo de eletrodutos

- **Nós:** só blocos elétricos. **Arestas:** linhas, polilinhas abertas, arcos e splines abertas nas camadas de eletroduto.
- Cada ponta vale pelo handle gravado (`from_handle`, `to_handle`), se o bloco está no raio de captura; senão, pelo bloco
  mais perto no raio. Sem bloco, o eletroduto fica solto e é apontado.
- Cruzamento em X, derivação em T e bloco encostado no meio da linha não conectam.
- Comprimento da aresta = comprimento do traçado em planta + |cota - Z de A| + |cota - Z de B|. A cota é o pé-direito no
  teto, 0 no piso e o Z do bloco A na parede.
- Sem caminho até o quadro, o comprimento é estimado por Manhattan 3D e marcado com `~`.

## Separação de etapas

Lançar pontos, circuitos e eletrodutos só grava os dados do projeto. `eletrica.dimensionar` roteia, dimensiona, grava o
resultado de cada circuito em `DIM.<circuito>` e atualiza tags, tabelas e diagramas. Depois disso, um circuito cujo cálculo atual difere
do gravado aparece como desatualizado. A ficha e a inspeção mostram o cálculo atual como prévia, só com a janela ou a
inspeção abertas.

## Dados no DXF

Aplicativo do XDATA: `FREE_ELETRICA`, com pares `chave=valor` em grupos 1000. Os nomes antigos (`FREEELETRICA`, `CRAFTELE`)
continuam sendo lidos.

| Onde | Chaves |
| :--- | :--- |
| `BLOCK_RECORD` | `schema_ver`, `domain`, `category`, `height_ref`, `fixed_elevation_m`, `physical_box`, `power_va`, `power_factor`, `keys`, `demand_class`, `demand_factor` |
| Ponto (`INSERT`) | Atributos do bloco, incluindo `TIPO_CARGA` e `FATOR_DEMANDA`; `elevation_m` fora da legenda; `target_handles.<letra>` no interruptor |
| Quadro (`INSERT`) | `SISTEMA_QUADRO`, `TENSAO_NOMINAL`, `CABO_PADRAO_QUADRO`, `ALIMENTADO_POR`; `DESC.`, `SECAO.`, `DISJ.`, `CURVA.`, `CABO.`, `CABONOME.`, `ESQUEMA.`, `TENSAO.`, `FASE.`, `DR.`, `DIM.` + circuito |
| Eletroduto | `schema_ver`, `from_handle`, `to_handle`, `install_elevation`, `conduit_type`, `conduit_profile`, `nominal_diameter_mm`, `manual_diameter_lock`, `passing_wires.<n>`, `tag_handle` |
| Tag de fiação | `schema_ver`, `parent_conduit_handle`, `anchor_x`, `anchor_y`, `user_offset_x`, `user_offset_y` |

O tipo de linha e a cor do perfil ficam na própria entidade do eletroduto. O diagrama unifilar é o bloco
`UNIFILAR_<handle do quadro>`, e a tabela, `QUADRO_TABELA_<handle do quadro>`.

As configurações do Setup, incluindo PE compartilhado, FCA fixo, catálogos, perfis e tabelas de demanda próprias, ficam nas
preferências do programa, não no DXF.

## Comandos

Todos respondem em JSON pelo canal de controle, pelo `cadcraft-cli` e pelo MCP. Os parâmetros exatos de cada um estão no
próprio `CommandSpec`.

| Grupo | Comandos |
| :--- | :--- |
| Pontos | `eleponto`, `eletrica.ponto.modelo`, `eletrica.ponto.altura`, `eletrica.ponto.demanda`, `eletrica.ponto.excluir`, `eletrica.blocos`, `eletrica.bootstrap` |
| Famílias | `eletrica.familia.ler`, `.definir`, `.geometria`, `.atributos`, `elefamilia_conector` |
| Atributos | `eletrica.atributos`, `eletrica.atributo.definir` |
| Circuitos | `elecircuito`, `eleremoverponto`, `elecomando`, `eletrica.circuito.criar`, `.definir`, `.descrever`, `.juntar`, `.excluir`, `.alvo` |
| Eletrodutos | `eleeletroduto`, `eletrica.eletroduto.modelo`, `.dividir`, `.definir`, `eletrica.eletrodutos` |
| Cálculo | `eletrica.dimensionar`, `eletrica.rotear`, `eletrica.sobrescrever`, `eletrica.quadro`, `eletrica.quadro.equilibrar`, `eletrica.cargas`, `eletrica.demanda.tabelas`, `.salvar`, `.excluir`, `eletrica.setup` |
| Documentação | `elefiacao`, `eleregen`, `elequadro_tabela`, `eletrica.unifilar`, `eletrica.materiais`, `eletrica.inspecionar`, `eletrica.analisar` |

Comandos que só existem na interface, porque abrem janelas: `ELETRICA`, `ELECONFIG`, `ELEPONTOS`, `ELEBLOCOS`, `ELEATRIBUTOS`,
`ELECARGAS`, `ELEROTEAR`, `ELEDIMENSIONAR`, `ELEMATERIAIS`, `ELECONFERIR`, `ELEINSPECAO`.

## Compilar e empacotar

O ambiente de compilação e os portões de qualidade estão em [CLAUDE.md](../CLAUDE.md).

| Comando | O quê |
| :--- | :--- |
| `scripts\package_release.ps1` | Compila em release e gera `dist/FreeEletrica/` e `dist/FreeEletrica-v<versão>-win64.zip` |
| `cargo xtask dist --versao 0.2.0` | O mesmo, dentro de `cadcraft/`, com o ambiente carregado |
| Workflow `Release` (`.github/workflows/release.yml`) | Faz o mesmo no GitHub Actions e publica na aba Releases; é disparado à mão |
