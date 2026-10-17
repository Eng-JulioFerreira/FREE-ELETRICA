# Arquitetura do CRAFT ELE e o fork do CADCraft

## Por que existe uma cópia do CADCraft

O CADCraft não tem sistema de plugins: a lista de painéis é fixa no código (`Panel` em
`crates/ui-egui/src/docking.rs`), os comandos são registrados numa lista estática
(`command_specs` em `crates/engine/src/cmd/mod.rs`) e o AutoLISP dele não abre janelas. Uma
paleta acoplada como Propriedades ou Camadas só existe mudando o código do CADCraft.

O CRAFT ELE por isso vive em `cadcraft/`, um **git subtree** do repositório original. Quase todo
o código é nosso e fica em arquivos próprios; o código do CADCraft só recebe linhas de registro.
Atualizar o CADCraft é um `git subtree pull` (ver `CLAUDE.md`).

## Camadas

```
crates/ele            L2  motor do CRAFT ELE (só depende de cadcraft-doc)
crates/engine         L4  comandos: cmd/eletrica.rs registra ELECIRCUITO, ELECOMANDO, ELEFIACAO, eletrica.*
crates/ui-egui        L5  paleta e janelas: eletrica.rs (só lê e chama comandos)
apps/cadcraft             carrega/grava as configurações e abre a paleta na primeira vez
```

`cargo xtask layers` confere: `ele` está registrado em `xtask/src/layers.rs` como L2.

## Pontos de registro no código do CADCraft

As únicas linhas do CRAFT ELE em arquivos do CADCraft. Mantenha a lista em dia; cada nova linha
aqui é um possível conflito ao atualizar o CADCraft.

| Arquivo | O quê |
| :--- | :--- |
| `Cargo.toml` | `cadcraft-ele` nas dependências do workspace |
| `crates/engine/Cargo.toml` | dependência de `cadcraft-ele` |
| `crates/engine/src/cmd/mod.rs` | `mod eletrica;` e `v.extend(eletrica::specs())` |
| `crates/ui-egui/Cargo.toml` | dependência de `cadcraft-ele` |
| `crates/ui-egui/src/lib.rs` | `pub mod eletrica;` e `eletrica::janelas(...)` no desenho da janela |
| `crates/ui-egui/src/docking.rs` | `Panel::Eletrica` (id, rótulo, lado, sob demanda, desenho, limites) |
| `crates/ui-egui/src/menus.rs` | item Janela > Elétrica e `eletrica::route(...)` |
| `crates/ui-egui/src/i18n/uk.tsv`, `pt_br.tsv` | rótulos dos comandos e do painel |
| `apps/cadcraft/src/main.rs` | carregar/gravar `craftele.setup` e abrir a paleta ao iniciar |
| `xtask/src/layers.rs` | `ele` na tabela de camadas |

## Como sair do fork um dia

O caminho sem cópia é propor ao CADCraft um ponto de extensão genérico, útil para qualquer
addon: registrar comandos extras no motor e painéis/janelas extras na interface, e o app aceitar
essas extensões na inicialização. Com isso aceito no projeto original, o CRAFT ELE vira um
repositório próprio que depende das crates do CADCraft e monta o próprio executável, sem mexer
no código delas. Os pontos de registro da tabela acima são exatamente o que essa API precisaria
cobrir.
