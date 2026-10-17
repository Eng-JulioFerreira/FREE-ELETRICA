@AGENTS.md

# CRAFT ELE — instruções para agentes

CRAFT ELE é um addon de projeto elétrico predial de baixa tensão (NBR 5410) e telecom para o
[CADCraft](https://github.com/storytold/cadcraft), em Rust. Ele vive dentro de uma cópia do
CADCraft (`cadcraft/`, um git subtree do repositório original) e aparece como a paleta
**Elétrica**, acoplada como os painéis do próprio CADCraft.

As premissas do CADCraft valem aqui. Ao mexer em `cadcraft/`, leia também
`cadcraft/CLAUDE.md`: as regras dele (nunca travar, tudo é comando, interface fina, camadas,
clean-room, assets) são obrigatórias. As exceções estão no fim deste arquivo.

## Mapa

| Onde | O quê |
| :--- | :--- |
| `cadcraft/crates/ele/` | **Motor do CRAFT ELE**: pontos, circuitos, NBR 5410, roteamento, fiação, setup. Sem interface. |
| `cadcraft/crates/engine/src/cmd/eletrica.rs` | **Comandos** do motor: `elecircuito`, `elecomando`, `elefiacao`, `eletrica.*`. |
| `cadcraft/crates/ui-egui/src/eletrica.rs` | **Interface**: a paleta Elétrica e as janelas. Só lê e chama comandos. |
| `cadcraft/crates/ele/src/tests.rs` | Testes do motor. |
| `cadcraft/crates/engine/tests/eletrica_comandos.rs` | Testes dos comandos, sem interface. |
| `cadcraft/crates/ui-egui/tests/eletrica_painel.rs` | Testes da paleta e das janelas. |
| `docs/` | Padrão de blocos, arquitetura do fork. |
| `templates/` | Template de blocos e camadas (`CRAFT_ELE_BLOCOS.dxf`) e a fonte dos símbolos. |
| `scripts/` | `compilar.bat`, `iniciar.bat`, `testar.bat`, `gerar_template.py`. |

Pontos de registro no código do CADCraft (as únicas linhas nossas fora dos nossos arquivos)
estão listados em `docs/arquitetura.md`. Qualquer mudança nova num arquivo do CADCraft entra
nessa lista e precisa ser pequena.

## Regras inegociáveis

1. **Test-driven.** Toda função nova nasce com teste; todo bug corrigido ganha um teste que
   falhava antes. Escreva o teste primeiro sempre que der. **Nenhum commit com teste falhando**,
   nem do CADCraft. Nunca apague, enfraqueça ou ignore um teste para passar: conserte o código,
   ou explique ao usuário por que o teste estava errado antes de mudá-lo.
2. **Motor desacoplado da interface.** Toda regra elétrica, cálculo, roteamento e escrita no
   desenho fica em `crates/ele` (camada L2, só depende de `cadcraft-doc`; nunca de egui).
   A interface não decide nada de engenharia.
3. **Tudo é comando.** Toda capacidade do motor é um `CommandSpec` em
   `crates/engine/src/cmd/eletrica.rs`, com a forma JSON (`run`) e, quando pede seleção no
   desenho, o prompt interativo (`interactive`). Cada comando tem teste em
   `eletrica_comandos.rs` rodando **sem interface**. Assim a linha de comando, scripts, o canal de
   controle, o `cadcraft-cli` e o MCP alcançam tudo. Chamadas programáticas nunca abrem janela.
4. **Interface fina.** A paleta e as janelas leem o estado e agem por `app.run(id, params)` /
   `app.start(id)`. Comandos só de interface (abrir paleta e janelas) ficam em `route()` de
   `ui-egui/src/eletrica.rs`. A paleta só tem ferramentas; dados aparecem em janelas.
5. **Nunca travar** (padrão do CADCraft): sem `unwrap`/`expect`/`panic!`/`todo!` fora de testes
   (`#![deny]` na raiz de `ele`), erros por `Result`, entrada do desenho e de parâmetros JSON é
   hostil (`get()`, nada de índice vindo de fora, NaN e infinitos tratados).
6. **Dados no desenho.** Valores vão para o atributo do bloco quando ele existe e, senão, para o
   XDATA `CRAFTELE` (`ele::editar`, `ele::xdata`). O que é gerado (marcas de fiação) leva XDATA
   próprio para poder ser apagado e refeito.
7. **Clean-room.** Programas de referência (OFE Elétrico, PRO-Elétrica, Lumine, AutoCAD) podem
   ser **observados** em uso; nunca descompilados, nunca copiados (código, símbolos, tabelas
   próprias, textos). Regras de cálculo vêm da NBR 5410 e são citadas no código (tabela, item).
8. **Licença.** O código do CRAFT ELE é GPL-3.0-only: todo arquivo novo nosso começa com
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
`refactor:`), um por arco de funcionalidade que compila e passa.

## Compilar, rodar e olhar o resultado

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

## Atualizar o CADCraft

```
git subtree pull --prefix cadcraft https://github.com/storytold/cadcraft.git main --squash
```

Conflitos só podem aparecer nos pontos de registro (`docs/arquitetura.md`). Depois do pull,
`scripts\testar.bat tudo` precisa passar.

## Exceções às regras de `cadcraft/CLAUDE.md`

- `plan/`, `craftrules`, `cargo xtask wasm` e o push direto para `main` do CADCraft são do
  projeto original; aqui valem os portões acima e o fluxo de commits deste arquivo.
- Os textos em português do CRAFT ELE não passam pelo catálogo de traduções (só rótulos de
  comandos e menus, regra 9).
