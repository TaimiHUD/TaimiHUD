## Common

join-discord = Junte-se ao nosso Discord
discord-link = "https://discord.gg/dKpaphTMGS"
having-issues = Caso tenha problemas com o TaimiHUD, entre em contato pelo Discord ou GitHub Issues!
height = Altura
font = Fonte
okay = OK
delete = Excluir
copy = Copiar
copy-arg = { copy } { $arg }
save = Salvar
quit = Sair
delete-item = { delete } "{ $item }"?
save-item = { save } "{ $item }"?
save-standalone = { save } como um novo arquivo
save-append = Anexar a um arquivo existente
save-edit = { save } alterações
save-edit-item = { save-edit } em "{ $item }"?
save-mode = Modo de salvamento
error = Erro
remove = Remover
unknown = Desconhecido
update = Atualizar
auto-update = Atualização automática
always = Sempre
ask = Perguntar
never = Nunca
unset = Não definido
add = Adicionar
create-arg = Criar novo { $arg }
not-create-arg = Use { $arg } existente
description = Descrição
location = Localização: { $path }
data = Dados
object = Objeto
files = Arquivos
clear = Limpar
refresh-files = Atualizar arquivos
# as in 3D
model = Modelo
revert = Reverter
close = Fechar
name = Nome
icon = Ícone
path = Caminho
title = Título
menu = Menu
controls = Controles
id = ID
category = Categoria
id-arg = { id }: { $id }
map-id = Mapa { id }
map-id-arg = { map-id }: { $id }
author = Autor(a)
position = posição
position_cap = Posição
not-applicable = N/A
rt-api-required-base = RTAPI é necessário para
rt-api-required = { rt-api-required-base } { $reason }.
no-description = Nenhuma descrição fornecida.
no-thing-arg = Nenhum { $thing } fornecido.
expand-all = Expandir Tudo
filetype = Tipo do Arquivo
filename = Nome do Arquivo
collapse-all = Recolher Tudo
active = Ativo
inactive = Inativo
enable = Habilitar
cancel = Cancelar
default = Padrão
disable = Desativar
enabled = Habilitado #TODO: idk if there is a better way to do past tense in FluentProject
disabled = Desativado #TODO: idk if there is a better way to do past tense in FluentProject
author-arg = { author }: { $author }
reset = Reiniciar
timer = Temporizador
timers = { timer }es
experimental-notice = Olá! Este recurso é (em sua maior parte) experimental. Algumas coisas podem ser confusas e podem exigir mais raciocínio e esforço para serem usadas do que os recursos menos experimentais. Peço desculpas por quaisquer problemas que você tenha; sinta-se à vontade para entrar em contato no Discord. - Kat
name-empty = Nome vazio.
no-trigger = Nenhuma posição de gatilho fornecida.
no-category = Nenhuma categoria fornecida.
map-id-wrong = ID do mapa incorreta.
no-positions = Nenhuma posição de marcador foi fornecida.
validation-fail = A validação falhou devido a:
filename-empty = Nenhum nome de arquivo fornecido.
count = Contagem #TODO: context?
actions = Ações
module = Módulo
unspecified = Não especificado

## Addon

addon = Extensão #TODO: should I keep it in english?
primary-window-toggle = Ativar/Desativar a janela da Taimi
context-menu-primary = { menu }
timer-window-toggle = Ativar/Desativar a janela do { timer }
marker-window-toggle = Ativar/Desativar a janela do Marcador
pathing-window-toggle = Ativar/Desativar a janela de Percurso
pathing-render-toggle = Ativar/Desativar a renderização de Percurso
pathing-render-minimap-toggle = Ativar/Desativar o percurso no minimapa
pathing-render-map-toggle = Ativar/Desativar o percurso no mapa
primary-window-toggle-text = Mostrar/ocultar janela principal da Taimi
timer-key-trigger = Gatilho de tecla do temporizador { $id }
timer-key-reset = Reiniciar { timers }

## Config

