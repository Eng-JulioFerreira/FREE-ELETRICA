# CRAFT ELE - Projeto elétrico (NBR 5410) dentro do CADCraft

O **CRAFT ELE** é um addon de projeto elétrico predial para o **CADCraft**, no estilo do
PRO-Elétrica e do Lumine. Ele aparece como um painel acoplado dentro do próprio CADCraft,
ao lado de Propriedades e Camadas.

---

## Por que um fork do CADCraft

O CADCraft não tem sistema de plugins: os painéis são fixos no código-fonte e o AutoLISP não
abre janelas. Para ter um painel acoplado de verdade, o CRAFT ELE vive num **fork do CADCraft**
(pasta `cadcraft/`, branch `craft-ele`), escrito em Rust como o resto do programa.

O fork mexe o mínimo possível nos arquivos do CADCraft, para facilitar puxar as atualizações
do projeto original:

| Onde | O quê |
| :--- | :--- |
| `cadcraft/crates/ele/` | Biblioteca do CRAFT ELE: pontos, circuitos e NBR 5410. Não depende da interface. |
| `cadcraft/crates/ui-egui/src/eletrica.rs` | O painel Elétrica. |
| `docking.rs`, `menus.rs`, `lib.rs`, `i18n/*.tsv` | Poucas linhas para registrar o painel, o comando e o menu. |

---

## Fluxo de uso

1. O projetista insere os blocos (quadro, pontos de luz, tomadas, interruptores) com os
   atributos do [padrão de blocos](docs/padrao_blocos_eletricos.md).
2. A paleta **Elétrica** já abre com o programa, acima das Tool Sets. Ela só tem ferramentas.
   Se for fechada, volta com `ELETRICA` ou **Janela > Elétrica**.
3. **Configurações...** (`ELECONFIG`) define o setup do projeto: sistema de distribuição
   (tensões e fases) e temperatura ambiente, de onde sai o FCT. O FCA não é digitado: vai sair
   do agrupamento nos eletrodutos e, até lá, vale 1,00.
4. **Conferir pontos e circuitos...** (`ELECONFERIR`) abre a janela com os pontos, os circuitos
   e os atributos que faltam. Clicar num circuito seleciona os pontos dele no desenho.
5. **Dimensionar quadro...** (`ELEQUADRO`) abre o quadro de cargas: corrente, seção, disjuntor e
   balanceamento de fases.
6. **Ajustar fiação** (próxima etapa) vai escrever fiação e bitolas sobre os eletrodutos.

Os mesmos resultados saem em JSON pelos comandos `eletrica.analisar`, `eletrica.dimensionar` e
`eletrica.setup`, pelo canal de controle do CADCraft.

---

## Compilar e abrir

- `COMPILAR_CADCRAFT.bat` compila o fork (`release` como argumento gera a versão otimizada).
- `INICIAR_CRAFT_ELE.bat` abre o CADCraft compilado.

O toolchain Rust (variante GNU), o MinGW e a pasta de saída ficam em `C:\rust`, porque as
ferramentas GNU não aceitam o caractere acentuado do nome da pasta do usuário.

---

## Estado do dimensionamento (NBR 5410)

Feito: corrente de projeto, seção mínima (tabela 47), capacidade de condução (tabela 36,
cobre, PVC, método B1) com FCT e FCA informados, disjuntor com Ib ≤ In ≤ Iz e
balanceamento de fases.

Falta: queda de tensão, fator de agrupamento pelos eletrodutos, outros métodos de instalação,
DR, eletrodutos e a escrita no desenho. Os valores das tabelas devem ser conferidos contra a
norma vigente.

---

## Protótipo em Python

`src/`, `craftele_ui.py` e `poc_read_cadcraft_dxf.py` são o primeiro protótipo, que lia DXF
salvo e abria uma janela separada. Ficam como referência e não são mais o caminho principal.
