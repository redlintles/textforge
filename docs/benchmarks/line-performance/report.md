# Auditoria de desempenho das instruções de linhas

Data: **31/10/2026, America/Sao_Paulo**. Revisão auditada:
`3e87c17b623f470be93df9b1f4fcb3300efe167a` (TextForge 0.2.0).

[Resumo e prioridades](../../line-performance-summary.md) ·
[Tempos completos](timings.json) · [Amostras de tempo](timings-raw.json) ·
[Memória](rss.json) · [Experimentos complementares](extras-timings.json) ·
[Memória complementar](extras-rss.json) · [Ambiente](environment.json)

## Conclusão

**Justifica-se otimizar a implementação atual, mas não substituir agora todo o
motor por ropes, listas de linhas ou uma árvore de edição.** Há ganhos grandes
com buscas limitadas ao intervalo necessário, cópias em blocos e aproveitamento
de `Cow`/`String` já previstos no contrato existente. Não houve modificação em
`src/`, nas APIs públicas, no bytecode ou no binding Python nesta auditoria.

Dois cuidados impedem uma recomendação indiscriminada de “tudo in-place”:

- `revln` tem um risco de memória proporcional à quantidade de linhas; um índice
  compacto se mostrou um compromisso melhor que sempre ler o texto de trás para frente.
- Truncar um buffer próprio pode manter quase todo o documento em RAM. Para
  cortes grandes, copiar só o trecho pequeno pode ser a escolha melhor.

## 1. O que acontece ao apagar a linha 2

O documento é uma `String` UTF-8 contínua. As linhas são fatias temporárias
`(&str, &str)`, com conteúdo e terminador. Não há mil objetos de linha cujos
números precisem ser atualizados individualmente.

A implementação atual de `dln`:

1. Lê todo o texto para contar as linhas e validar o índice.
2. Reserva um novo buffer com capacidade igual ao tamanho da entrada.
3. Percorre as linhas de novo, copiando as sobreviventes para esse buffer.

Portanto, **uma exclusão é O(N), não O(L²)**: não há uma realocação para cada linha.
Há um novo buffer de saída e cópia dos bytes sobreviventes. Para uma entrada
comum grande, entrada e saída coexistem e o pico se aproxima de dois documentos.

Em uma string própria, `String::drain` pode compactar o sufixo no mesmo buffer.
Isso ainda desloca os bytes posteriores uma vez: excluir perto do início continua
O(N). Com entrada emprestada e um corte interior, uma única `Cow<str>` não pode
representar duas fatias desconexas; construir uma saída contínua exige cópia.

Para **1.000 linhas de 64 bytes**, `dln 2` custou **126.14 µs**
na versão atual e **1.19 µs** no protótipo com
busca curta e cópia em dois blocos. Isso não significa que qualquer arquivo de
1.000 linhas terá esse custo: o tamanho em bytes e o comprimento das linhas importam.

O risco de repetição é outro: K exclusões sucessivas custam aproximadamente
O(KN), ou, mais precisamente, O(Σ Nᵢ). Deletar todas as linhas uma por vez pode
produzir comportamento quadrático em relação ao tamanho inicial.

## 2. Método e limites da medição

- Build `--release --locked`, Rust 1.99.1, Linux x86_64, AMD Ryzen 5 4600G.
  Cada processo foi fixado à CPU registrada em [environment.json](environment.json).
- 1.610 amostras na matriz principal e 150 complementares: cinco amostras por
  configuração, em ordem embaralhada com seed fixa. As tabelas mostram medianas;
  os JSONs incluem quartis, mínimo, máximo e as amostras individuais.
- 100 operações por amostra pequena, duas em 8 MiB e uma em 64 MiB; nos estudos
  complementares, 20 operações pequenas ou uma grande.
- Construção dos dados, preparação do token/regex na matriz principal e aquisição
  da string própria ficam **fora** do tempo da transformação. O descarte da saída
  também fica fora; liberações internas da entrada ficam dentro.