config-tab = Configuração
stock-imgui-progress-bar = Barra de progresso padrão do Imgui
shadow = Sombra
centre-text-after-icon = Centralizar o texto após o ícone
imgui-notice = Você pode clicar com a tecla Control pressionada em um elemento de controle deslizante, ou similar, para inserir dados diretamente nele. Lembre-se de pressionar Enter após inserir o valor.
context-click-notice = Clique com o botão direito para ver mais opções
dpi-scaling = Dimensionamento de DPI #TODO: or just Escala
dpi-notice = Certifique-se de que essa configuração corresponda à configuração de "{dpi-scaling}" nas Opções Gráficas do jogo para que os elementos do mapa sejam exibidos corretamente.
marker-trigger = Comportamento de ativação da posição do marcador
marker-condition = Condição comportamental
autoplace-warning = Caso você não tenha o RTAPI instalado, não conseguiremos detectar se você é um tenente em vez de um comandante.
nexus-quick-access = Ícones de acesso rápido
icon-style = Estilo de ícone
icon-style-plain = Plano
icon-style-scanlines-1 = Linhas de varredura
preferred-loader = Preferência do carregador
preferred-updater = Atualizar preferências do host
gh-api-token = Token da API do GitHub
gh-api-token-notice = Erros de limite de taxa ao atualizar fontes de dados podem ser evitados configurando um token personalizado - forneça-o somente se você entender as implicações disso!
language = Idioma
addonbinds = Atalhos
gamebinds = Atalhos de teclado do jogo
keybind = Atalhos de teclado
gamebind-notice = Configure essas opções para corresponder às suas configurações de controles no jogo. Elas podem ser detectadas automaticamente quando o pacote arcdps-unofficial-extras estiver instalado.
precise-markers = Marcadores precisos #TODO: or  "Marcadores de Precisão", I would need context for this

## Windows

primary-window = TaimiHUD
timers-window = { timers } de Encontro
markers-window = Marcadores de Esquadrão
pathing-window = Pacotes de Percursos
# deprecated(?) aliases
timer-window = { timers-window }
marker-window = { markers-window }

## Modals

addon-uninstall-modal-title = Desinstalar { $source }?
addon-uninstall-modal-button = Desinstalar
addon-uninstall-modal-description = Por favor, tenha cuidado! Isso apagará a pasta e tudo o que ela contém.
delete-markerset-warning = Atenção! Isso excluirá o registro do conjunto de marcadores no arquivo.
overwrite-markerset = Atenção! Isso sobrescreverá o registro do conjunto de marcadores no arquivo.

## Openable

open-button = Abrir { $kind }
open-error = { error } ao abrir { $kind }: { $path }

## Data sources

intro-to-data-sources = Certifique-se de atualizar o repositório antes de verificar se há atualizações.
data-sources = Fontes de dados
data-sources-tab = { data-sources }
data-source-repo-update = Atualizar fontes
data-source-repo-update-tooltip = Recupere o repositório de fontes de dados para visualizar os itens disponíveis para download.
checking-for-updates = Verificando atualizações...
downloading-update = Baixando atualização...
check-for-updates = Verifique se há atualizações
check-for-updates-tooltip = Verifique se há atualizações em todas as fontes de dados. Não fazemos isso automaticamente para respeitar sua escolha sobre se deseja ou não fazer solicitações de rede.
checked-for-updates-last = Última verificação de atualizações em: { $time }
reload-data-sources = Recarregar fontes de dados
reload-data-sources-tooltip = Recarrega itens das fontes de dados atualmente instaladas. Útil se você tiver alterado os arquivos dentro delas!

remote = Remoto #TODO: there might be a better translation here depending on the context
update-status = Atualizar status
version-installed = Versão instalada: { $version }
version-not-installed = Não instalado
update-unknown = Status da atualização desconhecido; verificar se há atualizações?
update-not-required = Atualização não necessária; está atualizado!
update-available = Nova versão disponível: { $version }!
update-error = { error } ao atualizar: { $error }!
download = Download
install = Instalar
attempt-update = Tentar atualizar mesmo assim?
settings-unloaded = As configurações ainda não foram carregadas!
available = Disponível
up-to-date = Atualizado!

## Info tab

info-tab = Informações
keybind-triggers = Se você precisar de gatilhos de temporizador baseados em atalhos de teclado, configure as teclas apropriadas nas configurações do carregador.
active-timer-phases = Fases de temporizador ativas
phase = Fase
# As in, like, "game engine" or "rendering engine" :o
engine = Motor
ecs-data = { data } do ECS
object-data = { data } do { object }
object-kind = Tipo de { object }
model-files = Arquivos de { model }
vertices = Vértices
textures = Texturas: { $count }
alloc-size = Alocações: { $size }
d3d-textures = Texturas do D3D: { $count }
size-frag = { $size } { $suffix }
#size-frag-mb = { size-frag(suffix: "MB", size: "$size") }
size-frag-mb = { $size } MB
size-frag-kb = { $size } KB

