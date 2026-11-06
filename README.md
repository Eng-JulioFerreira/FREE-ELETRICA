# Free Elétrica

**👉 [Guia de Início Rápido (Getting Started): aprenda a criar seu primeiro projeto passo a passo](docs/GETTING_STARTED.md)**

O Free Elétrica é uma ferramenta CAD nativa para projetos de **instalações elétricas de baixa tensão** (ABNT NBR 5410, com a
simbologia da NBR 5444) e de **cabeamento estruturado / telecomunicações** (ABNT NBR 14565). Funciona dentro do
[CADCraft](https://github.com/storytold/cadcraft): você desenha na planta, e o programa calcula, dimensiona e documenta
a prancha. Software livre, em português.

## Baixar para Windows

**[Baixar a versão mais recente](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest)**

1. Baixe o arquivo `.zip` da página acima.
2. Extraia em qualquer pasta.
3. Clique duas vezes em `FreeEletrica.exe`.

Não precisa instalar nada: nem Rust, nem compilador, nem scripts. Funciona no Windows 10 e 11, 64 bits. Na primeira vez,
o Windows pode avisar que o aplicativo é desconhecido: clique em **Mais informações** e **Executar assim mesmo**. Veja o
[guia de instalação](docs/INSTALL.md).

## Principais recursos

- **Blocos modulares com empilhamento:** tomadas, interruptores e pontos de dados como famílias, com potência, altura e
  caixa. Clique sobre um ponto da parede e o módulo novo entra na mesma caixa 4x2" ou 4x4", sem duplicar a caixa.
- **Grafo 3D de eletrodutos:** cada eletroduto liga dois blocos, em linha reta, a 45° ou em arco, pelo teto, pela parede ou
  pelo piso. O comprimento soma a planta e as subidas e descidas reais até cada bloco (o desnível em Z). A taxa de
  ocupação do eletroduto e o fator de agrupamento (FCA) saem do roteamento.
- **Quadro de cargas acoplado:** embaixo do desenho, com equilíbrio de fases, grupos de DR e a ficha de cada circuito.
- **Dimensionamento pela NBR 5410:** seção pela capacidade de condução nos métodos A1 a F, queda de tensão, disjuntor
  entre Ib e Iz, neutro e PE pelas tabelas 48 e 58, disjuntor geral, DR e DPS, com travas para o que você escolher à mão.
  Seleção múltipla com Shift e Ctrl para editar vários circuitos de uma vez.
- **Documentação gerada na prancha DXF:** tabela do quadro de cargas, diagrama unifilar com quadro de simbologia, vista
  frontal de montagem nos trilhos DIN, legendas de símbolos elétrica e de telecom, tags de fiação e lista de materiais.
  Tudo se refaz com um comando, sem mudar as posições.
- **Telecomunicações (NBR 14565):** topologia em estrela, um cabo inteiro de cada ponto até o rack; comprimento 3D com
  alerta acima de 90 m; eletroduto com 40 % de ocupação; o Quadro de Telecomunicações espelha as portas do rack.
- **Demanda da concessionária:** a tabela oficial da Equatorial (NT.00001.EQTL) e um editor para cadastrar a sua.
- **Tudo dentro do DXF:** circuitos, famílias e resultados viajam com o arquivo.
- **Pronto para automação:** todo o motor responde por comandos JSON, sem interface.

## O fluxo de um projeto

| Etapa | Onde | O que você faz |
| :--- | :--- | :--- |
| 1. Configurar | Configurações... | Unidade, alturas, critérios, cabos, eletrodutos e demanda. |
| 2. Pontos | Inserir pontos | Quadro, tomadas, luminárias e interruptores; módulos empilhados na mesma caixa. |
| 3. Circuitos | Criar circuito, Comandos e luminárias | Agrupe os pontos e ligue interruptores simples e paralelos às luminárias. |
| 4. Eletrodutos | Inserir eletroduto | Ligue os blocos pelo teto, pela parede ou pelo piso. |
| 5. Dimensionar | Dimensionar projeto... | Esquema, cabo, método, DR e proteção geral; calcule, revise e aplique. |
| 6. Documentar | Documentação e Tabelas | Tags, tabela do quadro, unifilar, vista frontal, legenda e materiais. |
| 7. Telecom | Aba Telecom | Pontos de dados e rack, vínculo, dimensionamento, tags e tabela do rack. |

O passo a passo completo está no [Guia de Início Rápido](docs/GETTING_STARTED.md).

## Responsabilidade técnica

O Free Elétrica apoia o projeto, e os resultados precisam da conferência de um profissional habilitado. Diâmetros de
cabos e eletrodutos são valores típicos de catálogo, as tabelas de capacidade devem ser conferidas na norma vigente e as
sugestões de DR e DPS devem ser revisadas. Veja o [aviso legal](AVISO_LEGAL.md).

## Para desenvolvedores

- [Documentação técnica](docs/SPECIFICATION.md): arquitetura, dados no DXF e comandos.
- [Padrão de blocos](docs/padrao_blocos_eletricos.md) e [regras do projeto](CLAUDE.md).
- O CADCraft fica em [`cadcraft/`](cadcraft/), e o código do Free Elétrica em `cadcraft/crates/free-eletrica`,
  `cadcraft/crates/engine/src/cmd/eletrica.rs` e `cadcraft/crates/ui-egui/src/eletrica.rs`.
- Para gerar o pacote: `scripts\package_release.ps1`.

## Licença

O código do Free Elétrica é [GPL-3.0](LICENSE). O CADCraft segue MIT OR Apache-2.0, em [`cadcraft/`](cadcraft/).

---

**Nota técnica:** hoje o Free Elétrica funciona como um fork integrado ao CADCraft, porque o CADCraft ainda não tem um
sistema de plugins. O código elétrico fica em arquivos próprios, ligados por poucas linhas de registro. Ele aguarda a API
oficial de plugins proposta em [storytold/cadcraft#1544](https://github.com/storytold/cadcraft/issues/1544). Baseado no
[CADCraft](https://github.com/storytold/cadcraft) da equipe ArtCraft; não é um produto ArtCraft.
