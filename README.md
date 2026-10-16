# CRAFT ELE - Addon de Instalações Elétricas (NBR 5410) para CADCraft

O **CRAFT ELE** é uma extensão/addon de engenharia elétrica projetada especificamente para o **CADCraft**, funcionando de forma análoga a extensões profissionais de CAD (como PRO-Elétrica e Lumine).

---

## 🎯 Proposta e Fluxo do Addon

O usuário projeta normalmente dentro do **CADCraft**:
1. **Lançamento no CADCraft**: O projetista insere os blocos (quadros, tomadas, interruptores, luminárias) e desenha o traçado dos eletrodutos no CADCraft.
2. **Disparo do Addon**: Pelo próprio CADCraft (via comando AutoLISP na linha de comando, ex: `CRAFTELE`), o addon é acionado sobre o desenho ativo.
3. **Processamento NBR 5410**: O motor analisa a topologia do circuito, calcula potência, corrente, fatores de agrupamento/temperatura, queda de tensão, dimensiona condutores, disjuntores e eletrodutos.
4. **Retorno Automático no Desenho**: O addon insere de volta no desenho do CADCraft as indicações de fiação (fase, neutro, terra, retornos), as etiquetas de circuito e desenha o Quadro de Cargas executivo.

---

## 🔌 Arquitetura de Integração com o CADCraft

```text
       ┌────────────────────────────────────────────────────────┐
       │                CADCraft (Aplicação CAD)                │
       │  - Interface de Desenho (Modelspace)                   │
       │  - Comandos AutoLISP (acaddoc.lsp em %APPDATA%/support)│
       │  - Canal de Controle IPC / MCP (JSON line 127.0.0.1)   │
       └───────────────────────────┬────────────────────────────┘
                                   │
              AutoLISP / IPC / DXF │ (Comunicação bidirecional)
                                   ▼
┌───────────────────────────────────────────────────────────────────────┐
│                     CRAFT ELE (Motor / Addon)                         │
│                                                                       │
│  [src/adapters/]  Adaptador CADCraft (Leitura/escrita de entidades)  │
│         │                                                             │
│  [src/graph/]     Grafo Topológico da Instalação (NetworkX)          │
│         │                                                             │
│  [src/core/]      Cálculos Puros da NBR 5410 (Bitolas, Queda, DR)    │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 📁 Estrutura do Repositório

```text
CRAFT ELE/
├── cadcraft/                     # Repositório do CADCraft (código-fonte de referência)
├── src/
│   ├── adapters/                 # Ponte de comunicação com o CADCraft (DXF / IPC)
│   ├── graph/                    # Montagem da malha elétrica e traçado dos circuitos
│   └── core/                     # Dimensionamento normativo NBR 5410
├── tests/                        # Testes automatizados de validação
├── pyproject.toml
└── requirements.txt
```
