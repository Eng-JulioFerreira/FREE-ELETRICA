# Especificação do Padrão de Blocos Elétricos (CADCraft / NBR 5410)

Este documento estabelece as regras e convenções de nomenclatura para os blocos CAD (`BLOCK` e `INSERT`) e seus atributos (`ATTRIB`) no ecossistema do **CRAFT ELE / CADCraft**.

---

## 🎯 1. Filosofia de Modelagem

Para que o motor de cálculo da **NBR 5410** realize o dimensionamento e o roteamento de fiação de forma automatizada, cada elemento elétrico inserido no desenho deve carregar seus parâmetros essenciais como atributos vinculados.

Os atributos são case-insensitive (não diferenciam maiúsculas de minúsculas) e toleram sinônimos comuns.

---

## 📋 2. Estrutura Unificada de Atributos (Modelo Orientado a Objetos)

Seguindo o paradigma de **Programação Orientada a Objetos (POO)**, todo ponto elétrico deriva de uma mesma classe base (`PontoEletrico`). Portanto, **todos os blocos compartilham exatamente os mesmos atributos**:

| Atributo | Descrição | Exemplo Tomada / Iluminação | Exemplo Interruptor |
| :--- | :--- | :--- | :--- |
| `TIPO_PONTO` | Categoria do elemento | `TOMADA`, `TUG`, `TUE`, `ILUM` | `INTERRUPTOR`, `INT_SIMPLES` |
| `POTENCIA` | Potência nominal | `100W`, `600VA`, `5400W` | **`0W` (Sempre zerada: 0)** |
| `TENSAO` | Tensão nominal | `127V`, `220V` | `127V`, `220V` |
| `NUMERO_FASES` | Quantidade de fases | `1`, `2`, `3` | `1` |
| `CIRCUITO` | Circuito associado | `1`, `2` | `1`, `2` |
| `COMANDO` | Tecla / acionamento | `a` (em lâmpadas) ou vazio | **`a`, `b`, `c` (Obrigatório)** |
| `TAG` | Identificador individual | `TUG-01`, `LUM-01` | `INT-01` |

> [!IMPORTANT]
> **Regra do Interruptor em POO**:
> O interruptor possui **todos** os atributos da família de pontos elétricos. A sua potência é **sempre 0W** (zero), e a única diferença operacional é que ele possui o atributo `COMANDO` preenchido indicando a tecla de acionamento da lâmpada.


---

### C. Quadros de Distribuição (`QD`)
Ponto de origem e alimentação dos circuitos terminais.

| Atributo | Descrição | Exemplos Válidos |
| :--- | :--- | :--- |
| `TAG` | Nome do quadro | `QD-01`, `QGBT`, `QDF` |
| `TENSAO` | Tensão de fornecimento | `220/127V`, `380/220V` |
| `FASES` | Fases de entrada do quadro | `2` (Bifásico), `3` (Trifásico) |
| `DISJUNTOR_GERAL` | Proteção geral pré-definida (opcional) | `50A`, `63A`, `AUTO` |

---

## 🔎 3. Normalização e Validação no Adaptador

O adaptador do CRAFT ELE realiza:
1. Normalização automática de strings: remove espaços em excesso e converte para maiúsculas internamente.
2. Extração de valores numéricos de potência (ex: extrai `600` de `'600VA'` ou `'600 W'`).
3. Validação de consistência: emite alerta caso um interruptor contenha potência indevida ou uma tomada não contenha tensão/circuito especificado.