- `borrowed`: `Cow::Borrowed`; `owned`: buffer próprio com capacidade inicial
  igual ao comprimento; `spare`: buffer próprio com 256 bytes extras.
- A memória usa processos novos: 171 amostras de pico via `/usr/bin/time`, mais
  24 complementares (três por configuração). O pico inclui criação da entrada. RAM após a operação vem
  de `/proc/self/smaps_rollup`; capacidade de `String` é medida separadamente.
- Dados ASCII alternam `keep`/`drop` e possuem conteúdo distinto por linha.
  Filtros `^keep` selecionam/removem aproximadamente metade das linhas.
  `dle` não encontra linhas vazias nesse conjunto; `uniqln` encontra linhas únicas.
- Também foram medidos Unicode com todos os terminadores, texto sem quebras,
  linhas repetidas e texto formado apenas por LF. Não são amostras de arquivos
  reais de produção e a máquina não estava dedicada exclusivamente ao benchmark.
- **Não** se mediram disco, decodificação de arquivos, conversão Python, callbacks,
  watchers ou latência completa do processador. Cadeias complementares usam os
  kernels diretamente, com o mesmo resultado textual, sem o interpretador.
- Não foram contadas chamadas de `malloc/realloc`: capacidade e identidade de
  ponteiro não substituem um contador de alocações. Nenhum hook com `unsafe` foi usado.

Uma busca curta que devolve uma fatia não lê do disco um arquivo de 64 MiB em
microssegundos: ela opera sobre uma entrada que já está na memória. APIs que
retornam `String` ainda materializam uma fatia emprestada na fronteira de retorno.

### Verificação semântica dos experimentos

O harness comparou candidatos com os tokens reais em **14.642 entradas × dois
modos de ownership = 29.284 casos**, incluindo entrada vazia, todos os oito
terminadores, CRLF atômico, emoji, combinantes e sequências UTF-8 que se parecem
com prefixos de separadores. Também comparou todos os intervalos válidos de cada
entrada e cadeias de exclusão com uma exclusão de intervalo equivalente.

Verificação: [semantic-check.json](semantic-check.json). Os candidatos são
experimentos, **não patches prontos**: seus erros usam rótulos genéricos. Para
produção ainda é necessário preservar códigos, mensagens, contexto, precedência
de erros, preparação, watchers, buffered e paridade das APIs. A matriz não é uma
prova de desempenho em outros sistemas, alocadores ou padrões regex.

## 3. Tempos atuais das 16 instruções

Tempos em **ms**, entrada emprestada. Os tamanhos são bytes do documento,
incluindo terminadores. Índices no fim e seleções de todo o texto também aparecem
no JSON; ganhos de busca curta diminuem quando é necessário atravessar quase N. As seleções/cortes são perto do início: linha 2 ou linhas
2–3; os textos de inserção/substituição são pequenos.

| Instrução | 1.000 linhas × 64 B (62,5 KiB) | 8 MiB / 131.072 linhas × 64 B | 64 MiB / 16.384 linhas × 4 KiB |
| --- | ---: | ---: | ---: |
| `apl` | 0.064 | 12.674 | 83.603 |
| `dln` | 0.126 | 20.191 | 128.462 |
| `dlna` | 0.063 | 8.311 | 50.956 |
| `dlnb` | 0.064 | 12.415 | 83.903 |
| `slnc` | 0.074 | 9.324 | 66.656 |
| `dlni` | 0.128 | 19.609 | 127.628 |
| `inl` | 0.073 | 13.237 | 99.757 |
| `nln` | 0.069 | 12.390 | 81.149 |
| `rln` | 0.137 | 21.687 | 131.625 |
| `pln` | 0.074 | 13.392 | 100.267 |
| `dle` | 0.076 | 13.578 | 93.093 |
| `klm` | 0.088 | 13.684 | 66.584 |
| `dlm` | 0.088 | 13.755 | 65.920 |
| `revln` | 0.070 | 15.689 | 83.857 |
| `uniqln` | 0.154 | 50.520 | 126.501 |
| `joinln` | 0.080 | 13.398 | 95.401 |

