# Instalar o Free Elétrica

O Free Elétrica é um programa de projeto elétrico predial de baixa tensão (NBR 5410), baseado no
CADCraft. O pacote não tem instalador e não precisa de Rust.

## Requisitos

- Windows 10 ou 11, 64 bits.
- Placa de vídeo com OpenGL.

## Instalação

1. Baixe o arquivo `FreeEletrica-v<versão>-win64.zip` na [página de versões](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest).
2. Extraia para uma pasta sua, como `Documentos\FreeEletrica`.
3. Clique duas vezes em `FreeEletrica.exe`. Na primeira vez, o Windows pode mostrar um aviso de aplicativo desconhecido,
   porque o executável não é assinado: clique em **Mais informações** e **Executar assim mesmo**.

A paleta **Elétrica** abre sozinha. Se tiver sido fechada, digite `ELETRICA` na linha de comando ou
use o menu **Janela**.

## O que vem no pacote

| Item | O quê |
| :--- | :--- |
| `FreeEletrica.exe` | O programa |
| `templates/FREE_ELETRICA_BLOCOS.dxf` | Template opcional de blocos; os blocos base entram sozinhos em qualquer desenho |
| `LEIA-ME.md` | O resumo para começar |
| `INSTALL.md`, `GETTING_STARTED.md` | Instalação e primeiros passos |
| `licencas/` | Licenças do Free Elétrica e do CADCraft |

## Atualizar e remover

Para atualizar, troque a pasta pela da versão nova. Para remover, apague a pasta. Seus desenhos ficam
onde você os salvou, e os dados elétricos ficam dentro de cada DXF.

As preferências do programa, como o layout das janelas e as configurações elétricas, ficam na pasta de
configuração do seu usuário e não são apagadas junto.

## Para quem desenvolve

O pacote é gerado com `scripts\package_release.ps1`, que chama `cargo xtask dist`. O resultado sai em `dist/`. Veja
`CLAUDE.md` para o ambiente de compilação.
