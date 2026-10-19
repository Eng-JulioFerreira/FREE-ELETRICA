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
crates/engine         L4  comandos: cmd/eletrica.rs registra ELECIRCUITO, ELECOMANDO, ELEFIACAO, eletrica.*
crates/ui-egui        L5  paleta e janelas: eletrica.rs (só lê e chama comandos)
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
| `crates/ui-egui/src/docking.rs` | `Panel::Eletrica` (id, rótulo, lado, sob demanda, desenho, limites) |
| `crates/ui-egui/src/menus.rs` | item Janela > Elétrica e `eletrica::route(...)` |
| `crates/ui-egui/src/i18n/uk.tsv`, `pt_br.tsv` | rótulos dos comandos e do painel |
| `apps/cadcraft/src/main.rs` | carregar/gravar `freeeletrica.setup` e abrir a paleta ao iniciar |
| `xtask/src/layers.rs` | `free-eletrica` na tabela de camadas |

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

O repositório `Eng-JulioFerreira/cadcraft-free-eletrica` foi criado pelo botão de fork (API
`POST /repos/storytold/cadcraft/forks`), então o GitHub o mostra ligado ao original. Ele não
tem histórico próprio de desenvolvimento: cada publicação (`scripts\publicar_fork.ps1`) é um
commit com a árvore de `cadcraft/` deste repositório. O primeiro commit tem como pai a Release
0.9.0 do CADCraft; os seguintes têm como pai o commit anterior do fork e, quando o subtree foi
atualizado, também o commit oficial novo (o `git-subtree-split` do squash), para o fork seguir
ligado ao CADCraft.

## Como sair do fork um dia

O caminho sem cópia é propor ao CADCraft um ponto de extensão genérico, útil para qualquer
addon: registrar comandos extras no motor e painéis/janelas extras na interface, e o app aceitar
essas extensões na inicialização. Com isso aceito no projeto original, o Free Elétrica vira um
repositório próprio que depende das crates do CADCraft e monta o próprio executável, sem mexer
no código delas. Os pontos de registro da tabela acima são exatamente o que essa API precisaria
cobrir.
