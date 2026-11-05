# Free Elétrica

Projeto elétrico predial de baixa tensão (NBR 5410) e telecom dentro do
[CADCraft](https://github.com/storytold/cadcraft), no estilo do OFE Elétrico, do PRO-Elétrica e
do Lumine. Aparece como a paleta **Elétrica**, acoplada à interface do CADCraft.

> **Aviso:** os resultados são de apoio e não substituem a responsabilidade técnica de um
> profissional habilitado. Leia o [AVISO_LEGAL.md](AVISO_LEGAL.md).

## Visão: de fork a plugin

Hoje o Free Elétrica é um **fork** do CADCraft: uma cópia dele em `cadcraft/` com algumas linhas de
registro, porque o CADCraft ainda não tem como receber extensões. **A meta é virar um plugin**:
um projeto separado que só traz o próprio código e usa o CADCraft original sem modificá-lo.

Isso depende de o CADCraft ganhar uma **API de extensão** (registrar comandos extras no motor e
painéis e janelas extras na interface, carregados quando o programa inicia), proposta em
[storytold/cadcraft#1544](https://github.com/storytold/cadcraft/issues/1544). O código do Free
Elétrica já está organizado para essa mudança: o motor fica numa biblioteca própria, os comandos e a
paleta ficam em arquivos próprios, e as poucas linhas nos arquivos do CADCraft estão listadas em
[docs/arquitetura.md](docs/arquitetura.md). Elas são exatamente o que a API de extensão precisaria
oferecer.

**Fork no GitHub:** [Eng-JulioFerreira/cadcraft-free-eletrica](https://github.com/Eng-JulioFerreira/cadcraft-free-eletrica)
é um fork de verdade do CADCraft (aparece na rede de forks do projeto original) com o Free
Elétrica dentro. Ele traz só a pasta `cadcraft/` deste repositório, publicada por
`scripts\publicar_fork.bat`; o desenvolvimento, os templates, os scripts e a documentação ficam
aqui.

## O que faz

A paleta **Elétrica** segue o fluxo de trabalho. Nada fica aberto sem ser pedido: enquanto um comando
espera cliques, uma mini-barra sobre o desenho muda o que ele usa; os dados ficam em janelas
independentes, que fecham no X ou com Esc.

| Ferramenta | Comando | O quê |
| :--- | :--- | :--- |
| 1. Blocos e famílias | `ELEBLOCOS` | Lista os blocos elétricos e tem **+ Criar novo bloco elétrico**. No editor de blocos, a paleta vira as Propriedades da Família: domínio, categoria, altura padrão, caixa física, potência, FP e teclas, mais geometria base, ponto de conexão e atributos prontos. |
| 1. Inspetor de atributos | `ELEATRIBUTOS` | Selecione um bloco e mude valor, visibilidade e posição de cada atributo. |
| 1. Inserir pontos | `ELEPONTOS` | A mini-barra tem categoria, bloco, potência, altura e quadro pai; cada clique no desenho insere um ponto. Ele alinha à parede mais próxima, e Tab gira. Altura diferente da legenda aparece na planta como `h=1,45m`. |
| 1. Lista de pontos | | Pontos por categoria: altura, selecionar, excluir. |
| 2. Criar circuito | `ELECIRCUITO` | Quadro, número e pontos; a barra edita descrição, esquema (F+N, F+N+T, 2F+T, 3F+N+T) e tensão. O atributo CIRCUITO muda na hora. |
| 2. Comandos e luminárias | `ELECOMANDO` | Interruptor e luminárias. A letra é a próxima livre, respeitando as teclas do interruptor, ou a que você digitar. |
| 3. Quadro de cargas | `ELECARGAS` (ou `ELEROTEAR`) | Janela em duas abas. O **resumo** tem pontos, potência, Ib, fases R, S, T, grupo DR e o cabo e disjuntor do último cálculo. A **ficha de engenharia** tem tipo de cabo, FCT, FCA, I'b, Iz, queda e as travas de seção e disjuntor (vermelho se viola a NBR 5410). Clicar no circuito o destaca no desenho. |
| 3. Dimensionar projeto | `ELEDIMENSIONAR` | Premissas (PE compartilhado, temperatura, FCA) e **Processar cálculo**. Só aqui o quadro, as tags e as tabelas recebem o resultado. |
| 4. Inserir eletroduto | `ELEELETRODUTO` | Clique nos blocos em sequência; a mini-barra escolhe traçado (reto, ortogonal ou arco), cota e tipo. Um clique no vazio vira um desvio do trajeto, e no meio de um eletroduto divide o trecho com uma caixa de passagem. |
| 4. Lista de eletrodutos | | Trechos com tipo, cota, diâmetro e ocupação. |
| 5. Tags de fiação | `ELEFIACAO` | Simbologia, circuito, seção e diâmetro em cada eletroduto. A posição que você der à tag fica guardada, e ela acompanha as mudanças do projeto. |
| 5. Tabela do quadro | `ELEQUADRO_TABELA` | Desenha o quadro de cargas num ponto da planta e o refaz no mesmo lugar. |
| 5. Atualizar | `ELEREGEN` | Refaz tags e tabelas sem mudar as posições. |
| 5. Lista de materiais | `ELEMATERIAIS` | Cabos por seção e tipo, eletrodutos por tipo e diâmetro, caixas, pontos e disjuntores. |
| Inspecionar projeto | `ELEINSPECAO`, `ELECONFERIR` | Halos no desenho: vermelho falta circuito ou comando, laranja sem caminho até o quadro, amarelo acima do limite, verde válido. |
| Configurações | `ELECONFIG` | Critérios elétricos (sistema, temperatura, isolação, quedas de iluminação, força e alimentador, folga de cabos), alturas padrão do projeto, cores, diâmetros, eletrodutos, telecom e demanda da concessionária. |

Os eletrodutos formam um **grafo estrito**: cada um liga exatamente dois blocos elétricos. Cruzamentos
e derivações sem caixa não conectam. Veja `docs/free-eletrica-2.0.md`.

Tudo também responde em JSON pelo canal de controle e pelo `cadcraft-cli`: `eletrica.analisar`,
`eletrica.rotear`, `eletrica.sobrescrever`, `eletrica.circuito.*`, `eletrica.quadro`,
`eletrica.eletroduto.*`, `eletrica.eletrodutos`, `eletrica.materiais`, `eletrica.dimensionar`,
`eletrica.quadro.equilibrar`, `eletrica.inspecionar`, `eletrica.bootstrap`,
`eletrica.demanda.tabelas`, `eletrica.blocos`, `eletrica.ponto.modelo`, `eletrica.ponto.altura`,
`eletrica.ponto.excluir`, `eletrica.familia.*`, `elefamilia_conector`,
`eletrica.setup`, `eleponto`, `elecircuito`, `eleremoverponto`, `elecomando`, `eleeletroduto`,
`elefiacao`, `eleregen`, `elequadro_tabela`.

## Começar

1. `scripts\compilar.bat` compila (`release` para a versão otimizada).
2. `scripts\iniciar.bat` abre o CADCraft com a paleta Elétrica.
3. Abra ou crie um desenho e use as ferramentas: as camadas e os blocos entram na primeira vez.
   O template `templates\FREE_ELETRICA_BLOCOS.dxf` é opcional.

Para só usar, sem compilar, veja [docs/INSTALL.md](docs/INSTALL.md) e
[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md). O pacote `.zip` é gerado com `cargo xtask dist`.

Pré-requisitos (uma vez): Rust e MinGW em `C:\rust`, como descrito em `CLAUDE.md`.

## Estrutura

```
CLAUDE.md     regras do projeto (test-driven, motor desacoplado, tudo é comando)
cadcraft/     o CADCraft com o Free Elétrica (crates/free-eletrica, cmd/eletrica.rs, ui-egui/eletrica.rs)
docs/         padrão de blocos, arquitetura do fork e o registro da versão 2.0
templates/    template de blocos e camadas
scripts/      compilar, iniciar, testar, gerar o template
```

## Desenvolvimento

Test-driven: `scripts\testar.bat` precisa passar antes de todo commit. Veja `CLAUDE.md` e
`docs/arquitetura.md`.

## Licença

- **Free Elétrica** (o código próprio: `cadcraft/crates/free-eletrica/`, os arquivos `eletrica*.rs` nos crates
  do CADCraft, `docs/`, `templates/`, `scripts/`): **GNU General Public License v3**
  (`GPL-3.0-only`, texto em [LICENSE](LICENSE)). Os arquivos de código levam o cabeçalho
  `SPDX-License-Identifier: GPL-3.0-only`.
- **Isenção de garantia e de responsabilidade:** [AVISO_LEGAL.md](AVISO_LEGAL.md), que complementa
  as seções 15 e 16 da GPL.
- **CADCraft** (o resto de `cadcraft/`): continua sob as licenças dele, MIT OR Apache-2.0
  (`cadcraft/LICENSE-MIT`, `cadcraft/LICENSE-APACHE`, `cadcraft/NOTICE`), que precisam acompanhar
  o repositório. Ambas são compatíveis com a GPL v3; o programa compilado como um todo é
  distribuído sob a GPL v3.