Não comparar apenas as colunas como se a quantidade de linhas fosse fixa:
comprimentos e contagens mudam. Mais linhas também aumentam chamadas por linha,
o índice de reversão e a tabela de deduplicação.

## 4. Inventário dos problemas

N = bytes de entrada; L = linhas; U = conteúdos únicos; P = bytes até o trecho;
R = bytes sobreviventes; T = texto inserido/substituído; S = separador de junção.
“Varredura” abaixo refere-se ao scanner de linhas; cópia de bytes é custo adicional.

| Token | Trabalho/armazenamento atual | Diagnóstico e direção |
| --- | --- | --- |
| `apl` | Busca primeiro terminador, percorre até o último, copia entrada e pode crescer a saída | Eliminar a busca do último: testar o sufixo. Reservar resultado uma vez; reaproveitar `Owned`. |
| `dln` | Conta N + percorre N de novo; saída com capacidade N | Localizar apenas os dois limites; copiar prefixo/sufixo em blocos, ou um `drain` em `Owned`. |
| `dlna` | Conta N mesmo mantendo duas linhas; depois busca prefixo | Parar na linha solicitada. Borrow de prefixo, ou escolher entre truncamento e cópia pequena. |
| `dlnb` | Conta N, busca início e copia sufixo | Buscar somente o início. Fatia emprestada ou uma compactação do sufixo. |
| `slnc` | Conta N, percorre até o fim do intervalo e reserva N mesmo para R minúsculo | Busca limitada; saída exata ou view. Evitar reter buffer grande em `Owned`. |
| `dlni` | Conta N + reconstrói N | Mesma solução de `dln`, aplicada a um único intervalo; não executar vários drains pequenos. |
| `inl` | Conta N + busca ponto/estilo + copia N+T | Busca limitada e inserção única em buffer próprio. No EOF ainda delega a `apl`, com mais varredura e metadados temporários. |
| `rln` | Conta N + busca estilo + reconstrói N; reserva N+T sem descontar conteúdo removido | Localizar um intervalo e usar uma substituição em bloco; retornar entrada se conteúdo for idêntico. |
| `pln` | Constrói um `Inl` temporário e conta todo N para inserir em 1 | Não precisa validar a existência da primeira linha para essa operação. Usar helper privado de inserção/prepend. |
| `nln` | Lê N e copia os conteúdos; copia também quando não há quebras | Fast path sem alteração; `retain` em `Owned` pode reduzir o pico, mas exige avaliar capacidade retida e ainda faz varredura. |
| `dle` | Lê N e copia todas as linhas mantidas; sempre aloca | Alocação tardia no primeiro descarte; conservar `Cow` se nenhuma linha for vazia. |
| `klm` | Lê N, aplica regex por linha e sempre reserva saída N | Regex literal já é preparada; ganho provável está em saída preguiçosa e em retornar entrada quando tudo é mantido. |
| `dlm` | Idem, copiando linhas que não correspondem | Mesmo cuidado de `klm`; não compilar regex novamente por linha nem trocar a semântica de match. |
| `revln` | `Vec<(&str,&str)>` + saída N | Cada descritor mede 32 B neste alvo. Reduzir para offsets ou usar iteração reversa sob restrição de memória. |
| `uniqln` | HashSet de fatias, O(U) metadados, e saída de capacidade N | Não duplica cada linha no conjunto. Hash e memória de U continuam necessários na estratégia atual; saída preguiçosa ajuda no caso sem duplicatas. |
| `joinln` | Lê N e reserva N; saída pode ser maior com S longo | Resultado tem soma dos conteúdos + max(L−1, 0)×S. Crescimento grande pode ser inerente; reserva exata exige outra passagem/contagem. |

A construção temporária de `Inl`/`Apl` também clona parâmetros e payload em tempo
de execução, apesar da preparação do token externo. Isso é evitável com helpers
privados. O benchmark usou payload pequeno; **não** mediu textos de inserção grandes.

