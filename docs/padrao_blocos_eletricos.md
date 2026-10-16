# Especificação do Padrão de Blocos Elétricos (CADCraft / NBR 5410)

Este documento estabelece as regras e convenções de nomenclatura para os blocos CAD (`BLOCK` e `INSERT`) e seus atributos (`ATTRIB`) no ecossistema do **CRAFT ELE / CADCraft**.

---

## 🎯 1. Filosofia de Modelagem

Para que o motor de cálculo da **NBR 5410** realize o dimensionamento e o roteamento de fiação de forma automatizada, cada elemento elétrico inserido no desenho deve carregar seus parâmetros essenciais como atributos vinculados.

Os atributos são case-insensitive (não diferenciam maiúsculas de minúsculas) e toleram sinônimos comuns.

---

## 📋 2. Estrutura de Atributos por Categoria de Bloco

### A. Pontos de Carga (Tomadas TUG/TUE, Iluminação, Cargas Especiais)
Elementos que consomem potência elétrica e demandam condutores ativos (Fase, Neutro, Terra).

| Atributo | Descrição | Exemplos Válidos | Sinônimos Aceitos |
| :--- | :--- | :--- | :--- |
| `TIPO` | Tipo do ponto ou finalidade | `TUG`, `TUE`, `ILUM`, `CHUVEIRO`, `AR_COND` | `TYPE`, `CATEGORIA` |
| `POTENCIA` | Potência nominal (W ou VA) | `100VA`, `600VA`, `5400W`, `100`, `600` | `POT`, `POWER`, `W`, `VA` |
| `TENSAO` | Tensão nominal de operação | `127V`, `220V`, `127`, `220` | `VOLTS`, `VOLTAGEM`, `TENSÃO` |
| `FASES` | Quantidade de fases (1, 2 ou 3) | `1`, `2`, `3`, `MONO`, `BI`, `TRI` | `NUM_FASES`, `FASE`, `F` |
| `CIRCUITO` | Identificador do circuito | `1`, `2`, `ILUM-01`, `TUG-02` | `CIRC`, `CIRCUIT` |
| `TAG` | Identificador individual do ponto | `TUG-1`, `LUM-3`, `CH-01` | `ID`, `NOME` |

> [!NOTE]
> Se a unidade de potência for omitida (ex: `600`), o motor assume `VA` para iluminação/TUG e `W` para cargas puramente resistivas (conforme fator de potência da NBR 5410).

---

### B. Pontos de Comando (Interruptores, Pulsadores, Sensores)
Elementos de manobra que seccionam condutores. **Não são receptores de carga**.

| Atributo | Descrição | Exemplos Válidos | Regra de Negócio |
| :--- | :--- | :--- | :--- |
| `TIPO` | Tipo do interruptor | `INT_SIMPLES`, `INT_PARALELO`, `INT_INTERMEDIARIO` | Obrigatório |
| `COMANDO` | Letra/código da lâmpada acionada | `a`, `b`, `c`, `s1`, `A` | **Obrigatório** |
| `CIRCUITO` | Circuito da lâmpada que ele aciona | `1`, `2` | Obrigatório |
| `TAG` | Identificador do interruptor | `SW-01`, `INT-a` | Opcional / Recomendado |
| `POTENCIA` | **NÃO DEVE EXISTIR** | *(ausente)* | **Interruptores não possuem potência** |

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
