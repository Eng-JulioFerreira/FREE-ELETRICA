# Política de Versionamento Git - CRAFT ELE

Para garantir a rastreabilidade, integridade do código e evitar perda de histórico ou de funcionalidades já desenvolvidas, o projeto adota as seguintes diretrizes de versionamento:

---

## 📌 1. Princípios Fundamentais

1. **Testes Antes do Commit**:
   - Nenhum commit deve ser realizado com a suíte de testes (`pytest`) falhando.
   - Qualquer nova funcionalidade deve vir acompanhada ou seguida de teste unitário correspondente.

2. **Commits Atômicos e Frequentes**:
   - Realizar commits a cada etapa concluída (ex: novo leitor de atributos, janela gráfica criada, novo cálculo de bitola implementado).
   - Evitar acumular múltiplas tarefas distintas em um único commit.

3. **Padrão de Mensagens de Commit (Conventional Commits)**:
   - `feat:` Nova funcionalidade (ex: `feat: janela gráfica de controle elétrico`)
   - `fix:` Correção de bug (ex: `fix: leitura de atributos com letras minúsculas`)
   - `docs:` Alterações na documentação (ex: `docs: especificação do padrão de blocos elétricos`)
   - `test:` Adição ou modificação de testes (ex: `test: validação de blocos com atributos de fase e tensão`)
   - `refactor:` Melhoria de código sem alterar comportamento externo

---

## 🌿 2. Estratégia de Branches

- **`main`**: Ramo principal que contém as versões estáveis e operacionais do Addon.
- **Tags de Versão**: A cada marco concluído (ex: `v0.1.0-poc-dxf`, `v0.2.0-ui-companion`), criar uma tag anotada (`git tag -a vX.X.X -m "..."`).