### Falhas e operações que não mudam nada

`check_line` e `check_range` copiam a entrada para o contexto do erro. Em falhas
sobre documentos grandes isso pode criar outra cópia N. Não recomendo remover
esse contexto ou mudar mensagens para obter desempenho; são contratos observáveis.
O caminho de sucesso pode parar cedo, contando tudo apenas se precisar produzir
o erro existente.

`dlnb 1`, `dlna última`, `slnc 1 última`, filtros que mantêm tudo e outras situações
sem mudança hoje também retornam uma nova string. Em pipelines longos, conservar
`Cow` entre passos evita várias cópias; a API externa pode continuar exigindo uma
única string própria ao final.

## 5. Experimentos e ganhos demonstrados

### 5.1 Busca limitada e uso do buffer próprio

8 MiB, linhas de 64 B; **ms por transformação**:

| Token | Atual borrowed | Protótipo borrowed | Atual owned | Protótipo owned |
| --- | ---: | ---: | ---: | ---: |
| `apl` | 12.674 | 3.965 | 12.883 | 0.002 |
| `dln` | 20.191 | 4.033 | 20.258 | 0.653 |
| `dlna` | 8.311 | 0.001 | 8.918 | 0.001 |
| `slnc` | 9.324 | 0.001 | 9.568 | 0.002 |
| `nln` | 12.390 | 12.367 | 12.909 | 9.988 |

Para `dln` com entrada emprestada, o ganho vem de eliminar as passagens completas
do scanner e copiar dois blocos, **não** de tornar o corte interior zero-copy.
O pico permaneceu próximo de dois documentos. Com 64 MiB e entrada própria,
o protótipo reduziu o pico de aproximadamente **131 MiB para 67 MiB** no conjunto de linhas de 64 B, deslocando
o sufixo uma vez no buffer existente.

`apl` passou de **83,603 para 32,662 ms** com 64 MiB emprestados. Em `owned`, o
alocador desta máquina conseguiu ampliar o buffer no lugar, e o tempo observado
foi de cerca de 5 µs. Esse caso favorável depende do alocador/espaço disponível;
**não é uma garantia de append O(1)**. Reservar de antemão ou aproveitar capacidade
existente evita o crescimento desnecessário observado na versão atual.

`nln` sem quebras, em 8 MiB emprestados, passou de **10,511 para 6,102 ms**,
retornando a mesma entrada. Com quebras presentes, a versão emprestada praticamente
empatou; a versão própria teve ganho mais moderado. Não há base para prometer um
ganho grande de CPU para todo filtro/retainer só por ser in-place.

### 5.2 Uma otimização ingênua pode reter 64 MiB

Seleção das linhas 2–3 de 64 MiB, linhas de 4 KiB, entrada **própria**:

| Estratégia | Tempo mediano | Comprimento da saída | Capacidade da saída | Pico RSS | RSS após retornar |
| --- | ---: | ---: | ---: | ---: | ---: |
| Atual `slnc` | 71,062 ms | 8 KiB | 64 MiB | 66,7 MiB | 2,9 MiB |
| Busca limitada + truncate/drain | 0,011 ms | 8 KiB | 64 MiB | 66,6 MiB | 66,7 MiB |
| Busca limitada + cópia exata | 5,092 ms | 8 KiB | 8 KiB | 66,6 MiB | 2,7 MiB |

A saída atual reserva muita memória virtual, mas toca poucas páginas. Truncar
reutiliza páginas **já tocadas** da entrada e as mantém residentes. O protótipo
com cópia exata precisa liberar o buffer grande dentro da chamada; esse custo do
alocador/kernel aparece no tempo. Mesmo assim ficou cerca de 14× mais rápido
que a versão atual nesse caso, sem reter o documento inteiro.

O mesmo risco apareceu em `nln` com 8 MiB de LF e saída vazia: a versão atual
ficou em 2,8 MiB residentes após a chamada, enquanto `retain` no buffer próprio
manteve 10,7 MiB. Os picos ficaram próximos de 10,6 MiB. Portanto, evitar alocação
não implica minimizar a memória mantida pela saída, inclusive quando ela está vazia.

