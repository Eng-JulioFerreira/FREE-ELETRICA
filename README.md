# CRAFT ELE

Projeto elétrico predial de baixa tensão (NBR 5410) e telecom dentro do
[CADCraft](https://github.com/storytold/cadcraft), no estilo do OFE Elétrico, do PRO-Elétrica e
do Lumine. Aparece como a paleta **Elétrica**, acoplada à interface do CADCraft.

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

O CADCraft é MIT OR Apache-2.0 (`cadcraft/LICENSE-MIT`, `cadcraft/LICENSE-APACHE`, `cadcraft/NOTICE`);
esses avisos precisam acompanhar o repositório. A licença do código próprio do CRAFT ELE ainda
não foi definida.
