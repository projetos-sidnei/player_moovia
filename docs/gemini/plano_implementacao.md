## Plano de Implementação - Funcionalidade de Cursos e Exclusão de Vídeos

Este documento detalha o plano para adicionar a funcionalidade de cursos ao player e a opção de exclusão de vídeos, seguindo as convenções do projeto e a estrutura existente.

### 1. Atualizar o Modelo de Dados (Backend - Rust)

- Adicionar um novo tipo `CollectionType::Course` ao enum `CollectionType` em `src-tauri/src/db/models.rs`. Isso permitirá categorizar pastas como cursos.

### 2. Modificar o Schema do Banco de Dados (Backend - Rust)

- Criar uma nova migration para atualizar a coluna `collections.type` para aceitar o novo valor 'course'.

### 3. Implementar Comando para Adicionar Curso Específico (Backend - Rust)

- Criar um novo comando Tauri (ex: `add_course_collection(path: String)`) que permitirá ao usuário selecionar uma pasta específica para ser adicionada como uma coleção do tipo `Course`.
- Este comando acionará uma lógica de scanner simplificada para cursos, onde todos os arquivos de vídeo dentro da pasta selecionada (e suas subpastas) serão tratados como vídeos individuais, sem a abstração de temporadas.

### 4. Ajustar a Lógica do Scanner para Cursos (Backend - Rust)

- Modificar a função de scan existente ou criar uma nova para processar `CollectionType::Course`, populando diretamente a tabela `videos` sem criar `seasons`.

### 5. Atualizações na Interface do Usuário para Adicionar Cursos (Frontend - TypeScript/React)

- Adicionar um botão na interface (ex: "Adicionar Curso") que abre um diálogo de seleção de pasta nativo (`@tauri-apps/plugin-dialog`).
- Ao selecionar uma pasta, invocar o comando `add_course_collection`.

### 6. Atualizações na Interface do Usuário para Exibir Cursos (Frontend - TypeScript/React)

- Modificar a exibição da biblioteca para diferenciar e renderizar corretamente as coleções do tipo `Course`.
- Ajustar a tela de detalhes da coleção para cursos, listando os vídeos de forma linear.

### 7. Implementar a Seleção de Todos os Vídeos (Backend & Frontend)

- Criar um comando Tauri (ex: `get_all_videos_in_collection(collection_id: i32)`) que retorna uma lista de IDs de vídeo para uma coleção específica.
- Adicionar um botão ou checkbox "Selecionar Todos" na interface do usuário para acionar este comando.

### 8. Implementar a Funcionalidade de Exclusão de Vídeos (Backend & Frontend)

- Criar dois novos comandos Tauri:
  - `delete_videos_from_player(video_ids: Vec<i32>)`: Removerá as entradas dos vídeos selecionados apenas do banco de dados do player.
  - `delete_videos_from_pc(video_ids: Vec<i32>)`: Removerá as entradas dos vídeos selecionados do banco de dados E excluirá os arquivos de vídeo correspondentes do sistema de arquivos. Este comando exigirá aprovação explícita do usuário.
- Adicionar botões correspondentes na interface do usuário para essas ações.