**Recomendação:** devolver fatia para entrada emprestada; em buffer próprio,
reutilizar quando o corte for pequeno e considerar saída exata/liberação de
capacidade quando o resultado for muito menor. `shrink_to_fit` condicional é
outra hipótese a medir, não um resultado demonstrado aqui. Não fazer shrink ou
remoção repetida dentro de um laço por linha.

### 5.3 Reversão: memória versus velocidade

Tempos complementares em **ms**, entrada emprestada:

| Documento | Atual: descritores 32 B/linha | Índice compacto: offsets 8 B/linha | Sem índice: scanner reverso |
| --- | ---: | ---: | ---: |
| 1.000 linhas / 62,5 KiB | 0.072 | 0.073 | 0.121 |
| 8 MiB / linhas de 64 B | 15.366 | 13.572 | 18.568 |
| 64 MiB / linhas de 4 KiB | 83.000 | 81.338 | 121.689 |
| 8 MiB / apenas LF (8.388.608 linhas) | 238.433 | 145.600 | 78.866 |

Picos de RSS em 8 MiB formados apenas por LF:

| Estratégia | Pico RSS mediano |
| --- | ---: |
| Atual | 274,7 MiB |
| Offsets compactos | 82,8 MiB |
| Sem índice | 18,7 MiB |

O `Vec` atual guarda **fatias**, não strings independentes, mas são quatro palavras
por linha no alvo medido: 32×8.388.608 = 256 MiB de metadados antes da saída.
Há também capacidade excedente e realocações amortizadas do vetor em tamanhos
que não sejam potências de dois. Com 1 GiB só de LF, seriam aproximadamente
32 GiB **apenas de descritores**; isso é extrapolação aritmética, não um teste de OOM.

O índice de offsets reduz esse custo quatro vezes por entrada e manteve o tempo
próximo ao atual em linhas longas. A diferença de ~2% nesse conjunto está dentro
da variabilidade observada e não sustenta uma promessa de ganho de CPU.

A reversão sem índice foi excelente no caso denso de linhas vazias, mas **piorou
cerca de 47%** em linhas longas. Portanto, não recomendo substituí-la universalmente.
Offsets são uma primeira mudança de custo moderado; um caminho sem índice pode
ser usado quando houver necessidade demonstrada de limitar memória auxiliar.
Uma escolha adaptativa precisa medir o custo da decisão e de eventuais novas passagens.

### 5.4 Repetir exclusões ainda custa caro

K exclusões da linha 2, 8 MiB, kernels diretos, **ms pela cadeia inteira**:

| K | Cadeia atual | Cadeia com busca curta + reuse | Uma `dlni 2 (K+1)` atual |
| --- | ---: | ---: | ---: |
| 10 | 175,948 | 11,297 | 19,410 |
| 50 | 823,447 | 43,658 | 19,360 |

Mesmo o candidato não elimina os K deslocamentos do sufixo. Para 50 exclusões,
um único intervalo existente foi ~42× mais rápido que a cadeia atual. Reescrever
manualmente o pipeline é uma solução imediata quando não se precisa observar
estados intermediários.

Não fazer essa fusão automaticamente sem considerar watchers, debug, contexto,
variáveis, interrupções e a posição lógica depois de cada passo. A comparação
mede operações constantes diretas, não prova equivalência de pipelines arbitrários.

### 5.5 Scanner, deduplicação e junção

O scanner experimental por bytes, sem dependência nova e preservando os oito
terminadores, reduziu a mediana de leitura de 8 MiB ASCII de **8,228 para 7,070 ms**
(~14%). Em Unicode, de **7,691 para 6,639 ms** (~14%); em linhas longas de 64 MiB,
de **51,331 para 49,868 ms** (~3%). Eliminar passagens inteiras traz benefício maior
que otimizar cada passagem. Não usar `str::lines()` como substituto sem adaptar
a semântica: o conjunto de terminadores do TextForge é mais amplo.