## Arc

arcdps = ArcDPS
arcdps-tab = { arcdps }
nexus = Nexus

## Markers tab

reload-markers = Recarregar { markers }
marker-tab = { marker-window }
pathing-tab = { pathing-config }
marker = Marcador
markers = { marker }es
markers-place = { markers } de posição
marker-set = Conjunto de { markers }
marker-set-create = Criar { marker-set }
marker-set-edit = Editar { marker-set }
marker-set-delete = Excluir { marker-set }
scaling-factor = fator de escala
current-scaling-factor = { scaling-factor } atual: ({ $x }, { $y })
current-scaling-factor-multiple = { scaling-factor } atual como um múltiplo de pés por unidade continental: ({ $x }, { $y })
scaling-factor-reset = { reset } a { scaling-factor } detectada
no-file-associated = Não foi possível encontrar o arquivo associado.
markers-arg = { markers }: { $count }
marker-type = Tipo do { marker }
local-header = Local (XYZ)
map-header = Mapa (XY)
screen-header = Tela (XY)
marker-not-on-screen = Não está na tela
select-a-marker = Por favor, selecione um marcador para configurar!
marker-filetype-explanation = Existem três tipos de arquivos de marcadores: há o tipo que
  vem com o módulo Commander's Markers do BlishHUD (integrado), há o tipo que eles usam para distribuir os Community Markers e há meu próprio formato, que pega o formato de conjunto de marcadores e o transforma em um único arquivo por conjunto de marcadores.
no-markers-for-map = Nenhum marcador encontrado para o mapa atual.
cant-place-markers = Impossível de posicionar
autoplacement-disable = Desativar posicionamento automático
autoplacement-enable = Ativar posicionamento automático
always-do-action = Sempre aja
do-action-if-commander = Execute a ação se for o comandante
do-action-if-lieutenant = Execute a ação se for tenente ou comandante
never-do-action = Nunca faça nada
open-markers-window = Abra a janela de marcadores
place-markers-automatically = Posicionar marcadores automaticamente
do-nothing = Não faça nada

## Markers window
clear-markers = { clear } { markers }
clear-spent-autoplace = Redefinir posicionamento automático gasto

## Edit markers window

edit-markers = Criar/editar marcadores
set-map-id = Defina o ID do mapa para o mapa atual
current-squad-markers = marcadores de esquadrão atuais
take-squad-markers = Extraia dos { current-squad-markers }
cannot-take-squad-markers = Não é possível extrair dos { current-squad-markers }; não está em um esquadrão.
rt-api-required-squad-markers = { rt-api-required-base } extração automática da localização dos marcadores de esquadrão.
no-position = Nenhuma posição informada.
trigger = Acionar: { $position }
position-plain = { $position }
position-get = Obtenha a { position } atual
set-manually = Configurar manualmente
manual-position = { position } manual
set-manually-save = { save } { position } manual
trigger-explanation = Um gatilho para um conjunto de marcadores é uma esfera com raio de 15 metros, centrada no local do gatilho.

## Timer tab

reload-timers = Recarregar { timers }
timer-tab = { timer-window }
source-arg = Fonte: { $source }
source-adhoc = Fonte: Ad-hoc
select-a-timer = Por favor, selecione um temporizador para configurar!

## Timer window

no-phases-active = Nenhuma fase está ativa no momento, nenhum temporizador está em execução.
reset-timers = { reset } { timers }

## Pathing

