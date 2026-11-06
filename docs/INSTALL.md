# Instalar o Free Elétrica

O Free Elétrica é um programa de projeto elétrico predial de baixa tensão (NBR 5410) e de cabeamento estruturado
(NBR 14565), baseado no CADCraft. Há um pacote para Windows e outro para Linux, na
[página de versões](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest). Nenhum tem instalador nem
precisa de Rust.

## Requisitos

- **Windows:** 10 ou 11, 64 bits.
- **Linux:** x86_64, com glibc 2.35 ou mais nova (Ubuntu 22.04, Debian 12, Fedora 36, Mint 21 ou mais novos), X11 ou
  Wayland.
- Placa de vídeo com Vulkan, DirectX 12 (Windows) ou OpenGL.

## Instalação no Windows

1. Baixe o arquivo `FreeEletrica-v<versão>-win64.zip` na [página de versões](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest).
2. Extraia para uma pasta sua, como `Documentos\FreeEletrica`.
3. Clique duas vezes em `FreeEletrica.exe`. Na primeira vez, o Windows pode mostrar um aviso de aplicativo desconhecido,
   porque o executável não é assinado: clique em **Mais informações** e **Executar assim mesmo**.

## Instalação no Linux

1. Baixe o arquivo `FreeEletrica-v<versão>-x86_64.AppImage` na mesma [página de versões](https://github.com/Eng-JulioFerreira/Free-Eletrica/releases/latest).
2. Marque como executável: no gerenciador de arquivos, **Propriedades → Permitir executar como programa**, ou no terminal:
   ```
   chmod +x FreeEletrica-v<versão>-x86_64.AppImage
   ```
3. Clique duas vezes no arquivo, ou rode `./FreeEletrica-v<versão>-x86_64.AppImage` no terminal.

O AppImage é um arquivo só, com o programa, os manuais, o template de blocos e as licenças dentro. Ele não precisa da
biblioteca FUSE. Para ver o que tem dentro: `./FreeEletrica-v<versão>-x86_64.AppImage --appimage-extract`.

A paleta **Elétrica** abre sozinha. Se tiver sido fechada, digite `ELETRICA` na linha de comando ou
use o menu **Janela**.

## O que vem no pacote

No Windows, dentro da pasta do `.zip`; no Linux, dentro do AppImage, em `usr/share/free-eletrica`:

| Item | O quê |
| :--- | :--- |
| `FreeEletrica.exe` (Windows) ou `FreeEletrica` (Linux) | O programa |
| `templates/FREE_ELETRICA_BLOCOS.dxf` | Template opcional de blocos; os blocos base entram sozinhos em qualquer desenho |
| `LEIA-ME.md` | O resumo para começar |
| `INSTALL.md`, `GETTING_STARTED.md` | Instalação e primeiros passos |
| `licencas/` | Licenças do Free Elétrica e do CADCraft |

## Atualizar e remover

Para atualizar, troque a pasta (Windows) ou o arquivo AppImage (Linux) pelo da versão nova. Para remover, apague a pasta
ou o arquivo. Seus desenhos ficam onde você os salvou, e os dados elétricos ficam dentro de cada DXF.

As preferências do programa, como o layout das janelas e as configurações elétricas, ficam na pasta de
configuração do seu usuário e não são apagadas junto.

## Para quem desenvolve

O pacote do Windows é gerado com `scripts\package_release.ps1`, que chama `cargo xtask dist`; o do Linux, com
`scripts/package_appimage.sh`, num Linux x86_64. O resultado sai em `dist/`. O workflow **Release** do GitHub Actions gera
os dois e publica a versão. Veja `CLAUDE.md` para o ambiente de compilação.