`uniqln`, em 64 MiB de linhas distintas de 64 B, chegou a aproximadamente
**174 MiB de pico RSS**: entrada, saída e conjunto de conteúdos únicos. Não há
um candidato medido de deduplicação nesta auditoria. Guardar só hashes não é
substituto exato: colisões exigem comparação do conteúdo. Usar saída preguiçosa
pode evitar cópia no caso sem duplicatas, mas não elimina O(U) de metadados.

O teste de `joinln` usou um separador de um byte. Com separadores grandes, o
próprio resultado pode ser muito maior que a entrada. Não justificar uma segunda
passagem para calcular capacidade exata apenas com os números deste caso pequeno;
medir a distribuição real de separadores e comprimentos antes.

## 6. Buffered e fronteiras da API

Nenhuma das 16 instruções implementa um `transform_buffered` próprio. O fallback
materializa o documento e executa a transformação de string. **Um threshold de
spool não é um limite global de RAM para essas instruções.**

Há uma distinção importante no código atual:

- `process_all`/batch no caminho buffered padrão usam `retain_materialized`.
  Uma vez materializada, a saída pode continuar como string entre passos;
  não é correto afirmar que todos esses caminhos regravam o spool a cada instrução.
- `process_all_buffered` explícito com um buffer comum não retido pode materializar
  e escrever o resultado no spool novamente a cada fallback.
- `PortableProcessor::run_transform` usa diretamente `Cow::Borrowed` e converte
  para `String`; `process_all` delega o método do core, cujo caminho depende das features.

Esses pontos são verificados por leitura de código, **não** por benchmark de I/O
nesta auditoria. Operações de corte, filtro e junção são candidatas a execução
incremental por chunks, mas exigem terminadores/UTF-8 atravessando chunks,
preservação de erros e integração com a infraestrutura existente. Regex por
linha pode exigir memória proporcional à maior linha; reversão e deduplicação
exata têm requisitos adicionais. Não prometer RAM constante universal.

## 7. Prioridades e custo de implementação

Estimativas são julgamento de engenharia, não horas medidas: desenvolvedor
familiar com o projeto, sem mudança de API pública ou novas dependências. Incluem
regressões Unicode/EOF/erros e a matriz local de features. CI multiplataforma e
sincronização autorizada de snapshots dos bindings acrescentam trabalho.

| Prioridade | Mudança | Custo indicativo | Justifica? |
| --- | --- | --- | --- |
| Alta | Helper de limites, parada antecipada e cópia em blocos em `dln/dlni/dlna/dlnb/slnc` | 1–3 dias para a família e testes | **Sim.** Grande ganho medido, desenho local; saída pequena precisa de política de capacidade. |
| Alta | `revln` com offsets compactos | 1–2 dias | **Sim para arquivos com muitas linhas.** Grande redução de pico, sem perda material demonstrada no conjunto longo. |
| Alta | `apl/inl/pln/rln`: helper privado, reserva e uso de `Owned`, sem criar tokens temporários | 2–4 dias para a família | **Sim.** Append medido; inserção/substituição ainda precisam de comparação específica do candidato. |
| Média | Fast paths e alocação tardia em `nln/dle/klm/dlm/uniqln/joinln` | 1–3 dias para os casos comuns | **Provável.** `nln` sem quebras foi medido; filtros e deduplicação precisam medir seus candidatos e taxas de descarte. |
| Média/baixa | Scanner por bytes seguro | 0,5–2 dias | **Depois das passagens redundantes.** Ganho 3–14% nos tamanhos grandes medidos, menos dominante após buscas curtas. |
| Condicional | `revln` sem índice/adaptativo | Mais 1–3 dias | **Se RAM for limitante.** Melhora o extremo denso, mas pode perder desempenho em linhas longas. |
| Condicional | Overrides buffered com processamento incremental | Uma ou mais semanas por conjunto, conforme a infraestrutura necessária | **Se arquivos excederem o orçamento de RAM.** Necessário medir I/O real; os microbenchmarks não estabelecem o retorno completo. |
| Baixa agora | Rope/piece table/documento indexado persistente | Semanas, com impacto em muitos transforms e observabilidade | **Não justificado pelos dados atuais.** O contrato existente já permite boa parte dos ganhos locais. |