pathing = Percurso
trail = Trilha
poi = Ponto de interesse #TODO: that I can't remember we don't have a acronym for POI in pt-br
space = KatRender
reload-packs = Recarregar
unload-packs = Descarregar tudo
filter-options = Opções de filtro
searchbar-clear = Limpe a barra de pesquisa e os resultados.
show-filter = Mostrar opções de filtro
hide-filter = Ocultar opções de filtro
current-map = Mapa atual
ignore-root = Ignorar estado raiz #TODO: root, leaf, branch are very much enginnering terms which usually we don't translate, so I went with literal translation for now, maybe there is a better option
ignore-leaf = Ignorar estado da folha
ignore-branch = Ignorar estado do ramo
show-hidden = Mostrar oculto
show-all = Mostrar tudo
#off-map = Elsewhere #TODO: was this supposed to be commented out with # or is something special?
ignore-whitespace = Ignore os espaços em branco
case-insensitive = Não diferencia maiúsculas de minúsculas
toggle = Ativar/Desativar
pathing-config = Opções de percurso
pathing-config-enable = {space} Percurso (Experimental)
pathing-config-minimap = Opções do minimapa
pathing-config-worldmap = Opções do mapa
pathing-config-trail-alpha = Opacidade
pathing-config-trail-alpha-minimap = Opacidade do minimapa
pathing-config-trail-alpha-worldmap = Opacidade do mapa
pathing-config-poi-alpha = Opacidade do painel de exibição
pathing-config-poi-alpha-minimap = Opacidade dos pontos de interesse no minimapa
pathing-config-poi-alpha-worldmap = Opacidade dos pontos de interesse
pathing-config-trail-scale = Escala
pathing-config-trail-scale-minimap = Escala do minimapa
pathing-config-trail-scale-worldmap = Escala do mapa
pathing-config-poi-scale = Tamanho do painel de exibição
pathing-config-poi-scale-minimap = Tamanho dos pontos de interesse no minimapa
pathing-config-poi-scale-worldmap = Tamanho dos pontos de interesse
pathing-config-player-overlap-threshold = Desvanecer perto do jogador
pathing-config-distance-fade-intensity = Intensidade
pathing-config-distance-max = Distância
pathing-config-textured = Trilhas texturizadas
pathing-config-textured-minimap = Trilhas texturizadas
pathing-config-textured-worldmap = Trilhas texturizadas
pathing-config-map-open = Fwoom
pathing-config-camera-source = Fonte de dados da câmera
pathing-config-advanced = Configurações avançadas
pathing-config-trail-notice = As configurações de geração de trilhas podem exigir uma alteração ou recarga do mapa para entrarem em vigor e podem não funcionar como você espera.
pathing-config-trail-y-offset = Deslocamento vertical
pathing-config-trail-resolution = Resolução da Trilha
pathing-config-trail-width = Largura Padrão #TODO: base as in default or initial?
pathing-config-goggles = Experimento: Óculos de raios X
pathing-config-goggles-notice = Atualmente, isso requer que você defina a Amostragem de Renderização como Nativa nas Opções Gráficas.
pathing-config-festivals = Festivais
pathing-config-festival-active = {$festival} (ativo)
pathing-config-reset-notice = Clique com o botão direito em qualquer controle deslizante abaixo para restaurar sua configuração padrão.
pathing-config-edge-feather-scale = escala de suavização de borda
pathing-config-corner-boudary-scale = escala do limite do canto
pathing-notice-space = {space} é necessário para a funcionalidade de percurso.
pathing-notice-mumblelink = se você estiver enfrentando travamentos, tente alterar a Sincronização Vertical nas configurações gráficas do jogo
pathing-notice-rtapi-missing = RTAPI é uma extensão separada que deve ser instalada via Nexus.
pathing-notice-rtapi = se você estiver enfrentando problemas de travamento, tente alterar a Sincronização Vertical ou mudar para o MumbleLink
mumblelink = MumbleLink
rtapi = Nexus RealTime API

## Space

render-unload = Descarregar Renderização
render-reload = Recarregar Renderização
render-notice-gameplay = Faça login no jogo para começar
render-notice-gameplay-initial = Selecione um personagem para começar
render-notice-error = Erro! Consulte o log na pasta de addon do Nexus ou do Taimi para obter mais detalhes
packs-empty = Nenhum arquivo carregado
packs-empty-notice = Depois de instalados a partir da aba { data-sources-tab } ou baixados manualmente, o botão "Recarregar" deverá detectá-los!

## Festivals

festival = Festival
# TODO: I considered translating the festival names, but as GW2 does not have PT-BR language that could create confusion.
halloween = Halloween
wintersday = Wintersday
superadventurefestival = Super Adventure Box
lunarnewyear = Lunar New Year
festivalofthefourwinds = Festival Of The Four Winds
dragonbash = Dragon Bash

## Gamebinds (see `default_keybind`s in src/exports/runtime/bindings/controls.rs)
UI_ShowHideUI = Alternar Visibilidade da Interface
