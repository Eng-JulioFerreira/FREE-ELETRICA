# CRAFT ELE

Projeto elétrico predial de baixa tensão (NBR 5410) e telecom dentro do
[CADCraft](https://github.com/storytold/cadcraft), no estilo do OFE Elétrico, do PRO-Elétrica e
do Lumine. Aparece como a paleta **Elétrica**, acoplada à interface do CADCraft.

> **Aviso:** os resultados são de apoio e não substituem a responsabilidade técnica de um
> profissional habilitado. Leia o [AVISO_LEGAL.md](AVISO_LEGAL.md).

## Visão: de fork a plugin

Hoje o CRAFT ELE é um **fork** do CADCraft: uma cópia dele em `cadcraft/` com algumas linhas de
registro, porque o CADCraft ainda não tem como receber extensões. **A meta é virar um plugin**:
um projeto separado que só traz o próprio código e usa o CADCraft original sem modificá-lo.

Isso depende de o CADCraft ganhar uma **API de extensão** (registrar comandos extras no motor e
painéis e janelas extras na interface, carregados quando o programa inicia). O código do CRAFT
ELE já está organizado para essa mudança: o motor fica numa biblioteca própria, os comandos e a
paleta ficam em arquivos próprios, e as poucas linhas nos arquivos do CADCraft estão listadas em
[docs/arquitetura.md](docs/arquitetura.md). Elas são exatamente o que a API de extensão precisaria
oferecer.

## O que faz

| Ferramenta | Comando | O quê |
| :--- | :--- | :--- |
| Configurações | `ELECONFIG` | Setup do projeto: sistema (380/220 V padrão), pé-direito, temperatura (FCT), queda admissível, cores das fases, seções omitidas, camadas de eletroduto de teto e piso, catálogo de cabos de telecom. |
| Criar circuito | `ELECIRCUITO` | Clique no quadro, aceite o número do circuito e clique nos pontos. |
| Comandos e luminárias | `ELECOMANDO` | Clique no interruptor e nas luminárias que ele comanda. |
| Conferir pontos e circuitos | `ELECONFERIR` | Pontos, circuitos e atributos que faltam. |
| Rotear fiação | `ELEROTEAR` | Rota pelos eletrodutos, FCA por agrupamento, queda de tensão, seção e disjuntor pré-selecionados (com sobrescrita), fases, resumo técnico. |
| Ajustar fiação | `ELEFIACAO` | Desenha as marcas de fase, neutro, retorno e proteção, com circuito e seção, sobre cada trecho de eletroduto. |

Tudo também responde em JSON pelo canal de controle e pelo `cadcraft-cli`:
`eletrica.analisar`, `eletrica.rotear`, `eletrica.sobrescrever`, `eletrica.setup`,
`elecircuito`, `elecomando`, `elefiacao`.

## Começar

1. `scripts\compilar.bat` compila (`release` para a versão otimizada).
2. `scripts\iniciar.bat` abre o CADCraft com a paleta Elétrica.
3. Abra `templates\CRAFT_ELE_BLOCOS.dxf` e use "Salvar como" para começar um projeto: ele traz os
   blocos INT-SIM, TOM-SIM-BAIXA, QDG e ILU-OCT e as camadas de eletroduto PVC linha média.

Pré-requisitos (uma vez): Rust e MinGW em `C:\rust`, como descrito em `CLAUDE.md`.

## Estrutura

```
CLAUDE.md     regras do projeto (test-driven, motor desacoplado, tudo é comando)
cadcraft/     o CADCraft com o CRAFT ELE (crates/ele, cmd/eletrica.rs, ui-egui/eletrica.rs)
docs/         padrão de blocos e arquitetura do fork
templates/    template de blocos e camadas
scripts/      compilar, iniciar, testar, gerar o template
```

## Desenvolvimento

Test-driven: `scripts\testar.bat` precisa passar antes de todo commit. Veja `CLAUDE.md` e
`docs/arquitetura.md`.

## Licença

- **CRAFT ELE** (o código próprio: `cadcraft/crates/ele/`, os arquivos `eletrica*.rs` nos crates
  do CADCraft, `docs/`, `templates/`, `scripts/`): **GNU General Public License v3**
  (`GPL-3.0-only`, texto em [LICENSE](LICENSE)). Os arquivos de código levam o cabeçalho
  `SPDX-License-Identifier: GPL-3.0-only`.
- **Isenção de garantia e de responsabilidade:** [AVISO_LEGAL.md](AVISO_LEGAL.md), que complementa
  as seções 15 e 16 da GPL.
- **CADCraft** (o resto de `cadcraft/`): continua sob as licenças dele, MIT OR Apache-2.0
  (`cadcraft/LICENSE-MIT`, `cadcraft/LICENSE-APACHE`, `cadcraft/NOTICE`), que precisam acompanhar
  o repositório. Ambas são compatíveis com a GPL v3; o programa compilado como um todo é
  distribuído sob a GPL v3.
