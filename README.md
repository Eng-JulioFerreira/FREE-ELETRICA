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
   atributos do [padrão de blocos](docs/padrao_blocos_eletricos.md). O template
   `templates/CRAFT_ELE_BLOCOS.dxf` já traz os blocos INT-SIM, TOM-SIM-BAIXA, QDG e ILU-OCT e
   as camadas de eletroduto PVC linha média de teto (contínua) e de piso (tracejada). Ele é
   gerado por `ferramentas/gerar_template.py` a partir de `templates/fontes/simbolos_referencia.dxf`.
2. A paleta **Elétrica** já abre com o programa, acima das Tool Sets. Ela só tem ferramentas.
   Se for fechada, volta com `ELETRICA` ou **Janela > Elétrica**.
3. **Configurações...** (`ELECONFIG`) é a aba de setup, com as premissas globais:
   - Geral: sistema de distribuição (padrão 380/220 V), pé-direito, unidade do desenho,
     camadas dos eletrodutos e raio de captura;
   - Condutores: temperatura do projeto (dá o FCT), queda de tensão admissível, fator para
     materiais, cores das fases A, B e C e seções omitidas nas indicações;
   - Agrupamento: o FCA é automático, pelo número de circuitos em cada trecho de eletroduto;
   - Telecom: catálogo de cabos (UTP Cat 5/6, fibra N2/N4/N12, CCI 1P a 4P, CI 10P/20P,
     coaxial RG 59) e cadastro de cabos novos.
4. **Criar circuito** (`ELECIRCUITO`): selecione o quadro de destino, informe o número (Enter
   aceita o próximo livre) e selecione os pontos do circuito, um a um.
5. **Comandos e luminárias** (`ELECOMANDO`): selecione o interruptor e depois as luminárias que
   ele comanda. Elas recebem a letra e o circuito dele.
6. **Conferir pontos e circuitos...** (`ELECONFERIR`) lista pontos, circuitos e atributos que
   faltam. Clicar num circuito seleciona os pontos dele no desenho.
7. **Rotear fiação...** (`ELEROTEAR`): escolha o quadro e clique em Rotear. Os circuitos são
   roteados pelos eletrodutos (caminho mínimo do quadro a cada ponto, com subidas e descidas)
   e aparecem na tabela de resumo: fatores de correção, comprimento, queda de tensão calculada
   e admitida, pré-seleção automática de seção e disjuntor, fases e indicação de fiação. Seção e
   disjuntor podem ser sobrescritos na própria tabela; a escolha fica gravada no quadro e é
   conferida contra a norma. **Copiar resumo** gera o resumo técnico em texto.

Os mesmos resultados saem em JSON pelo canal de controle do CADCraft: `eletrica.analisar`,
`eletrica.rotear`, `eletrica.sobrescrever`, `eletrica.setup`, `elecircuito` e `elecomando`.

---

## Compilar e abrir

- `COMPILAR_CADCRAFT.bat` compila o fork (`release` como argumento gera a versão otimizada).
- `INICIAR_CRAFT_ELE.bat` abre o CADCraft compilado.

O toolchain Rust (variante GNU), o MinGW e a pasta de saída ficam em `C:\rust`, porque as
ferramentas GNU não aceitam o caractere acentuado do nome da pasta do usuário.

---

## Estado do dimensionamento (NBR 5410)

Feito: corrente de projeto, seção mínima (tabela 47), capacidade de condução (tabela 36,
cobre, PVC, método B1), FCT pela temperatura do projeto (tabela 40), FCA pelo agrupamento nos
eletrodutos (tabela 42), queda de tensão pelo momento elétrico com ρ do cobre a 70 °C,
disjuntor com Ib ≤ In ≤ Iz, balanceamento de fases e levantamento de cabo por seção.

Premissas do roteamento: eletrodutos são linhas e polilinhas nas camadas configuradas; os das
camadas de piso (padrão `*PISO*`) correm no chão e os demais no pé-direito, e a passagem do piso
ao teto num mesmo ponto soma um pé-direito; linhas que se cruzam sem vértice comum não se ligam; uma ponta encostada no meio
de outro trecho liga (derivação). Ponto fora da rede tem o comprimento estimado em L a partir do
quadro, marcado com `*` na tabela. O FCA conta só os circuitos do quadro roteado.

Falta: escrever a fiação no desenho, retornos de interruptor no levantamento, outros métodos de
instalação e isolações, DR, curto-circuito e taxa de ocupação dos eletrodutos (inclusive com
os cabos de telecom). Os valores das tabelas devem ser conferidos contra a norma vigente.

---

## Protótipo em Python

`src/`, `craftele_ui.py` e `poc_read_cadcraft_dxf.py` são o primeiro protótipo, que lia DXF
salvo e abria uma janela separada. Ficam como referência e não são mais o caminho principal.
