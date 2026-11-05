# Especificação técnica e de UX/UI atualizada: Free Elétrica 2.0 (CADCraft / Rust / egui)

Texto da especificação como foi passado. O estado da implementação e os limites estão em
[free-eletrica-2.0.md](free-eletrica-2.0.md).

## 1. Diretrizes gerais e filosofia

Refatoração completa do plugin **Free Elétrica** para CADCraft (Rust + egui), unindo a velocidade do CAD 2D, a lógica de famílias do **Revit** e o fluxo de engenharia estruturado do **FElétrico / AuE**.

### Princípios inegociáveis

1. **Motor headless desacoplado:** lógica de grafo, roteamento e cálculos NBR 5410 isolados em Rust, acessíveis via comandos, CLI, JSON e MCP.
2. **Persistência 100% no DXF:** todo estado vive no arquivo `.dxf` (`XDATA` sob `AppID: FREE_ELETRICA` e atributos de blocos).
3. **Janelas independentes e sob demanda:** nenhuma barra ou painel inferior fica fixo poluindo a tela. Todas as ferramentas abrem em `egui::Window` flutuantes e fecháveis (`[X]` / `ESC`), preservando a viewport limpa.
4. **Separação rígida de etapas:** a modelagem lógica e física (pontos, circuitos, eletrodutos) é independente do **Dimensionamento** e da **Documentação**.

## 2. Ajustes críticos solicitados nesta versão

### A. Traçado de eletrodutos: retos e arqueados (`ELEELETRODUTO`)

- A ferramenta de eletroduto deve permitir ao projetista escolher na mini-barra superior o modo de traçado:
  - **Modo reto / ortogonal:** linhas e polilinhas ortogonais conectando o nó de origem ao nó de destino.
  - **Modo arqueado / curvo (`ARC` / `SPLINE` suave):** permite desenhar desvios curvilíneos contornando pilares ou paredes curvas, mantendo o vínculo estrito (`from_handle` e `to_handle`) nos nós do grafo nas extremidades.
- O cálculo 3D e a taxa de ocupação dos cabos aplicam-se igualmente a trechos retos ou arqueados (calculando o comprimento real do arco/polilinha + desníveis Z).

### B. Comando de dimensionamento separado e avançado (`ELEDIMENSIONAR`)

- **Independência do fluxo:** o dimensionamento **não** ocorre automaticamente a cada clique na planta. Ele é acionado por um comando/botão explícito: `[ Dimensionar Projeto ]`.
- **Janela de opções avançadas de dimensionamento:** antes (ou durante) o processamento, o engenheiro tem uma janela dedicada para configurar premissas avançadas da NBR 5410:
  - **Opção de compartilhamento de condutor de proteção (Terra / PE):** permitir ou não compartilhar o mesmo cabo PE para circuitos que passam pelo mesmo eletroduto/eixo principal (otimizando a quantidade de cabos na tubulação).
  - Ajustes de fatores de correção (temperatura, agrupamento FCA, etc.).
  - Quando o usuário clica em `[ Processar Cálculo ]`, o motor varre o grafo, calcula o FCA real por trecho, valida quedas de tensão e atualiza a tabela do Quadro de Cargas.

### C. Melhoria na UX de criação de blocos e editor visual de atributos

1. **Botão claro de criação:** na janela de gerenciamento de blocos, incluir um botão evidente e destacado: `[ + Criar Novo Bloco Elétrico ]`.
2. **Editor visual de atributos (sem linhas de código):**
   - Para evitar que o usuário precise editar atributos manualmente por texto ou código cru, o plugin deve fornecer um **Inspetor de Atributos** visual.
   - Ao selecionar um bloco (ou um Quadro / `INSERT`), a janela exibe de forma amigável os campos e tags (`CIRCUITO`, `POTENCIA`, `ALTURA`, `COMANDO`), permitindo alterar valores, visibilidade e posição gráfica de forma intuitiva por inputs visuais do `egui`.

### D. Empacotamento de distribuição e documentação no repositório

- **Organização de releases:** o repositório deve incluir uma pasta `/docs/` contendo o manual de instalação (`INSTALL.md`), o guia de primeiros passos (`GETTING_STARTED.md`) e esta especificação atualizada.
- **Script de build/release (sem arquivos `.bat` na ponta):** configurar uma rotina/script de build em Rust (ou comando de CI/local) que compile a versão otimizada (`--release`) e empacote automaticamente em uma pasta de distribuição limpa contendo o executável do CADCraft, a pasta de templates DXF e o manual, gerando um `.zip` pronto para uso por novos usuários (eliminando qualquer dependência de scripts `.bat` manuais para quem vai apenas utilizar o software).

## 3. Resumo dos módulos principais do plugin

1. **`ELECONFIG` (Setup):** bootstrap automático de layers/blocos e definição de cotas padrão de legenda e critérios elétricos.
2. **`ELEPONTOS` (mini-janela superior):** inserção ágil de pontos filtrados por categoria com aplicação automática da regra do `h=X,XXm` quando a altura divergir do setup.
3. **`ELECIRCUITO` e `ELECOMANDO`:** vinculação lógica de pontos e interruptores às luminárias (letras `a`, `b`, `c`...) com modo de inspeção visual (overlay colorido na viewport).
4. **`ELECARGAS` (Quadro de Cargas em 2 abas):**
   - *Aba 1:* resumo operacional limpo, divisão de fases (R, S, T), grupos DR e status.
   - *Aba 2:* ficha completa de engenharia (método de instalação, tipo de isolação de PVC 70 °C vs HEPR 90 °C por circuito, FCA, queda de tensão e travas manuais de override).
5. **`ELEELETRODUTO` e `ELEROTEAR`:** grafo estrito 3D (zero conexão em cruzamentos sem caixa), suporte a traçados retos e arqueados, e seleção de tipos de eletroduto (corrugado leve/reforçado, rígido, galvanizado).
6. **`ELEFIACAO` e `ELEMATERIAIS`:** tags de fiação não destrutivas (preservam posição ao atualizar) e extração de lista quantitativa de materiais.
