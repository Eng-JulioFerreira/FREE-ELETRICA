# Free Elétrica

Projeto elétrico predial de baixa tensão direto no CAD: pontos, circuitos, eletrodutos, dimensionamento pela
ABNT NBR 5410, quadro de cargas, tags de fiação e lista de materiais. Software livre, em português.

## Baixar para Windows

**[Baixar a versão mais recente](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest)**

1. Baixe o arquivo `.zip` da página acima.
2. Extraia em qualquer pasta.
3. Clique duas vezes em `FreeEletrica.exe`.

Não precisa instalar nada: nem Rust, nem compilador, nem scripts. Funciona no Windows 10 e 11, 64 bits. Na
primeira vez, o Windows pode avisar que o aplicativo é desconhecido: clique em **Mais informações** e
**Executar assim mesmo**. Veja o [guia de instalação](docs/INSTALL.md).

## O que ele faz

- **Dimensionamento pela NBR 5410:** corrente de projeto, FCT, FCA real por trecho, capacidade de condução,
  queda de tensão em 3D, seção e disjuntor, com travas para o que você escolher à mão.
- **Eletrodutos que seguem a obra:** retos, a 45° ou em arco, com desvios e perfis que trazem o tipo de linha, a cor, o
  caminhamento e os diâmetros. Cruzamento sem caixa não conecta, como na instalação real.
- **Cada quadro com o seu sistema:** sistema, tensão nominal, cabo padrão e quem o alimenta ficam no próprio quadro.
- **Dimensionamento revisável:** esquema, cabo, curva e grupos de DR por circuito, com o resultado à vista antes de aplicar.
- **Catálogos editáveis:** cabos elétricos e de telecom e perfis de eletroduto, com editar e excluir.
- **Blocos como famílias:** categoria, altura padrão, caixa, potência e classe de demanda no próprio bloco. A altura fora
  da legenda aparece sozinha na planta, como `h=1,45m`.
- **Documentação que não se desfaz:** tags de fiação que guardam a posição que você deu, tabela do quadro e diagrama unifilar
  no desenho e lista de materiais.
- **Tudo dentro do DXF:** circuitos, famílias e resultados viajam com o arquivo.
- **Demanda da concessionária:** tabelas da Equatorial, da CEMIG e da Enel Goiás, e um editor para cadastrar a sua. Cada
  ponto ou família pode ter fator próprio.
- **Pronto para automação:** todo o motor responde por comandos JSON, sem interface.

Os símbolos dos blocos base seguem as formas usuais da NBR 5444, com medidas proporcionais do projeto.

## O fluxo de um projeto

| Etapa | Ferramenta | O que você faz |
| :--- | :--- | :--- |
| 1. Setup | Configurações | Temperatura, limites de queda, alturas, cabos, perfis de eletroduto e demanda. Camadas e blocos entram sozinhos. |
| 2. Blocos e pontos | Blocos e famílias, Inserir pontos | Crie seus símbolos ou use os prontos. Na mini-barra, escolha bloco, potência e altura, e clique na planta. |
| 3. Circuitos e comandos | Criar circuito, Comandos e luminárias | Agrupe os pontos e ligue interruptores às luminárias. A inspeção visual aponta o que falta. |
| 4. Eletrodutos | Inserir eletroduto | Escolha o perfil e ligue os blocos em traçado reto, a 45° ou em arco. Clique no meio de um trecho para pôr uma caixa. |
| 5. Dimensionamento | Dimensionar projeto | Escolha esquema, cabo e grupos de DR, calcule, revise e aplique ao quadro. |
| 6. Quadro e tags | Quadro de cargas, Tags de fiação, Lista de materiais | Confira o resumo e a ficha de cada circuito e documente na planta. |

O passo a passo completo está em [Primeiros passos](docs/GETTING_STARTED.md).

## Responsabilidade técnica

O Free Elétrica apoia o projeto, e os resultados precisam da conferência de um profissional habilitado. Diâmetros de
cabos e eletrodutos são valores típicos de catálogo, e as sugestões de DR e DPS devem ser conferidas. Veja o
[aviso legal](AVISO_LEGAL.md).

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
