# Resumo da auditoria de desempenho de linhas

[Relatório completo](benchmarks/line-performance/report.md) ·
[Resultados e método](benchmarks/line-performance/environment.json)

**Vale otimizar as implementações atuais. Não há justificativa, por enquanto,
para reescrever o motor com rope ou uma lista persistente de linhas.**

- **Apagar uma linha não realoca mil objetos de linha.** O texto é contínuo;
  atualmente `dln` faz duas varreduras e cria outra string. Uma exclusão é O(N).
  Em 1.000 linhas de 64 bytes, mediu 126 µs; o candidato por limites e cópia em
  blocos, 1,2 µs. Isso é tempo do transform, sem carregar/gravar o arquivo.
- **Em arquivos grandes, as cópias são relevantes.** Com 64 MiB e entrada própria,
  `dln` passou de 133 ms para 7,2 ms nas linhas longas. No teste de memória com
  linhas curtas, o pico caiu de aproximadamente 131 MiB para 67 MiB. Ainda há um deslocamento do sufixo; não virou O(1).
- **`revln` merece atenção especial.** Em 8 MiB só de quebras de linha, seu índice
  levou o pico a 275 MiB. Offsets compactos reduziram para 83 MiB; sem índice,
  para 19 MiB. A versão sem índice ficou mais lenta em linhas longas, portanto
  não deve substituir o caminho atual indiscriminadamente.
- **Buscar apenas o trecho necessário é o ganho mais simples.** `dlna`/`slnc`
  contam todo o arquivo mesmo quando só usam duas linhas iniciais. Parar cedo
  remove esse trabalho; a estratégia de retorno deve considerar ownership.
- **Reutilizar buffers nem sempre economiza RAM ao final.** Selecionar 8 KiB de
  uma string própria de 64 MiB com `truncate` manteve cerca de 67 MiB residentes.
  Busca curta + cópia exata ficou 14× mais rápida que a versão atual e liberou
  o buffer grande. Capacidade reservada não é o mesmo que RAM física.
- **Há cópias desnecessárias quando nada muda**, além de criação de tokens
  temporários em `pln` e na inserção no EOF. São melhorias locais possíveis.
- **`uniqln` também usa memória por linha única.** O conjunto já guarda fatias,
  não cópias independentes de cada linha. Remover essa memória mantendo
  deduplicação exata e ordem não é uma troca trivial.
- **Muitas pequenas exclusões acumulam custo.** Em 8 MiB, 50 `dln 2` custaram
  823 ms; uma única `dlni 2 51` existente, 19 ms. A troca só vale se não forem
  necessários estados intermediários, watchers ou outras observações.
- **Buffered ainda pode materializar tudo.** O threshold de spool não limita
  globalmente a RAM dessas instruções; isso exige trabalho específico de streaming.

## Ordem sugerida

1. Buscas curtas + cópias em blocos + uso criterioso de `Cow`/`Owned`.
2. Índice compacto em `revln` e helpers de append/insert sem tokens temporários.
3. Fast paths para operações sem alteração e alocação tardia dos filtros.
4. Depois, scanner mais rápido, streaming e heurísticas conforme cargas reais.

As primeiras famílias têm custo estimado de **poucos dias cada**, com testes;
streaming e uma nova representação do documento custam bem mais. São estimativas
de engenharia, não prazos garantidos. Para poucas operações em textos pequenos,
pode não compensar toda a sofisticação; para vários MiB ou muitas operações,
os ganhos locais medidos justificam a mudança.

Foram coletadas **1.760 amostras de tempo e 195 de memória**. Os protótipos passaram
por 29.284 combinações de entrada/ownership, além de comparações de cadeias.
Medições locais, em release e uma CPU fixada; não incluem I/O, Python ou latência
completa do processador. Nenhuma implementação de produção ou binding foi alterada.
Os candidatos ainda precisam preservar integralmente os contratos de erro antes
que possam virar patches de produção.