Essas estimativas não são promessa de prazo. Para uma carga exclusivamente de
arquivos pequenos e poucas operações, a manutenção adicional pode valer menos
que dezenas de microssegundos economizados. Para arquivos de vários MiB,
processamento em lote ou pipelines longos, os ganhos locais são suficientemente
grandes para justificar uma primeira rodada focada.

O retorno operacional depende de quantas vezes cada caminho roda. Por exemplo,
a diferença medida de `dln` em 8 MiB emprestados foi cerca de **16 ms/operação**;
10.000 chamadas representam aproximadamente **160 s de execução acumulada dos transforms**, antes de disco,
FFI e concorrência. Isso é uma projeção do kernel, não um SLA de produção.

## 8. O que eu implementaria primeiro

1. Um helper privado que encontra limites e para assim que os encontra. Em erro,
   preservar a contagem/contexto e a precedência atuais, sem mudar construtores.
2. `dln`/`dlni` por cópia de poucos blocos ou compactação única; cortes por range
   com tratamento diferente para `Borrowed`, `Owned` e redução grande de tamanho.
3. Append/prepend/inserção/substituição sem instanciar outros tokens durante
   `transform`; reservar apenas o necessário e conservar ownership quando possível.
4. Trocar os descritores de `revln` por offsets compactos. Manter a iteração sem
   índice como opção a avaliar para cargas limitadas por memória, não como padrão universal.
5. Medir e aplicar fast paths sem alteração. Só depois decidir sobre scanner,
   índices persistentes, heurísticas adaptativas e execução streaming.

Para aceitar os patches: comparar bytes de saída e todos os campos de erro,
rodar os testes de texto/bytecode/portable/watchers/buffered, observar capacidade
retida e RSS além da latência e repetir benchmarks em cargas reais. Alterações
nos bindings ou contratos públicos seguem o processo de aprovação do projeto;
esta exploração não as executou.

## 9. Fontes e reprodução

Leitura central: [scanner e erros](../../../src/utils/lines.rs),
[dln](../../../src/tokens/transforms/dln/mod.rs),
[slnc](../../../src/tokens/transforms/slnc/mod.rs),
[apl](../../../src/tokens/transforms/apl/mod.rs),
[revln](../../../src/tokens/transforms/revln/mod.rs),
[uniqln](../../../src/tokens/transforms/uniqln/mod.rs),
[fallback do trait](../../../src/tokens/mod.rs),
[TextBuffer](../../../src/api/text_buffer.rs),
[processador buffered](../../../src/api/processor/buffered.rs),
[processador portable](../../../src/api/portable/processor.rs).

```sh
cargo build --release --locked --example line_performance_audit
./target/release/examples/line_performance_audit verify
python3 scripts/run_line_performance_audit.py --binary "$PWD/target/release/examples/line_performance_audit"
python3 scripts/run_line_performance_extras.py --binary "$PWD/target/release/examples/line_performance_audit"
```

Reprodução requer Linux, `taskset`, `/usr/bin/time`, Python 3 e Rust. Os scripts
reescrevem os JSONs de resultados. As variações pequenas devem ser avaliadas
pelos quartis; repetir em outra máquina não deve produzir os mesmos números.
O código experimental fica apenas em
[examples/line_performance_audit.rs](../../../examples/line_performance_audit.rs).

### Checks executados para os artefatos da auditoria

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features test_access -- -D warnings`
- `cargo test`, `cargo test --features bytecode`, `cargo test --features watchers`
  e `cargo test --features test_access`
- Build release e execução de `line_performance_audit verify`
- Ruff lint/format nos dois scripts Python; ambos foram executados para coletar os dados.

Os checks do core passaram; não houve alteração nos repositórios dos bindings.
