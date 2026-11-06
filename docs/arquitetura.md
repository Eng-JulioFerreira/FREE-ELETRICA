# Arquitetura do Free Elétrica e o fork do CADCraft

## Por que existe uma cópia do CADCraft

O CADCraft não tem sistema de plugins: a lista de painéis é fixa no código (`Panel` em
`crates/ui-egui/src/docking.rs`), os comandos são registrados numa lista estática
(`command_specs` em `crates/engine/src/cmd/mod.rs`) e o AutoLISP dele não abre janelas. Uma
paleta acoplada como Propriedades ou Camadas só existe mudando o código do CADCraft.

O Free Elétrica por isso vive em `cadcraft/`, um **git subtree** do repositório original. Quase todo
o código é nosso e fica em arquivos próprios; o código do CADCraft só recebe linhas de registro.
Atualizar o CADCraft é um `git subtree pull` (ver `CLAUDE.md`).

## Camadas

```
crates/free-eletrica            L2  motor do Free Elétrica (só depende de cadcraft-doc)
crates/engine         L4  comandos: cmd/eletrica.rs registra ELEPONTO, ELECIRCUITO, ELECOMANDO, ELEELETRODUTO, ELEFIACAO, eletrica.*
crates/ui-egui        L5  paleta, mini-barra, janelas e sobreposição: eletrica.rs (só lê e chama comandos)
apps/cadcraft             carrega/grava as configurações e abre a paleta na primeira vez
```

`cargo xtask layers` confere: `free-eletrica` está registrado em `xtask/src/layers.rs` como L2.

## Pontos de registro no código do CADCraft

As únicas linhas do Free Elétrica em arquivos do CADCraft. Mantenha a lista em dia; cada nova linha
aqui é um possível conflito ao atualizar o CADCraft.

| Arquivo | O quê |
| :--- | :--- |
| `Cargo.toml` | `free-eletrica` nas dependências do workspace |
| `crates/engine/Cargo.toml` | dependência de `free-eletrica` |
| `crates/engine/src/cmd/mod.rs` | `mod eletrica;` e `v.extend(eletrica::specs())` |
| `crates/ui-egui/Cargo.toml` | dependência de `free-eletrica` |
| `crates/ui-egui/src/lib.rs` | `pub mod eletrica;` e `eletrica::janelas(...)` no desenho da janela |
| `crates/ui-egui/src/docking.rs` | `Panel::QuadroCargas`, o quadro de cargas acoplado embaixo do desenho enquanto está aberto (`eletrica::quadro_acoplado`), e `Panel::Eletrica`, a coluna das versões anteriores, que `eletrica::janelas` fecha; o nome fica para um workspace salvo continuar abrindo (id, rótulo, lado, sob demanda, desenho, limites). O título da aba passa por `eletrica::titulo_do_painel`: com o quadro VDI no lugar do de cargas, "Quadro de Telecomunicações" |
| `crates/ui-egui/src/palettes.rs` | a terceira aba das Tool Sets, `eletrica::ABA_FERRAMENTAS` (Installations), com a largura das abas dividida, e o corpo dela em `eletrica::show` |
| `crates/ui-egui/src/canvas.rs` | `eletrica::sobre_canvas(...)` depois de desenhar o espaço do modelo |
| `crates/ui-egui/src/cmdline.rs` | `eletrica::tab(app)` no início de `tab`: Tab gira o ponto durante ELEPONTO |
| `crates/ui-egui/src/menus.rs` | item Janela > Elétrica e `eletrica::route(...)` |
| `crates/ui-egui/src/i18n/uk.tsv`, `pt_br.tsv` | rótulos dos comandos e dos painéis |
| `apps/cadcraft/src/main.rs` | carregar/gravar `freeeletrica.setup` e abrir a paleta ao iniciar |
| `xtask/src/layers.rs` | `free-eletrica` na tabela de camadas |
| `xtask/src/main.rs` | `mod dist;` e o comando `dist`, em `xtask/src/dist.rs` |

## Marca ArtCraft

A licença da marca (`cadcraft/docs/brand/LICENSE-brand.txt`) pede que versões modificadas
retirem os logos do ArtCraft e não se apresentem como produto ArtCraft; dizer em texto que o
Free Elétrica é baseado no CADCraft da equipe ArtCraft é permitido. Os avisos de copyright
(LICENSE-MIT, NOTICE, recursos do executável) ficam como estão. Mudanças feitas por isso:

| Arquivo | O quê |
| :--- | :--- |
| `docs/brand/` | os arquivos de logo removidos; só o texto da licença fica |
| `README.md`, `ATTRIBUTION.md` | sem as imagens do logo, com a nota de versão modificada |
| `crates/ui-egui/src/dialogs.rs` | Sobre: "Free Elétrica, based on CADCraft by the ArtCraft team" e o link do Free Elétrica no lugar dos links do ArtCraft |
| `apps/cadcraft/src/native_menu.rs` | sem o item "Join the ArtCraft Discord" (menu do macOS) |

Ao atualizar o CADCraft, confira se a versão nova trouxe a marca de volta (logos novos, links
para getartcraft.com ou o Discord do ArtCraft na interface).

## O fork no GitHub

O repositório público é `Eng-JulioFerreira/Free-Eletrica`, criado pelo botão de fork (API
`POST /repos/storytold/cadcraft/forks`), então o GitHub o mostra ligado ao original. A branch padrão, `free-eletrica`, é
este repositório inteiro, com o CADCraft dentro de `cadcraft/`: quem visita vê o Free Elétrica primeiro. O `main` do fork
ficou parado na última publicação antiga, que copiava a árvore de `cadcraft/` num commit por vez. O repositório antigo,
`FREE-ELETRICA-legacy-archive`, é privado e arquivado, só para consulta.

## Como sair do fork um dia

O caminho sem cópia é propor ao CADCraft um ponto de extensão genérico, útil para qualquer
addon: registrar comandos extras no motor e painéis/janelas extras na interface, e o app aceitar
essas extensões na inicialização. Com isso aceito no projeto original, o Free Elétrica vira um
repositório próprio que depende das crates do CADCraft e monta o próprio executável, sem mexer
no código delas. Os pontos de registro da tabela acima são exatamente o que essa API precisaria
cobrir.
