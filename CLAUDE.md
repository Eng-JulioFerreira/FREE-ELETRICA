# Free Elétrica — instruções para agentes

Free Elétrica é um addon de projeto elétrico predial de baixa tensão (NBR 5410) e telecom para o
[CADCraft](https://github.com/storytold/cadcraft), em Rust. Ele vive dentro de uma cópia do
CADCraft (`cadcraft/`, um git subtree do repositório original) e aparece como a paleta
**Elétrica**, acoplada como os painéis do próprio CADCraft.

As premissas do CADCraft valem aqui. Ao mexer em `cadcraft/`, leia também
`cadcraft/CLAUDE.md`: as regras dele (nunca travar, tudo é comando, interface fina, camadas,
clean-room, assets) são obrigatórias. As exceções estão no fim deste arquivo.

## POLÍTICA DE GIT E CONTROLE DE VERSÃO (COMMITS LOCAIS VS. RELEASES NO GITHUB)

**Nenhum `git push`, Release, PR ou qualquer outra mudança no GitHub sem a autorização explícita do usuário.** O projeto é
divulgado para engenheiros e usuários finais: código em teste ou incompleto não sai desta máquina.

1. **Commits locais são livres.** Faça `git add` e `git commit` ao concluir e validar uma etapa que compila e passa nos
   portões de qualidade. Isso mantém o histórico local seguro para voltar atrás.
2. **O remoto é proibido por iniciativa própria.** Isso inclui `git push`, `gh release create`, `gh pr create`, comentários
   em issues, disparo de workflows, renomear, arquivar ou mudar a visibilidade de repositórios, por `gh` ou pela API. Vale
   para qualquer repositório do GitHub, e nenhuma aprovação anterior se estende a uma próxima vez.
3. **O gatilho é a frase do usuário**, depois de ele testar o software localmente: **"Publicar nova versão [vX.Y.Z] no GitHub"**.
   Ler o remoto (`git fetch`, consultar uma issue) continua permitido.

### Protocolo de lançamento (só com o gatilho acima)

1. Conferir que o workspace compila em release (`cargo build --release`), sem avisos críticos, e que os testes passam
   (`scripts\testar.bat tudo`).
2. Atualizar a versão e registrar as novidades em `CHANGELOG.md`, e em `README.md` e `docs/` se a interface mudou. A versão do
   Free Elétrica é `VERSAO`, em `cadcraft/xtask/src/dist.rs`, e o `Cargo.toml` de `crates/free-eletrica`; a versão do workspace
   é a do CADCraft e não muda.
3. Criar o commit de release local e a tag (`v0.2.0`, `v0.3.0`...).
4. Gerar os pacotes da versão: o `.zip` para Windows e o `.AppImage` para Linux. `scripts\package_release.ps1` gera
   `dist/FreeEletrica/` (executável, `templates/`, manuais e licenças, sem `.bat`) e `dist/FreeEletrica-vX.Y.Z-win64.zip`.
   O empacotamento do `.AppImage` ainda não existe no projeto: precisa ser criado e testado em Linux antes da primeira
   Release que o inclua. **Os dois pacotes só são gerados quando o usuário autorizar uma release com o gatilho acima.**
5. Fazer o `git push` para a branch principal do GitHub (`free-eletrica`) e publicar a Release com os pacotes (`gh release create`).

## Arquitetura do Free Elétrica 2.0

- **Motor headless NBR 5410:** cálculo, roteamento e escrita no desenho ficam em `crates/free-eletrica`, sem interface. Tudo é
  comando, alcançável por linha de comando, JSON, `cadcraft-cli` e MCP.
- **Persistência no desenho:** atributos dos blocos e XDATA `FREE_ELETRICA`, incluindo famílias no `BLOCK_RECORD` e o
  resultado do último cálculo nos quadros. As configurações do Setup ficam nas preferências do programa.
- **Grafo 3D estrito de eletrodutos:** nós são só blocos elétricos, e cada eletroduto liga exatamente dois deles.
  Cruzamento sem bloco não conecta. Cada trecho é uma entidade só (`LINE`, `ARC` ou `LWPOLYLINE`, nunca `SPLINE`), e inserir
  um trecho não mexe em nenhum outro. O traçado para na borda só dos pontos de luz (`borda.rs`); nos outros blocos vai até o ponto de inserção. Um bloco a até 5 cm do traçado já é uma caixa: `eletroduto::encaixar` corta o trecho
  nele, sem deformar. Um módulo empilhado (`host_point_handle`) não é nó: o caminho até ele é o da caixa base. Comprimento =
  traçado + desníveis até os blocos.
- **Etapas separadas:** lançar não dimensiona. O cálculo, as tags e as tabelas só mudam com `ELEDIMENSIONAR` e `ELEREGEN`.
- **Interface sem coluna nem painel fixos a mais:** a paleta é a aba Installations das Tool Sets, à esquerda (seletor Elétrica |
  Telecom no topo, seções recolhíveis, ferramentas planas com ícone); o quadro de cargas, quando aberto, fica acoplado embaixo do
  desenho, entre as Tool Sets e as Camadas, e sai do layout ao fechar (X ou Esc); mini-barra só enquanto um comando espera cliques, janelas sem
  barra de título abertas sob demanda (X do cabeçalho interno ou Esc) e sobreposição de inspeção.
- **Decisões por quadro e por circuito:** sistema, tensão nominal, cabo padrão e alimentação ficam em cada quadro; esquema,
  cabo, curva e grupo DR em cada circuito; classe e fator de demanda na família e no ponto. As Configurações guardam o que vale
  para o projeto todo: critérios, catálogos de cabos, perfis de eletroduto e tabelas de demanda.
- **Disciplinas no mesmo grafo:** `ele::disciplina` diz a central, os pontos e por onde passa o cabo de cada uma. Telecom
  (`ele::telecom`, NBR 14565) é em estrela, um cabo inteiro de cada ponto até o rack; o alarme de incêndio (laço) fica preparado.
- **Eletroduto no desenho:** a cor é do material e o tipo de linha é do caminhamento (teto contínua, parede tracejada ou
  pontilhada, piso traço e ponto).
- **Toda janela que cria algo tem Excluir:** blocos e famílias, pontos, circuitos, grupos DR, eletrodutos, perfis, cabos e
  tabelas de demanda, inclusive os de fábrica, que têm como restaurar.

Os detalhes estão em `docs/SPECIFICATION.md`.

## Mapa

| Onde | O quê |
| :--- | :--- |
| `cadcraft/crates/free-eletrica/` | **Motor do Free Elétrica**: pontos, circuitos, NBR 5410, roteamento, fiação, setup. Sem interface. |
| `cadcraft/crates/engine/src/cmd/eletrica.rs` | **Comandos** do motor: `elecircuito`, `elecomando`, `elefiacao`, `eletrica.*`. |
| `cadcraft/crates/ui-egui/src/eletrica.rs` | **Interface**: a paleta Elétrica, a mini-barra, as janelas e a sobreposição. Só lê e chama comandos. |
| `cadcraft/crates/free-eletrica/src/tests.rs` | Testes do motor. |
| `cadcraft/crates/engine/tests/eletrica_comandos.rs` | Testes dos comandos, sem interface. |
| `cadcraft/crates/ui-egui/tests/eletrica_painel.rs` | Testes da paleta, da mini-barra e das janelas. |
| `docs/` | Documentação técnica (`SPECIFICATION.md`), padrão de blocos, arquitetura do fork, especificação pedida e registro da versão 2.0. |
| `templates/` | Template opcional de blocos e camadas (`FREE_ELETRICA_BLOCOS.dxf`) e a fonte dos símbolos. Os blocos base nascem no motor (`ele::biblioteca`). |
| `docs/LEIA-ME.md`, `docs/INSTALL.md`, `docs/GETTING_STARTED.md` | Manuais de quem só usa; vão no pacote de `cargo xtask dist`. |
| `.github/workflows/release.yml` | Gera o pacote no GitHub Actions; é disparado à mão, e publicar segue a política acima. |
| `scripts/` | `compilar.bat`, `iniciar.bat`, `testar.bat`, `package_release.ps1`, `gerar_template.py`. |

Pontos de registro no código do CADCraft (as únicas linhas nossas fora dos nossos arquivos)
estão listados em `docs/arquitetura.md`. Qualquer mudança nova num arquivo do CADCraft entra
nessa lista e precisa ser pequena.

## Regras inegociáveis

1. **Test-driven.** Toda função nova nasce com teste; todo bug corrigido ganha um teste que
   falhava antes. Escreva o teste primeiro sempre que der. **Nenhum commit com teste falhando**,
   nem do CADCraft. Nunca apague, enfraqueça ou ignore um teste para passar: conserte o código,
   ou explique ao usuário por que o teste estava errado antes de mudá-lo.
2. **Motor desacoplado da interface.** Toda regra elétrica, cálculo, roteamento e escrita no
   desenho fica em `crates/free-eletrica` (camada L2, só depende de `cadcraft-doc`; nunca de egui).
   A interface não decide nada de engenharia.
3. **Tudo é comando.** Toda capacidade do motor é um `CommandSpec` em
   `crates/engine/src/cmd/eletrica.rs`, com a forma JSON (`run`) e, quando pede seleção no
   desenho, o prompt interativo (`interactive`). Cada comando tem teste em
   `eletrica_comandos.rs` rodando **sem interface**. Assim a linha de comando, scripts, o canal de
   controle, o `cadcraft-cli` e o MCP alcançam tudo. Chamadas programáticas nunca abrem janela.
4. **Interface fina.** A paleta, a mini-barra e as janelas leem o estado e agem por `app.run(id, params)` /
   `app.start(id)`. Comandos só de interface (abrir paleta e janelas) ficam em `route()` de
   `ui-egui/src/eletrica.rs`. A paleta só tem ferramentas; dados aparecem em janelas independentes (o quadro de
   cargas, acoplado embaixo), abertas só quando pedidas, que fecham no X ou com Esc. Inserir fica na mini-barra.
5. **Nunca travar** (padrão do CADCraft): sem `unwrap`/`expect`/`panic!`/`todo!` fora de testes
   (`#![deny]` na raiz de `ele`), erros por `Result`, entrada do desenho e de parâmetros JSON é
   hostil (`get()`, nada de índice vindo de fora, NaN e infinitos tratados).
6. **Dados no desenho.** Valores vão para o atributo do bloco quando ele existe e, senão, para o
   XDATA `FREE_ELETRICA` (`ele::editar`, `ele::xdata`). O que é gerado (tags de fiação, tabelas) leva XDATA
   próprio para poder ser apagado e refeito.
7. **Clean-room.** Programas de referência (OFE Elétrico, PRO-Elétrica, Lumine, AutoCAD) podem
   ser **observados** em uso; nunca descompilados, nunca copiados (código, símbolos, tabelas
   próprias, textos). Regras de cálculo vêm da NBR 5410 e são citadas no código (tabela, item).
   Fatores de demanda só entram como preset depois de lidos na norma pública da concessionária, com a
   fonte em `fonte`; o que não puder ser conferido não vira número no código, e o editor de tabelas cobre.
8. **Licença.** O código do Free Elétrica é GPL-3.0-only: todo arquivo novo nosso começa com
   `// SPDX-License-Identifier: GPL-3.0-only`. Os arquivos do CADCraft continuam MIT OR
   Apache-2.0; não mude a licença deles nem cole código GPL dentro deles além das linhas de
   registro.
9. **Idioma.** Textos para o usuário em português do Brasil. Ids e rótulos de `CommandSpec` e de
   menu seguem o CADCraft (rótulo em inglês, com linha em `i18n/uk.tsv`, obrigatória, e em
   `i18n/pt_br.tsv`).

## Portões de qualidade (antes de todo commit)

`scripts\testar.bat` roda: `cargo fmt --check`, `cargo clippy ... -D warnings`, os testes e
`cargo xtask layers`. Periodicamente, e antes de publicar, `scripts\testar.bat tudo` (workspace
inteiro). Commits atômicos em Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`,
`refactor:`), um por arco de funcionalidade que compila e passa. Commits são locais: veja a política de Git no início.

## Compilar, rodar e olhar o resultado

- Testes de uma parte: `cargo test -p free-eletrica`, `cargo test -p cadcraft-engine --test eletrica_comandos` e
  `cargo test -p cadcraft-ui-egui --test eletrica_painel`, em `cadcraft/`, com o ambiente carregado.

- Ambiente: `scripts\ambiente.cmd`. O toolchain Rust (GNU), o MinGW e a saída (`C:\rust\target`)
  ficam em `C:\rust` porque as ferramentas GNU não aceitam o "ú" da pasta do usuário. A saída é
  compilada sem informação de depuração (com ela os testes passam de 100 GB).
- `scripts\compilar.bat [release]`, `scripts\iniciar.bat` (porta de controle 47800).
- Para olhar a interface, use outra porta e outra pasta de configuração
  (`CADCRAFT_CONFIG_DIR`) e dirija pelo canal de controle (`cadcraft/docs/control-protocol.md`):
  `ui.screenshot` e leia o PNG. **Cuidado:** a pasta de autosave é compartilhada entre as
  instâncias. Nunca clique na janela "Drawing Recovery" de uma instância de teste (Discard apaga
  o autosave do usuário); prefira os testes sem interface e o `cadcraft-cli`.
- Não compile por cima do executável que o usuário está usando (o Windows bloqueia o arquivo):
  veja qual está aberto e use o outro perfil (release ou desenvolvimento).

## Repositórios no GitHub

- `origin` é `Eng-JulioFerreira/Free-Eletrica`, o fork público de `storytold/cadcraft`. A branch padrão é `free-eletrica`, com a
  árvore deste repositório (`cadcraft/`, `docs/`, `templates/`, `scripts/`). O `main` do fork ficou parado na publicação antiga.
- `legacy` é `Eng-JulioFerreira/FREE-ELETRICA-legacy-archive`, privado e arquivado, só para consulta e sem push. Ele guarda a
  versão 1: o `main`, o branch `versao-1` e a tag `v1-antes-da-2.0`, também presentes aqui.

Desenvolva em `free-eletrica`. O GitHub não diferencia maiúsculas em nomes: uma URL `.../FREE-ELETRICA.git` aponta para o
`Free-Eletrica` público, nunca para o arquivo antigo. Publicar segue a política de Git do início deste arquivo.

## Atualizar o CADCraft

```
git subtree pull --prefix cadcraft https://github.com/storytold/cadcraft.git main --squash
```

Conflitos só podem aparecer nos pontos de registro (`docs/arquitetura.md`). Depois do pull,
`scripts\testar.bat tudo` precisa passar.

## Exceções às regras de `cadcraft/CLAUDE.md`

- `plan/`, `craftrules`, `cargo xtask wasm` e o push direto para `main` do CADCraft são do
  projeto original; aqui valem os portões acima e o fluxo de commits deste arquivo.
- Os textos em português do Free Elétrica não passam pelo catálogo de traduções (só rótulos de
  comandos e menus, regra 9).
